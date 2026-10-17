//! Registered-pose visualisation and servo-rate pen coupling; docs/spec/11-relative-pointer.md.
use macroquad::prelude::Vec3;
use std::collections::VecDeque;
use std::io::Write;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};
use visiontouch_device::{Demand, Device, ForceLimits, RequestError, Telemetry};
use visiontouch_hal::device::Sample;

/// A coupling descriptor older than this is ignored and the pen force is zero, so a
/// stalled UI cannot tether the pen to a frozen proxy (about six frames at 60 fps).
pub const COUPLING_MAX_AGE: Duration = Duration::from_millis(100);
/// Default device coupling stiffness, in model N/m (not scene newtons per unit).
pub const DEFAULT_STIFFNESS_N_M: f64 = 150.;
/// Slider range for the coupling stiffness.
pub const STIFFNESS_RANGE: std::ops::RangeInclusive<f64> = 20.0..=1500.0;
/// Default coupling damping, in model N·s/m.
pub const DEFAULT_DAMPING_N_S_M: f64 = 1.;
/// Time constant of the velocity low-pass, which hides single-count quantisation.
const VELOCITY_TAU_S: f64 = 0.01;
const SCENE_ORIGIN_Y: f32 = -0.3;

/// Why the pen is receiving no force.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MuteReason {
    /// The simulation is paused.
    NotRunning,
    /// The time-scale slider is not at 1.00x.
    NotRealTime,
    /// The reset pose has not been registered on this connection.
    Unregistered,
    /// The scene has not refreshed the coupling within [`COUPLING_MAX_AGE`].
    SceneStalled,
    /// Pen force feedback is switched off.
    FeedbackOff,
}
impl MuteReason {
    /// Operator-facing text shown beside "Pen force".
    pub fn text(self) -> &'static str {
        match self {
            Self::NotRunning => "force muted: simulation paused",
            Self::NotRealTime => "force muted: time scale is not 1.00x",
            Self::Unregistered => "force muted: zero at reset pose first",
            Self::SceneStalled => "force muted: scene stalled",
            Self::FeedbackOff => "force off: feedback not enabled",
        }
    }
}

/// Spring-damper tether between the pen and the scene proxy, in registered model
/// metres. The worker evaluates it at servo rate from each fresh sample.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coupling {
    /// Proxy position in registered model metres.
    pub proxy_m: [f64; 3],
    /// Proxy velocity in model m/s.
    pub proxy_velocity_m_s: [f64; 3],
    /// Scene box in registered model metres, `(min, max)` per axis. The pen is clamped
    /// into it before the spring is evaluated, so a pen outside the scene feels no
    /// force the scene did not ask for (the proxy never leaves the box).
    pub bounds_m: ([f64; 3], [f64; 3]),
    /// Model N/m.
    pub stiffness_n_m: f64,
    /// Model N·s/m.
    pub damping_n_s_m: f64,
    /// When the UI built this descriptor.
    pub issued: Instant,
}
impl Coupling {
    /// `F = K (proxy - p) - B (v - v_proxy)`, a model force command in newtons.
    /// The pen is clamped into [`Self::bounds_m`] first; on a clamped axis its
    /// velocity is taken as the proxy's.
    pub fn force(&self, p: [f64; 3], v: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|i| {
            let (lo, hi) = (self.bounds_m.0[i], self.bounds_m.1[i]);
            let pc = p[i].clamp(lo, hi);
            let vc = if pc == p[i] {
                v[i]
            } else {
                self.proxy_velocity_m_s[i]
            };
            self.stiffness_n_m * (self.proxy_m[i] - pc)
                - self.damping_n_s_m * (vc - self.proxy_velocity_m_s[i])
        })
    }
}

/// Servo state of the worker: the held coupling and the velocity estimate.
#[derive(Default)]
struct Servo {
    coupling: Option<Coupling>,
    last: Option<(Instant, [f64; 3])>,
    velocity: [f64; 3],
    muted_ticks: u64,
}
impl Servo {
    /// One servo tick from a fresh sample. Returns the demand and the mute reason.
    fn tick(
        &mut self,
        now: Instant,
        position: Option<[f64; 3]>,
        feedback: bool,
    ) -> (Demand, Option<MuteReason>) {
        let Some(p) = position else {
            self.last = None;
            self.velocity = [0.; 3];
            return self.mute(if feedback {
                MuteReason::Unregistered
            } else {
                MuteReason::FeedbackOff
            });
        };
        if let Some((t, q)) = self.last {
            let dt = now.saturating_duration_since(t).as_secs_f64();
            if dt > 0. {
                let a = 1. - (-dt / VELOCITY_TAU_S).exp();
                for i in 0..3 {
                    self.velocity[i] += a * ((p[i] - q[i]) / dt - self.velocity[i]);
                }
            }
        }
        self.last = Some((now, p));
        if !feedback {
            return self.mute(MuteReason::FeedbackOff);
        }
        match self
            .coupling
            .filter(|c| now.saturating_duration_since(c.issued) < COUPLING_MAX_AGE)
        {
            Some(c) => (Demand::Force(c.force(p, self.velocity)), None),
            None => self.mute(MuteReason::SceneStalled),
        }
    }
    fn mute(&mut self, reason: MuteReason) -> (Demand, Option<MuteReason>) {
        if reason != MuteReason::FeedbackOff {
            self.muted_ticks += 1;
        }
        (Demand::Force([0.; 3]), Some(reason))
    }
}

/// The slice of [`Device`] the servo loop needs, so the loop can run on a fake.
trait Link {
    fn sample(&mut self) -> std::io::Result<Sample>;
    fn position_m(&self) -> Option<[f64; 3]>;
    fn feedback_enabled(&self) -> bool;
    fn request(&mut self, demand: Demand, time: Instant) -> Result<(), RequestError>;
    fn telemetry(&self) -> Telemetry;
}
impl Link for Device {
    fn sample(&mut self) -> std::io::Result<Sample> {
        Device::sample(self)
    }
    fn position_m(&self) -> Option<[f64; 3]> {
        self.state.position_m()
    }
    fn feedback_enabled(&self) -> bool {
        Device::feedback_enabled(self)
    }
    fn request(&mut self, demand: Demand, time: Instant) -> Result<(), RequestError> {
        Device::request(self, demand, time)
    }
    fn telemetry(&self) -> Telemetry {
        self.guard.telemetry()
    }
}

/// Result of one worker iteration.
struct Serviced {
    sample: Sample,
    position_m: Option<[f64; 3]>,
    telemetry: Telemetry,
    muted: Option<MuteReason>,
    muted_ticks: u64,
}

/// One worker iteration: always one sample, then (feedback armed) one fresh request
/// computed from that sample, so a request is never older than one loop.
fn service<L: Link>(link: &mut L, servo: &mut Servo, now: Instant) -> Result<Serviced, String> {
    let sample = link.sample().map_err(|e| format!("Read stopped: {e}"))?;
    let position_m = link.position_m();
    let feedback = link.feedback_enabled();
    let (demand, muted) = servo.tick(now, position_m, feedback);
    if feedback {
        link.request(demand, now)
            .map_err(|e| format!("Force request stopped: {e}"))?;
    }
    Ok(Serviced {
        sample,
        position_m,
        telemetry: link.telemetry(),
        muted,
        muted_ticks: servo.muted_ticks,
    })
}

/// Event from the worker to the UI.
pub enum Event {
    Connected,
    Sample {
        sample: Sample,
        time: Instant,
        position_m: Option<[f64; 3]>,
        telemetry: Telemetry,
        muted: Option<MuteReason>,
        muted_ticks: u64,
    },
    Registered,
    Feedback(bool),
    Notice(String),
    Disconnected,
    Error(String),
}
enum Command {
    Connect,
    Disconnect,
    Stop,
    Feedback(bool),
    RegisterReset,
    Coupling(Option<Coupling>),
}
/// Apply every queued coupling update to the servo and return the other commands, so
/// a flood of UI messages can never starve sampling.
fn drain(rx: &Receiver<Command>, servo: &mut Servo) -> Vec<Command> {
    let mut other = Vec::new();
    for command in rx.try_iter() {
        match command {
            Command::Coupling(c) => servo.coupling = c,
            command => other.push(command),
        }
    }
    other
}

struct Feed {
    commands: Sender<Command>,
    events: Receiver<Event>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Feed {
    fn spawn() -> Self {
        let (commands, rx) = mpsc::channel();
        let (tx, events) = mpsc::channel();
        let worker = thread::spawn(move || {
            let mut reader: Option<Device> = None;
            let mut servo = Servo::default();
            let mut capture = std::env::args()
                .find_map(|arg| arg.strip_prefix("--capture-input=").map(str::to_owned))
                .and_then(|path| std::fs::File::create(path).ok())
                .map(std::io::BufWriter::new);
            let capture_start = Instant::now();
            if let Some(file) = capture.as_mut() {
                let _ = writeln!(file, "elapsed_ms,raw_hex");
            }
            'worker: loop {
                if visiontouch_device::shutdown::requested() {
                    break;
                }
                for command in drain(&rx, &mut servo) {
                    match command {
                        Command::Stop => break 'worker,
                        Command::Coupling(_) => {}
                        Command::Feedback(enabled) => {
                            let Some(device) = reader.as_mut() else {
                                continue;
                            };
                            let result = if enabled && device.state.registration().is_none() {
                                let _ = tx.send(Event::Feedback(false));
                                continue;
                            } else if enabled {
                                // Mass Lab inherits the proven default envelope
                                // (0.5 N / 96 counts, ADR-0002); only the Studio's
                                // verification ladder constructs higher limits.
                                device
                                    .guard
                                    .set_limits(ForceLimits::default())
                                    .map_err(std::io::Error::other)
                                    .and_then(|()| device.enable_feedback())
                            } else {
                                device.disable_feedback()
                            };
                            if let Err(e) = result {
                                reader = None;
                                let _ = tx.send(Event::Error(format!("Feedback stopped: {e}")));
                            } else {
                                let _ = tx.send(Event::Feedback(enabled));
                            }
                        }
                        Command::RegisterReset => {
                            let Some(device) = reader.as_mut() else {
                                continue;
                            };
                            match device.register_reset() {
                                Ok(_) => {
                                    servo = Servo::default();
                                    let _ = tx.send(Event::Feedback(false));
                                    let _ = tx.send(Event::Registered);
                                }
                                Err(e) if e.contains("reconnect") => {
                                    reader = None;
                                    let _ = tx.send(Event::Error(e.into()));
                                }
                                Err(e) => {
                                    let _ = tx.send(Event::Notice(e.into()));
                                }
                            }
                        }
                        Command::Disconnect => {
                            if let Some(device) = reader.take()
                                && let Err(e) = device.close()
                            {
                                let _ = tx.send(Event::Error(format!("Disconnect: {e}")));
                                continue;
                            }
                            if tx.send(Event::Disconnected).is_err() {
                                break 'worker;
                            }
                        }
                        Command::Connect => {
                            if reader.is_some() {
                                continue;
                            }
                            match Device::connect("/dev/parport0") {
                                Ok(device) => {
                                    reader = Some(device);
                                    servo = Servo::default();
                                    if tx.send(Event::Connected).is_err() {
                                        break 'worker;
                                    }
                                }
                                Err(e) => {
                                    if tx.send(Event::Error(format!("{e}"))).is_err() {
                                        break 'worker;
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some(device) = reader.as_mut() {
                    match service(device, &mut servo, Instant::now()) {
                        Ok(s) => {
                            if let Some(file) = capture.as_mut() {
                                let hex: String =
                                    s.sample.raw.iter().map(|b| format!("{b:02x}")).collect();
                                if writeln!(
                                    file,
                                    "{:.3},{hex}",
                                    capture_start.elapsed().as_secs_f64() * 1000.
                                )
                                .and_then(|()| file.flush())
                                .is_err()
                                {
                                    eprintln!("Input capture stopped: write failed");
                                    capture = None;
                                }
                            }
                            if tx
                                .send(Event::Sample {
                                    sample: s.sample,
                                    time: Instant::now(),
                                    position_m: s.position_m,
                                    telemetry: s.telemetry,
                                    muted: s.muted,
                                    muted_ticks: s.muted_ticks,
                                })
                                .is_err()
                            {
                                break;
                            }
                        }
                        Err(e) => {
                            reader = None;
                            if tx.send(Event::Error(e)).is_err() {
                                break;
                            }
                        }
                    }
                }
                thread::sleep(Duration::from_millis(2));
            }
            // Dropping the reader releases the claim and attempts zero/shutdown.
        });
        Self {
            commands,
            events,
            worker: Some(worker),
        }
    }
}
impl Drop for Feed {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// Mapping between registered device metres and scene coordinates. The scene origin
/// `(0, -0.3, 0)` is the registered reset pose.
#[derive(Default)]
pub struct RelativeMotion {
    /// Latest registered model position in metres, if registered.
    pub position_m: Option<[f64; 3]>,
}
fn axes(swap_height_depth: bool) -> [usize; 3] {
    if swap_height_depth {
        [0, 2, 1]
    } else {
        [0, 1, 2]
    }
}
fn scene_per_metre(sensitivity: f32) -> f64 {
    f64::from(sensitivity) * 10000.
}
impl RelativeMotion {
    /// Record the latest registered position (`None` while unregistered).
    pub fn sample(&mut self, position_m: Option<[f64; 3]>) {
        self.position_m = position_m;
    }
    /// Scene position of the pen and whether it was clamped to the scene box.
    pub fn position(
        &self,
        sensitivity: f32,
        invert: [bool; 3],
        swap_height_depth: bool,
    ) -> (Vec3, bool) {
        let p = self.position_m.unwrap_or([0.; 3]);
        let axes = axes(swap_height_depth);
        let d = std::array::from_fn::<_, 3, _>(|i| {
            (p[axes[i]] * scene_per_metre(sensitivity)) as f32 * if invert[i] { -1. } else { 1. }
        });
        let p = Vec3::new(d[0], SCENE_ORIGIN_Y + d[1], d[2]);
        let clamped = p.clamp(SCENE_MIN, SCENE_MAX);
        (clamped, (p - clamped).length_squared() > 0.0001)
    }
}
/// Exact inverse of the unclamped [`RelativeMotion::position`] map: scene point to
/// registered device metres.
pub fn device_position(
    scene: Vec3,
    sensitivity: f32,
    invert: [bool; 3],
    swap_height_depth: bool,
) -> [f64; 3] {
    let axes = axes(swap_height_depth);
    let s = [scene.x, scene.y - SCENE_ORIGIN_Y, scene.z];
    let mut p = [0.; 3];
    for i in 0..3 {
        let sign = if invert[i] { -1. } else { 1. };
        p[axes[i]] = f64::from(s[i]) * sign / scene_per_metre(sensitivity);
    }
    p
}
const SCENE_MIN: Vec3 = Vec3::new(-6., -0.3, -5.);
const SCENE_MAX: Vec3 = Vec3::new(6., 8., 5.);
/// The scene clamp box mapped to registered model metres. The map is a signed
/// permutation plus an offset, so mapping both corners and sorting each axis is exact.
pub fn scene_bounds_m(
    sensitivity: f32,
    invert: [bool; 3],
    swap_height_depth: bool,
) -> ([f64; 3], [f64; 3]) {
    let a = device_position(SCENE_MIN, sensitivity, invert, swap_height_depth);
    let b = device_position(SCENE_MAX, sensitivity, invert, swap_height_depth);
    (
        std::array::from_fn(|i| a[i].min(b[i])),
        std::array::from_fn(|i| a[i].max(b[i])),
    )
}
/// Linear part of the same inverse, for a scene velocity (no origin offset).
pub fn device_velocity(
    scene: Vec3,
    sensitivity: f32,
    invert: [bool; 3],
    swap_height_depth: bool,
) -> [f64; 3] {
    let axes = axes(swap_height_depth);
    let s = scene.to_array();
    let mut v = [0.; 3];
    for i in 0..3 {
        let sign = if invert[i] { -1. } else { 1. };
        v[axes[i]] = f64::from(s[i]) * sign / scene_per_metre(sensitivity);
    }
    v
}

pub struct Pointer {
    feed: Option<Feed>,
    motion: RelativeMotion,
    pub status: String,
    pub connected: bool,
    /// The reset pose has been registered on this connection.
    pub registered: bool,
    pub button_held: bool,
    pub feedback: bool,
    pub applied_force: [f64; 3],
    pub telemetry: Telemetry,
    /// Worker-reported reason the latest tick sent zero, if any.
    pub worker_muted: Option<MuteReason>,
    pub muted_ticks: u64,
    /// Continuous counts of all six channels, for the encoder readout.
    pub continuous: [i64; 6],
    encoder_state: visiontouch_device::EncoderState,
    pub connecting: bool,
    pub sensitivity: f32,
    pub invert: [bool; 3],
    pub swap_height_depth: bool,
    /// Device coupling stiffness in model N/m.
    pub stiffness_n_m: f64,
    /// Device coupling damping in model N·s/m.
    pub damping_n_s_m: f64,
    pub position: Vec3,
    pub clamped: bool,
    pub trail: VecDeque<Vec3>,
    pub encoders: [i16; 6],
    pub samples: u64,
    pub last_sample: Option<Instant>,
    pub visible: bool,
    pub show_trail: bool,
}
impl Default for Pointer {
    fn default() -> Self {
        Self {
            feed: None,
            motion: Default::default(),
            status: "Not connected".into(),
            connected: false,
            registered: false,
            button_held: false,
            feedback: false,
            applied_force: [0.; 3],
            telemetry: Telemetry::default(),
            worker_muted: None,
            muted_ticks: 0,
            continuous: [0; 6],
            encoder_state: Default::default(),
            connecting: false,
            sensitivity: 0.004,
            invert: [false; 3],
            swap_height_depth: false,
            stiffness_n_m: DEFAULT_STIFFNESS_N_M,
            damping_n_s_m: DEFAULT_DAMPING_N_S_M,
            position: Vec3::new(0., SCENE_ORIGIN_Y, 0.),
            clamped: false,
            trail: VecDeque::new(),
            encoders: [0; 6],
            samples: 0,
            last_sample: None,
            visible: true,
            show_trail: true,
        }
    }
}
impl Pointer {
    pub fn connect(&mut self) {
        if self.connected || self.connecting {
            return;
        }
        let feed = self.feed.get_or_insert_with(Feed::spawn);
        match feed.commands.send(Command::Connect) {
            Ok(()) => {
                self.connecting = true;
                self.status = "Connecting…".into();
            }
            Err(_) => self.status = "Reader thread stopped; restart the app".into(),
        }
    }
    pub fn set_feedback(&mut self, enabled: bool) {
        if let Some(feed) = &self.feed {
            let _ = feed
                .commands
                .send(Command::Feedback(enabled && self.registered));
        }
    }
    /// Register the current pose as the reset pose. Turns feedback off.
    pub fn zero_at_reset_pose(&mut self) {
        if let Some(feed) = &self.feed
            && self.live()
        {
            let _ = feed.commands.send(Command::RegisterReset);
            self.feedback = false;
            self.trail.clear();
            self.clamped = false;
        }
    }
    /// Send the proxy tether (scene position and velocity), or `None` to mute at once.
    pub fn send_coupling(&self, proxy: Option<(Vec3, Vec3)>) {
        let Some(feed) = &self.feed else { return };
        let coupling = proxy
            .filter(|_| self.feedback && self.registered)
            .map(|(p, v)| Coupling {
                proxy_m: device_position(p, self.sensitivity, self.invert, self.swap_height_depth),
                proxy_velocity_m_s: device_velocity(
                    v,
                    self.sensitivity,
                    self.invert,
                    self.swap_height_depth,
                ),
                bounds_m: scene_bounds_m(self.sensitivity, self.invert, self.swap_height_depth),
                stiffness_n_m: self
                    .stiffness_n_m
                    .clamp(*STIFFNESS_RANGE.start(), *STIFFNESS_RANGE.end()),
                damping_n_s_m: self.damping_n_s_m,
                issued: Instant::now(),
            });
        let _ = feed.commands.send(Command::Coupling(coupling));
    }
    pub fn disconnect(&mut self) {
        if let Some(feed) = &self.feed {
            let _ = feed.commands.send(Command::Disconnect);
        }
        self.connecting = false;
        self.connected = false;
        self.registered = false;
        self.status = "Disconnecting…".into();
        self.last_sample = None;
        self.trail.clear();
    }
    pub fn live(&self) -> bool {
        self.connected
            && self
                .last_sample
                .is_some_and(|t| t.elapsed() < Duration::from_millis(500))
    }
    /// Live and registered: the pointer may drive the scene.
    pub fn active(&self) -> bool {
        self.live() && self.registered
    }
    pub fn poll(&mut self) {
        if let Some(feed) = &self.feed {
            for event in feed.events.try_iter() {
                match event {
                    Event::Connected => {
                        self.connected = true;
                        self.connecting = false;
                        self.registered = false;
                        self.status = "Waiting for motion data".into();
                        self.motion = RelativeMotion::default();
                        self.encoder_state = Default::default();
                        self.samples = 0;
                        self.last_sample = None;
                        self.trail.clear();
                    }
                    Event::Feedback(enabled) => {
                        self.feedback = enabled;
                    }
                    Event::Registered => {
                        self.registered = true;
                        self.trail.clear();
                    }
                    Event::Notice(note) => self.status = note,
                    Event::Sample {
                        sample,
                        time,
                        position_m,
                        telemetry,
                        muted,
                        muted_ticks,
                    } => {
                        self.telemetry = telemetry;
                        self.applied_force = telemetry.applied_force;
                        self.worker_muted = muted;
                        self.muted_ticks = muted_ticks;
                        self.encoder_state.update(sample);
                        self.continuous = self.encoder_state.continuous_counts();
                        self.registered = position_m.is_some();
                        let counts = sample.encoder_counts();
                        self.button_held = sample.stylus_button_pressed();
                        self.encoders = counts;
                        self.motion.sample(position_m);
                        self.samples += 1;
                        self.last_sample = Some(time);
                        self.status = if self.registered {
                            "Live · registered".into()
                        } else {
                            "Live · unregistered: zero at reset pose".into()
                        };
                    }
                    Event::Disconnected => {
                        self.connected = false;
                        self.connecting = false;
                        self.registered = false;
                        self.status = "Disconnected".into();
                        self.last_sample = None;
                    }
                    Event::Error(error) => {
                        self.connected = false;
                        self.connecting = false;
                        self.registered = false;
                        self.last_sample = None;
                        self.status = error;
                        self.trail.clear();
                    }
                }
            }
        }
        if self.connected && !self.live() && self.last_sample.is_some() {
            self.status = "Stale data · pointer stopped".into();
        }
        if !self.live() {
            self.button_held = false;
            self.applied_force = [0.; 3];
            if !self.connected {
                self.feedback = false;
            }
        }
        let (position, clamped) =
            self.motion
                .position(self.sensitivity, self.invert, self.swap_height_depth);
        self.position = position;
        self.clamped = clamped;
        if self.active()
            && self
                .trail
                .back()
                .is_none_or(|last| (*last - position).length() > 0.015)
        {
            self.trail.push_back(position);
            while self.trail.len() > 80 {
                self.trail.pop_front();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_device_round_trip() {
        let (mut seen_clamped, mut seen_free) = (false, false);
        for swap in [false, true] {
            for invert in [[false; 3], [true, false, true], [false, true, false]] {
                for sensitivity in [0.0005_f32, 0.004, 0.02] {
                    let mut m = RelativeMotion::default();
                    for p in [[0.01, -0.02, 0.015], [0., 0., 0.], [-0.03, 0.04, -0.01]] {
                        m.sample(Some(p));
                        let (scene, clamped) = m.position(sensitivity, invert, swap);
                        let back = device_position(scene, sensitivity, invert, swap);
                        let (lo, hi) = scene_bounds_m(sensitivity, invert, swap);
                        for i in 0..3 {
                            // f32 scene coordinates bound the round trip; a clamped
                            // point lands on the mapped scene box instead.
                            if clamped {
                                assert!(back[i] >= lo[i] - 1e-7 && back[i] <= hi[i] + 1e-7);
                            } else {
                                assert!(
                                    (back[i] - p[i]).abs() < 1e-7,
                                    "{swap} {invert:?} {p:?} {back:?}"
                                );
                            }
                        }
                        seen_clamped |= clamped;
                        seen_free |= !clamped;
                    }
                }
            }
        }
        assert!(seen_clamped && seen_free, "both cases are exercised");
        let origin = RelativeMotion::default()
            .position(0.004, [false; 3], false)
            .0;
        assert_eq!(origin, Vec3::new(0., SCENE_ORIGIN_Y, 0.));
        assert_eq!(device_position(origin, 0.004, [false; 3], false), [0.; 3]);
    }

    fn coupling(t: Instant) -> Coupling {
        Coupling {
            proxy_m: [0., -0.003, 0.],
            proxy_velocity_m_s: [0.; 3],
            bounds_m: ([-1.; 3], [1.; 3]),
            stiffness_n_m: 150.,
            damping_n_s_m: 0.,
            issued: t,
        }
    }

    #[test]
    fn pen_below_the_scene_floor_feels_no_spurious_force() {
        // Reset pose maps to the scene floor edge (y = -0.3). A pen 10 mm lower is
        // clamped in the scene, so the proxy sits at the clamped origin.
        let (s, inv) = (0.004_f32, [false; 3]);
        let (lo, hi) = scene_bounds_m(s, inv, false);
        assert!(
            lo[1].abs() < 1e-12,
            "floor edge is the registered origin: {lo:?}"
        );
        let mut m = RelativeMotion::default();
        m.sample(Some([0., -0.01, 0.]));
        let (scene, clamped) = m.position(s, inv, false);
        assert!(clamped);
        let c = Coupling {
            proxy_m: device_position(scene, s, inv, false),
            proxy_velocity_m_s: [0.; 3],
            bounds_m: (lo, hi),
            stiffness_n_m: 150.,
            damping_n_s_m: 1.,
            issued: Instant::now(),
        };
        let f = c.force([0., -0.01, 0.], [0., -0.05, 0.]);
        assert!(f.iter().all(|x| x.abs() < 1e-9), "{f:?}");
        let inside = c.force([0., 0.01, 0.], [0.; 3]);
        assert!(
            inside[1] < 0.,
            "unclamped pen is still pulled to the proxy: {inside:?}"
        );
    }

    #[test]
    fn coupling_expires_after_100_ms() {
        let t = Instant::now();
        let mut servo = Servo {
            coupling: Some(coupling(t)),
            ..Servo::default()
        };
        let at = |ms| t + Duration::from_millis(ms);
        let (d, m) = servo.tick(at(10), Some([0.; 3]), true);
        assert_eq!(m, None);
        assert!(matches!(d, Demand::Force(f) if f[1] < 0.));
        let (_, m) = servo.tick(at(99), Some([0.; 3]), true);
        assert_eq!(m, None);
        let (d, m) = servo.tick(at(100), Some([0.; 3]), true);
        assert_eq!(
            (d, m),
            (Demand::Force([0.; 3]), Some(MuteReason::SceneStalled))
        );
        assert_eq!(servo.muted_ticks, 1);
    }

    #[derive(Default)]
    struct Fake {
        samples: u64,
        requests: Vec<(Demand, Instant)>,
        feedback: bool,
        position: Option<[f64; 3]>,
    }
    impl Link for Fake {
        fn sample(&mut self) -> std::io::Result<Sample> {
            self.samples += 1;
            Ok(Sample { raw: [0; 16] })
        }
        fn position_m(&self) -> Option<[f64; 3]> {
            self.position
        }
        fn feedback_enabled(&self) -> bool {
            self.feedback
        }
        fn request(&mut self, demand: Demand, time: Instant) -> Result<(), RequestError> {
            self.requests.push((demand, time));
            Ok(())
        }
        fn telemetry(&self) -> Telemetry {
            Telemetry::default()
        }
    }

    #[test]
    fn worker_samples_without_message_gaps() {
        let t0 = Instant::now();
        let (tx, rx) = mpsc::channel();
        let mut servo = Servo::default();
        let mut link = Fake {
            feedback: true,
            position: Some([0.; 3]),
            ..Fake::default()
        };
        let mut last_request = t0;
        for tick in 0..1000_u64 {
            let now = t0 + Duration::from_micros(2400 * tick);
            // A UI hammering the channel (far faster than the servo) every tick.
            for _ in 0..50 {
                tx.send(Command::Coupling(Some(coupling(now)))).unwrap();
            }
            assert!(drain(&rx, &mut servo).is_empty());
            let s = service(&mut link, &mut servo, now).unwrap();
            assert_eq!(s.muted, None);
            last_request = now;
        }
        assert_eq!(link.samples, 1000, "exactly one sample per iteration");
        assert_eq!(link.requests.len(), 1000);
        assert_eq!(
            link.requests.last().unwrap().1,
            last_request,
            "stamped on the tick"
        );
        // Commands other than coupling are handed back in order, not dropped.
        tx.send(Command::RegisterReset).unwrap();
        tx.send(Command::Coupling(None)).unwrap();
        tx.send(Command::Stop).unwrap();
        let other = drain(&rx, &mut servo);
        assert!(matches!(other[..], [Command::RegisterReset, Command::Stop]));
        assert!(servo.coupling.is_none());
    }

    #[test]
    fn unregistered_and_feedback_off_send_zero() {
        let t = Instant::now();
        let mut servo = Servo {
            coupling: Some(coupling(t)),
            ..Servo::default()
        };
        assert_eq!(servo.tick(t, None, true).1, Some(MuteReason::Unregistered));
        assert_eq!(
            servo.tick(t, Some([0.; 3]), false).1,
            Some(MuteReason::FeedbackOff)
        );
    }

    #[test]
    fn absent_or_stale_samples_are_not_live() {
        let mut p = Pointer::default();
        assert!(!p.live());
        p.connected = true;
        p.last_sample = Some(Instant::now() - Duration::from_secs(1));
        assert!(!p.live());
        p.last_sample = Some(Instant::now());
        assert!(p.live());
        assert!(
            !p.active(),
            "live but unregistered does not drive the scene"
        );
        p.registered = true;
        assert!(p.active());
    }

    #[test]
    fn reference_model_maps_lift_left_and_away_consistently() {
        let defaults = Pointer::default();
        let mut motion = RelativeMotion::default();
        motion.sample(Some([0.; 3]));
        assert_eq!(
            motion
                .position(defaults.sensitivity, defaults.invert, false)
                .0,
            Vec3::new(0., SCENE_ORIGIN_Y, 0.)
        );
        // Model +Y lifts, model -X is left, model -Z is away.
        motion.sample(Some([-0.005, 0.01, -0.01]));
        let p = motion
            .position(defaults.sensitivity, defaults.invert, false)
            .0;
        assert!(p.x < 0. && p.y > SCENE_ORIGIN_Y && p.z < 0., "{p:?}");
    }
}
