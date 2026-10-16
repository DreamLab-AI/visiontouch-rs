//! Relative encoder-space visualization; docs/spec/11-relative-pointer.md.
use haptics_device::{Device, Wrench};
use haptics_hal::device::Sample;
use macroquad::prelude::Vec3;
use std::collections::VecDeque;
use std::io::Write;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

pub enum Event {
    Connected,
    Sample(Sample, Instant, [f64; 3]),
    Feedback(bool),
    Disconnected,
    Error(String),
}
enum Command {
    Connect,
    Disconnect,
    Stop,
    Feedback(bool),
    Force(Wrench, Instant),
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
            let mut capture = std::env::args()
                .find_map(|arg| arg.strip_prefix("--capture-input=").map(str::to_owned))
                .and_then(|path| std::fs::File::create(path).ok())
                .map(std::io::BufWriter::new);
            let capture_start = Instant::now();
            if let Some(file) = capture.as_mut() {
                let _ = writeln!(file, "elapsed_ms,raw_hex");
            }
            loop {
                if haptics_device::shutdown::requested() {
                    break;
                }
                match rx.recv_timeout(Duration::from_millis(2)) {
                    Ok(Command::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Ok(Command::Feedback(enabled)) => {
                        if let Some(device) = reader.as_mut() {
                            let result = if enabled {
                                device.enable_feedback()
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
                    }
                    Ok(Command::Force(wrench, time)) => {
                        if let Some(device) = reader.as_mut()
                            && let Err(e) = device.request(wrench, time)
                        {
                            reader = None;
                            let _ = tx.send(Event::Error(format!("Force request stopped: {e}")));
                        }
                    }
                    Ok(Command::Disconnect) => {
                        if let Some(device) = reader.take()
                            && let Err(e) = device.close()
                        {
                            let _ = tx.send(Event::Error(format!("Disconnect: {e}")));
                            continue;
                        }
                        if tx.send(Event::Disconnected).is_err() {
                            break;
                        }
                    }
                    Ok(Command::Connect) => {
                        if reader.is_some() {
                            continue;
                        }
                        match Device::connect("/dev/parport0") {
                            Ok(device) => {
                                reader = Some(device);
                                if tx.send(Event::Connected).is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                if tx.send(Event::Error(format!("{e}"))).is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if let Some(device) = reader.as_mut() {
                            match device.sample() {
                                Ok(sample) => {
                                    if let Some(file) = capture.as_mut() {
                                        let hex: String =
                                            sample.raw.iter().map(|b| format!("{b:02x}")).collect();
                                        if writeln!(
                                            file,
                                            "{:.3},{hex}",
                                            capture_start.elapsed().as_secs_f64() * 1000.
                                        )
                                        .and_then(|_| file.flush())
                                        .is_err()
                                        {
                                            eprintln!("Input capture stopped: write failed");
                                            capture = None;
                                        }
                                    }
                                    if tx
                                        .send(Event::Sample(
                                            sample,
                                            Instant::now(),
                                            device.guard.applied_force,
                                        ))
                                        .is_err()
                                    {
                                        break;
                                    }
                                }
                                Err(e) => {
                                    reader = None;
                                    if tx.send(Event::Error(format!("Read stopped: {e}"))).is_err()
                                    {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // Dropping the reader releases the claim; no force packets exist in this API.
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

#[derive(Default)]
pub struct RelativeMotion {
    previous: Option<[i16; 3]>,
    pub counts: [i64; 3],
    origin_counts: [i64; 3],
}
impl RelativeMotion {
    pub fn sample(&mut self, encoders: [i16; 6]) {
        let current = [encoders[0], encoders[1], encoders[2]];
        if let Some(previous) = self.previous {
            for i in 0..3 {
                self.counts[i] += i64::from(current[i].wrapping_sub(previous[i]));
            }
        }
        if self.previous.is_none() {
            self.origin_counts = current.map(i64::from);
        }
        self.previous = Some(current);
    }
    pub fn recenter(&mut self) {
        for i in 0..3 {
            self.origin_counts[i] += self.counts[i];
        }
        self.counts = [0; 3];
    }
    pub fn position(
        &self,
        sensitivity: f32,
        invert: [bool; 3],
        swap_height_depth: bool,
    ) -> (Vec3, bool) {
        let axes = if swap_height_depth {
            [0, 2, 1]
        } else {
            [0, 1, 2]
        };
        let origin = haptics_device::position_from_counts(self.origin_counts.map(|x| x as f64));
        let absolute = haptics_device::position_from_counts(std::array::from_fn(|i| {
            (self.origin_counts[i] + self.counts[i]) as f64
        }));
        let d = std::array::from_fn::<_, 3, _>(|i| {
            ((absolute[axes[i]] - origin[axes[i]]) * f64::from(sensitivity) * 10000.) as f32
                * if invert[i] { -1. } else { 1. }
        });
        let p = Vec3::new(d[0], -0.3 + d[1], d[2]);
        let clamped = p.clamp(Vec3::new(-6., -0.3, -5.), Vec3::new(6., 8., 5.));
        (clamped, (p - clamped).length_squared() > 0.0001)
    }
}

pub struct Pointer {
    feed: Option<Feed>,
    motion: RelativeMotion,
    pub status: String,
    pub connected: bool,
    pub button_held: bool,
    pub feedback: bool,
    pub applied_force: [f64; 3],
    pub relative_encoders: [i64; 6],
    encoder_state: haptics_device::EncoderState,
    pub connecting: bool,
    pub sensitivity: f32,
    pub invert: [bool; 3],
    pub swap_height_depth: bool,
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
            button_held: false,
            feedback: false,
            applied_force: [0.; 3],
            relative_encoders: [0; 6],
            encoder_state: Default::default(),
            connecting: false,
            sensitivity: 0.004,
            invert: [false; 3],
            swap_height_depth: false,
            position: Vec3::new(0., -0.3, 0.),
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
            let _ = feed.commands.send(Command::Feedback(enabled));
        }
    }
    pub fn send_force(&self, force: Vec3) {
        if self.feedback
            && let Some(feed) = &self.feed
        {
            let scene = if self.live() {
                force.to_array()
            } else {
                [0.; 3]
            };
            let axes = if self.swap_height_depth {
                [0, 2, 1]
            } else {
                [0, 1, 2]
            };
            let mut physical = [0_f32; 3];
            for i in 0..3 {
                physical[axes[i]] = scene[i] * if self.invert[i] { -1. } else { 1. };
            }
            let force = Vec3::from_array(physical);
            let _ = feed.commands.send(Command::Force(
                Wrench {
                    force: [force.x as f64, force.y as f64, force.z as f64],
                    torque: [0.; 3],
                },
                Instant::now(),
            ));
        }
    }
    pub fn disconnect(&mut self) {
        if let Some(feed) = &self.feed {
            let _ = feed.commands.send(Command::Disconnect);
        }
        self.connecting = false;
        self.connected = false;
        self.status = "Disconnecting…".into();
        self.last_sample = None;
        self.trail.clear();
    }
    pub fn recenter(&mut self) {
        self.motion.recenter();
        self.encoder_state.recenter();
        self.position = Vec3::new(0., -0.3, 0.);
        self.trail.clear();
        self.clamped = false;
    }
    pub fn live(&self) -> bool {
        self.connected
            && self
                .last_sample
                .is_some_and(|t| t.elapsed() < Duration::from_millis(500))
    }
    pub fn poll(&mut self) {
        if let Some(feed) = &self.feed {
            for event in feed.events.try_iter() {
                match event {
                    Event::Connected => {
                        self.connected = true;
                        self.connecting = false;
                        self.status = "Waiting for motion data".into();
                        self.motion = RelativeMotion::default();
                        self.encoder_state = Default::default();
                        self.samples = 0;
                        self.last_sample = None;
                        self.trail.clear();
                        if std::env::args().any(|a| a == "--force-feedback") {
                            let _ = feed.commands.send(Command::Feedback(true));
                        }
                    }
                    Event::Feedback(enabled) => {
                        self.feedback = enabled;
                    }
                    Event::Sample(sample, time, applied_force) => {
                        self.applied_force = applied_force;
                        self.encoder_state.update(sample);
                        self.relative_encoders = self.encoder_state.relative_counts;
                        let counts = sample.encoder_counts();
                        self.button_held = sample.stylus_button_pressed();
                        self.encoders = counts;
                        self.motion.sample(counts);
                        self.samples += 1;
                        self.last_sample = Some(time);
                        self.status = "Live · relative motion".into();
                    }
                    Event::Disconnected => {
                        self.connected = false;
                        self.connecting = false;
                        self.status = "Disconnected".into();
                        self.last_sample = None;
                    }
                    Event::Error(error) => {
                        self.connected = false;
                        self.connecting = false;
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
        if self.live()
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
    fn first_sample_centers_and_rollover_is_continuous() {
        let mut m = RelativeMotion::default();
        m.sample([32767, -32768, 100, 0, 0, 0]);
        assert_eq!(m.counts, [0, 0, 0]);
        m.sample([-32768, 32767, 110, 0, 0, 0]);
        assert_eq!(m.counts, [1, -1, 10]);
    }
    #[test]
    fn recenter_gain_and_inversion_are_software_only() {
        let mut m = RelativeMotion::default();
        m.sample([0; 6]);
        m.sample([100, 200, -100, 0, 0, 0]);
        let (p, c) = m.position(0.01, [false, true, false], false);
        assert!(c);
        assert_eq!(p.y, -0.3);
        m.recenter();
        assert_eq!(
            m.position(0.01, [false; 3], false),
            (Vec3::new(0., -0.3, 0.), false)
        );
        m.sample([101, 201, -99, 0, 0, 0]);
        assert_eq!(m.counts, [1, 1, 1]);
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
    }

    #[test]
    fn reference_model_maps_lift_left_and_away_consistently() {
        let defaults = Pointer::default();
        let mut motion = RelativeMotion::default();
        motion.sample([0; 6]);
        assert_eq!(
            motion
                .position(defaults.sensitivity, defaults.invert, false)
                .0,
            Vec3::new(0., -0.3, 0.)
        );
        motion.sample([100, 100, -600, 0, 0, 0]);
        let p = motion
            .position(defaults.sensitivity, defaults.invert, false)
            .0;
        assert!(p.x < 0. && p.y > 0. && p.z < 0., "{p:?}");
        motion.recenter();
        assert_eq!(
            motion
                .position(defaults.sensitivity, defaults.invert, false)
                .0,
            Vec3::new(0., -0.3, 0.)
        );
    }
}
