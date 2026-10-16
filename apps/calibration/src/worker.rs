//! Dedicated servo worker: UI supplies intent, never individual motor packets.
//! Output is explicitly armed per held attempt, independently timed, and disarmed
//! before persistence. Snapshots are lossy and bounded so graphics cannot block I/O.
use crate::model::*;
use crate::sweep::Sweep;
use crate::teaching::{self, Stage, Teaching};
use haptics_device::{Device, Wrench};
use haptics_effects::{Effect, Frame, Kind};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::io::Write;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc,
};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const DIRECTORY: &str = "captures/calibration";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Test {
    Zero,
    Object(Kind),
}
impl Test {
    pub fn name(self) -> &'static str {
        match self {
            Self::Zero => "Move to zero",
            Self::Object(k) => k.name(),
        }
    }
}
pub enum Command {
    Connect,
    Disconnect,
    Bottom,
    Capture(usize),
    Sweep(bool),
    SweepZero,
    ReviewSweepPath,
    TeachApproach,
    FinishTeaching,
    SaveCalibration(Volume),
    Start(Volume, Settings, Test),
    Stop,
    Quit,
}
#[derive(Default)]
pub struct Controls {
    pub held: AtomicBool,
    pub pulse: AtomicU64,
}
#[derive(Clone)]
pub struct Snapshot {
    pub connected: bool,
    pub reference: bool,
    pub stable: bool,
    pub active: bool,
    pub measured: [Option<f64>; 6],
    pub sweep: Arc<Sweep>,
    pub sweep_mode: bool,
    pub sweeping: bool,
    pub teaching: bool,
    pub teaching_instruction: String,
    pub route_status: String,
    pub return_route: Arc<Vec<V>>,
    pub approach_ready: bool,
    pub approach_start: Option<V>,
    pub approach_remaining_s: u64,
    pub current_supported: bool,
    pub position: V,
    pub velocity: V,
    pub counts: [i16; 6],
    pub accumulated: [i64; 6],
    pub samples: u64,
    pub force: V,
    pub requested: V,
    pub offsets: [i16; 3],
    pub target: V,
    pub frame: Frame,
    pub status: String,
    pub log: Option<String>,
    pub rate: f64,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            connected: false,
            reference: false,
            stable: false,
            active: false,
            measured: [None; 6],
            sweep: Arc::new(Sweep::default()),
            sweep_mode: false,
            sweeping: false,
            teaching: false,
            teaching_instruction: String::new(),
            route_status: String::new(),
            return_route: Arc::new(Vec::new()),
            approach_ready: false,
            approach_start: None,
            approach_remaining_s: 0,
            current_supported: false,
            position: [0.; 3],
            velocity: [0.; 3],
            counts: [0; 6],
            accumulated: [0; 6],
            samples: 0,
            force: [0.; 3],
            requested: [0.; 3],
            offsets: [0; 3],
            target: [0.; 3],
            frame: Frame::default(),
            status: "Connect the device, then capture the bottom reference. Motor output is off."
                .into(),
            log: None,
            rate: 0.,
        }
    }
}
pub struct Worker {
    pub controls: Arc<Controls>,
    pub commands: mpsc::Sender<Command>,
    pub snapshots: mpsc::Receiver<Snapshot>,
    join: Option<thread::JoinHandle<()>>,
}
impl Worker {
    pub fn spawn() -> Self {
        let (commands, rx) = mpsc::channel();
        let (tx, snapshots) = mpsc::sync_channel(1);
        let controls = Arc::new(Controls::default());
        let shared = controls.clone();
        let join = thread::spawn(move || run(rx, tx, shared));
        Self {
            controls,
            commands,
            snapshots,
            join: Some(join),
        }
    }
    pub fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.controls.held.store(false, Ordering::Relaxed);
        self.send(Command::Quit);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

#[derive(Serialize)]
struct Row {
    seconds: f64,
    interval_us: u128,
    sample_io_us: u128,
    command_limited: bool,
    counts: [i16; 6],
    accumulated: [i64; 6],
    model: V,
    working: V,
    target: V,
    requested: V,
    next_request: V,
    guarded: V,
    offsets: [i16; 3],
    status: u8,
    sequence: u8,
    popped: bool,
    deformation: f64,
}
/// Exists only after a complete live motor-off teaching trace. Never deserialized.
struct Approach {
    evidence: Arc<teaching::Evidence>,
    cloud: Arc<haptics_workspace::EvidenceCloud>,
    confirmed: Instant,
}
impl Approach {
    fn from_teaching(
        evidence: teaching::Evidence,
        sweep: &Sweep,
        now: Instant,
    ) -> Result<Self, String> {
        let mut points = sweep.points_model_m.clone();
        points.extend(evidence.observations.iter().map(|r| [r[1], r[2], r[3]]));
        points.extend([evidence.start_model_m, evidence.zero_model_m]);
        let cloud = haptics_workspace::EvidenceCloud::new(points, Default::default())
            .map_err(|e| e.to_string())?;
        if evidence.return_route_model_m.first() != Some(&evidence.start_model_m)
            || evidence.return_route_model_m.last() != Some(&evidence.zero_model_m)
            || evidence.return_route_model_m.len() < 2
        {
            return Err("Recorded route is empty.".into());
        }
        // Each pair is supported by its own explicitly observed endpoints. This
        // is linear in route length and never substitutes a start-to-zero chord.
        for pair in evidence.return_route_model_m.windows(2) {
            let local = haptics_workspace::EvidenceCloud::new(pair.to_vec(), Default::default())
                .map_err(|e| e.to_string())?;
            if local
                .review_segment(pair[0], pair[1])
                .map_err(|e| e.to_string())?
                != haptics_workspace::SegmentReview::Supported
            {
                return Err("Recorded route has an unsupported gap; record again.".into());
            }
        }
        Ok(Self {
            evidence: Arc::new(evidence),
            cloud: Arc::new(cloud),
            confirmed: now,
        })
    }
    fn validate_start(
        &self,
        now: Instant,
        p: V,
        zero: V,
        test: Test,
        force: f64,
    ) -> Result<(), &'static str> {
        if now.saturating_duration_since(self.confirmed) >= teaching::MAX_AGE {
            return Err("Manual route confirmation expired; teach it again.");
        }
        if !force.is_finite() || force <= 0. || force > teaching::PILOT_FORCE_N {
            return Err("Swept-map pilot attempts are capped at 0.1 model N.");
        }
        if norm(sub(zero, self.evidence.zero_model_m)) > 1e-9 {
            return Err("Working zero changed; teach the route again.");
        }
        if !self.cloud.contains(p) {
            return Err("Return to the sample-supported region before starting.");
        }
        if test == Test::Zero && norm(sub(p, self.evidence.start_model_m)) > 0.00075 {
            return Err("Return within 0.75 model mm of the taught starting pose.");
        }
        Ok(())
    }
}
fn invalidate_approach(
    trainer: &mut Option<Teaching>,
    approach: &mut Option<Approach>,
    s: &mut Snapshot,
) {
    *trainer = None;
    *approach = None;
    s.teaching = false;
    s.teaching_instruction.clear();
    s.return_route = Arc::new(Vec::new());
    s.approach_ready = false;
    s.approach_start = None;
    s.approach_remaining_s = 0;
    s.current_supported = false;
}

struct Attempt {
    volume: Volume,
    settings: Settings,
    test: Test,
    start: Instant,
    started_unix_ns: String,
    planner: Move,
    effect: Effect,
    rows: Vec<Row>,
    reference: V,
    reference_counts: [i16; 6],
    support: Option<Arc<haptics_workspace::EvidenceCloud>>,
    approach_evidence: Option<Arc<teaching::Evidence>>,
    authorization_expires: Option<Instant>,
    effect_radius_m: f64,
}
impl Attempt {
    fn save(self, reason: &str) -> std::io::Result<String> {
        std::fs::create_dir_all(DIRECTORY)?;
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let prefix = format!("{DIRECTORY}/attempt-{id}");
        let metadata = serde_json::json!({"schema":1,"test":self.test,"settings":self.settings,
            "volume":self.volume,"unit_derating":haptics_device::UNIT_DERATING,"reference_model_m":self.reference,"reference_counts":self.reference_counts,
            "approach_evidence":self.approach_evidence.as_deref(),"effect_radius_m":self.effect_radius_m,
            "result":reason,"samples":self.rows.len(),"started_unix_ns":self.started_unix_ns,"force_units":"model newtons, not measured force"});
        std::fs::write(
            format!("{prefix}.json"),
            serde_json::to_vec_pretty(&metadata)?,
        )?;
        let mut file = std::io::BufWriter::new(std::fs::File::create(format!("{prefix}.csv"))?);
        write!(file, "seconds")?;
        for group in ["raw", "accumulated"] {
            for i in 0..6 {
                write!(file, ",{group}_{i}")?;
            }
        }
        for group in [
            "model_m",
            "working_mm",
            "target_m",
            "requested_N",
            "next_request_N",
            "guarded_N",
        ] {
            for axis in ["x", "y", "z"] {
                write!(file, ",{group}_{axis}")?;
            }
        }
        writeln!(
            file,
            ",motor_0,motor_1,motor_2,status,sequence,popped,deformation,interval_us,sample_io_us,command_limited"
        )?;
        for row in self.rows {
            write!(file, "{:.6}", row.seconds)?;
            for c in row.counts {
                write!(file, ",{c}")?;
            }
            for c in row.accumulated {
                write!(file, ",{c}")?;
            }
            for vector in [
                row.model,
                row.working,
                row.target,
                row.requested,
                row.next_request,
                row.guarded,
            ] {
                for x in vector {
                    write!(file, ",{x:.8}")?;
                }
            }
            for c in row.offsets {
                write!(file, ",{c}")?;
            }
            writeln!(
                file,
                ",{},{},{},{:.6},{},{},{}",
                row.status,
                row.sequence,
                u8::from(row.popped),
                row.deformation,
                row.interval_us,
                row.sample_io_us,
                u8::from(row.command_limited)
            )?;
        }
        file.flush()?;
        Ok(format!("{prefix}.csv"))
    }
}

/// Every attempt passes this check before the device can be armed. Sweeps are
/// observational evidence and cannot be substituted for an approved force map.
fn validate_attempt_mapping(s: &Snapshot, volume: &Volume) -> Result<(), &'static str> {
    if s.sweeping || s.teaching || (s.sweep_mode && !s.approach_ready) {
        return Err("Finish motor-off approach teaching before a swept-map pilot attempt.");
    }
    if !s.reference || volume.measured != s.measured {
        return Err("Capture a fresh bottom reference and all six bounds.");
    }
    volume.validate()
}

/// Reject a held attempt when operator/input timing or measured travel is invalid.
fn interlock(
    held: bool,
    ui_age: Duration,
    dt: Duration,
    button: bool,
    p: V,
    v: V,
    volume: &Volume,
) -> Result<(), &'static str> {
    if !held {
        return Err("Hold released; output stopped.");
    }
    if ui_age > Duration::from_millis(250) {
        return Err("UI heartbeat lost; output stopped.");
    }
    if dt > haptics_device::COMMAND_TIMEOUT {
        return Err("Servo interval exceeded 25 ms; output stopped.");
    }
    if button {
        return Err("Pen button pressed; output stopped.");
    }
    if !volume.contains(p, 0.002) {
        return Err("Pen left the recorded volume; output stopped.");
    }
    if !norm(v).is_finite() || norm(v) > 0.03 {
        return Err("Pen speed exceeded 30 model mm/s; output stopped.");
    }
    Ok(())
}
fn stop(
    device: &mut Option<Device>,
    attempt: &mut Option<Attempt>,
    s: &mut Snapshot,
    reason: &str,
) {
    s.status = reason.into();
    s.active = false;
    s.sweeping = false;
    s.force = [0.; 3];
    s.requested = [0.; 3];
    s.offsets = [0; 3];
    if let Some(attempt) = attempt.take() {
        if let Some(d) = device.as_mut()
            && let Err(error) = d.disable_feedback()
        {
            s.status = format!("{reason} Zero/shutdown error: {error}");
            *device = None;
            s.connected = false;
            s.reference = false;
        }
        match attempt.save(&s.status) {
            Ok(path) => s.log = Some(path),
            Err(error) => s.status.push_str(&format!(" Log save failed: {error}")),
        }
        eprintln!("{}", s.status);
    }
}
fn stable(history: &VecDeque<(Instant, V)>) -> Option<V> {
    let (first, last) = (history.front()?, history.back()?);
    if last.0.duration_since(first.0) < Duration::from_millis(250) {
        return None;
    }
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    let mut sum = [0.; 3];
    for (_, p) in history {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
            sum[i] += p[i];
        }
    }
    if (0..3).any(|i| hi[i] - lo[i] > 0.0005) {
        return None;
    }
    Some(mul(sum, 1. / history.len() as f64))
}
fn run(rx: mpsc::Receiver<Command>, tx: mpsc::SyncSender<Snapshot>, controls: Arc<Controls>) {
    let mut device: Option<Device> = None;
    let mut attempt: Option<Attempt> = None;
    let mut trainer: Option<Teaching> = None;
    let mut finish_teaching = false;
    let mut approach: Option<Approach> = None;
    let mut s = Snapshot::default();
    let mut reference = [0.; 3];
    let mut reference_counts = [0; 6];
    let mut history = VecDeque::new();
    let mut previous: Option<(Instant, V)> = None;
    let mut last_pulse = 0;
    let mut ui_time = Instant::now();
    let mut publish = Instant::now();
    let mut quitting = false;
    while !quitting && !haptics_device::shutdown::requested() {
        let now = Instant::now();
        let pulse = controls.pulse.load(Ordering::Relaxed);
        if pulse != last_pulse {
            last_pulse = pulse;
            ui_time = now;
        }
        if approach
            .as_ref()
            .is_some_and(|a| now.saturating_duration_since(a.confirmed) >= teaching::MAX_AGE)
        {
            invalidate_approach(&mut trainer, &mut approach, &mut s);
            s.status =
                "Manual route expired; teach it again before another swept-map attempt.".into();
        }
        for command in rx.try_iter() {
            if !s.active
                && matches!(
                    &command,
                    Command::Connect
                        | Command::Disconnect
                        | Command::Bottom
                        | Command::Capture(_)
                        | Command::Sweep(_)
                        | Command::SweepZero
                        | Command::ReviewSweepPath
                        | Command::TeachApproach
                )
            {
                invalidate_approach(&mut trainer, &mut approach, &mut s);
            }
            if matches!(&command, Command::Stop | Command::Quit) {
                if trainer.is_some() {
                    s.route_status = "Route recording cancelled by operator.".into();
                }
                trainer = None;
                finish_teaching = false;
                s.teaching = false;
                s.teaching_instruction.clear();
            }
            match command {
                Command::Quit => {
                    quitting = true;
                    break;
                }
                Command::Stop => stop(
                    &mut device,
                    &mut attempt,
                    &mut s,
                    "Stopped by operator; output off.",
                ),
                Command::Disconnect => {
                    invalidate_approach(&mut trainer, &mut approach, &mut s);
                    stop(
                        &mut device,
                        &mut attempt,
                        &mut s,
                        "Disconnected; output off.",
                    );
                    if let Some(d) = device.take()
                        && let Err(e) = d.close()
                    {
                        s.status = format!("Close error: {e}");
                    }
                    s.connected = false;
                    s.reference = false;
                    s.stable = false;
                    s.measured = [None; 6];
                    s.sweeping = false;
                    s.sweep_mode = false;
                    s.sweep = Arc::new(Sweep::default());
                    previous = None;
                    history.clear();
                }
                Command::Connect if device.is_none() => {
                    match Device::connect("/dev/parport0").and_then(|mut d| {
                        d.sample()?;
                        Ok(d)
                    }) {
                        Ok(d) => {
                            device = Some(d);
                            s.connected = true;
                            s.reference = false;
                            s.measured = [None; 6];
                            s.sweeping = false;
                            s.sweep_mode = false;
                            s.sweep = Arc::new(Sweep::default());
                            s.samples = 0;
                            previous = None;
                            history.clear();
                            s.status="Connected · output OFF. Rest the pen at bottom/centre and capture the bottom reference.".into();
                        }
                        Err(e) => s.status = format!("Connection failed: {e}"),
                    }
                }
                Command::Bottom if s.connected && !s.active => {
                    if let Some(p) = stable(&history) {
                        s.sweeping = false;
                        s.sweep_mode = false;
                        s.sweep = Arc::new(Sweep::default());
                        reference = p;
                        reference_counts = s.counts;
                        s.reference = true;
                        s.measured = [None, None, Some(0.), None, None, None];
                        s.status="Bottom recorded. Enter working bounds and record left, right, top, back and front with motors off.".into();
                    } else {
                        s.status = "Hold the pen still for 0.3 seconds before recording.".into();
                    }
                }
                Command::Sweep(recording) if s.connected && s.reference && !s.active => {
                    s.sweep_mode = true;
                    s.sweeping = recording;
                    if recording && s.sweep.points_model_m.is_empty() {
                        Arc::make_mut(&mut s.sweep).record([0.; 3]);
                    }
                    s.status = if recording {"Recording comfortable sweep; motors OFF. Cover multiple heights and depths."} else {"Sweep stopped; motors OFF. Choose a comfortable interior zero, then export."}.into();
                }
                Command::SweepZero if s.reference && !s.active && !s.sweeping && s.sweep_mode => {
                    s.status = match stable(&history) {
                        Some(p) => match Arc::make_mut(&mut s.sweep).choose_zero(sub(p, reference))
                        {
                            Ok(()) => "Working zero recorded manually; motors OFF.".into(),
                            Err(e) => e.into(),
                        },
                        None => "Hold still for 0.3 seconds before recording zero.".into(),
                    };
                }
                Command::ReviewSweepPath
                    if s.reference && !s.active && !s.sweeping && s.sweep_mode =>
                {
                    s.status = match stable(&history) {
                        Some(p) => match Arc::make_mut(&mut s.sweep).review_path(sub(p,reference)) {
                            Ok(()) => "Path evidence reviewed; motor output remains locked for sweep maps.".into(),
                            Err(e) => e,
                        },
                        None => "Hold still for 0.3 seconds to review this starting pose.".into(),
                    };
                }
                Command::TeachApproach
                    if s.connected && s.reference && !s.active && !s.sweeping && s.sweep_mode =>
                {
                    let result = (|| -> Result<Teaching, &'static str> {
                        if stable(&history).is_none() {
                            return Err("Hold still before starting motor-off teaching.");
                        }
                        if device.as_ref().is_some_and(|d| d.state.button_pressed) {
                            return Err("Release the pen button before teaching; it acts as Stop.");
                        }
                        if ui_time.elapsed() > Duration::from_millis(250) {
                            return Err("Live UI required for teaching.");
                        }
                        Arc::make_mut(&mut s.sweep).choose_zero(s.position)?;
                        Teaching::new(s.position)
                    })();
                    match result {
                        Ok(t) => {
                            finish_teaching = false;
                            s.teaching = true;
                            s.teaching_instruction = t.stage.instruction().into();
                            s.status = s.teaching_instruction.clone();
                            s.route_status = s.status.clone();
                            trainer = Some(t);
                        }
                        Err(e) => {
                            s.status = e.into();
                            s.route_status = s.status.clone();
                        }
                    }
                }
                Command::FinishTeaching if s.teaching && !s.active => {
                    finish_teaching = true;
                }
                Command::Capture(index) if s.reference && !s.active && index < 6 => {
                    if let Some(p) = stable(&history) {
                        s.measured[index] = Some(p[index / 2] - reference[index / 2]);
                        s.status = format!("{} recorded. Motor output remains off.", NAMES[index]);
                    } else {
                        s.status = "Hold the pen still for 0.3 seconds before recording.".into();
                    }
                }
                Command::SaveCalibration(volume) if s.connected && s.reference && !s.active => {
                    let result = (|| -> Result<String, String> {
                        if !s.sweep_mode {
                            volume.validate().map_err(str::to_owned)?;
                        }
                        if volume.measured != s.measured {
                            return Err("Capture state changed; export again.".into());
                        }
                        let stamp = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map_err(|e| e.to_string())?
                            .as_nanos();
                        std::fs::create_dir_all(DIRECTORY).map_err(|e| e.to_string())?;
                        let path = format!("{DIRECTORY}/calibration-{stamp}.json");
                        let evidence = serde_json::json!({
                            "schema":1, "unix_ns":stamp.to_string(),
                            "kind":"input-only calibration evidence", "volume":volume,
                            "reference_model_m":reference, "reference_counts":reference_counts,
                            "restores_live_calibration":false,
                            "sweep":s.sweep.as_ref(), "sweep_mode":s.sweep_mode,
                            "workspace_verified_for_force":false,
                            "manual_approach_evidence":approach.as_ref().map(|a|a.evidence.as_ref()),
                            "live_manual_route_confirmed":approach.is_some(),
                            "route_status":s.route_status,
                            "sample_cell_m":crate::sweep::CELL_M
                        });
                        std::fs::write(
                            &path,
                            serde_json::to_vec_pretty(&evidence).map_err(|e| e.to_string())?,
                        )
                        .map_err(|e| e.to_string())?;
                        Ok(format!("Calibration evidence saved: {path}"))
                    })();
                    s.status = result.unwrap_or_else(|e| format!("Calibration export failed: {e}"));
                }
                Command::Start(volume, settings, test) if s.connected && !s.active => {
                    let result = (|| -> Result<Attempt, String> {
                        validate_attempt_mapping(&s, &volume).map_err(str::to_owned)?;
                        if !settings.valid() {
                            return Err("Invalid speed or force limit.".into());
                        }
                        if stable(&history).is_none() {
                            return Err("Hold the pen still before starting the test.".into());
                        }
                        if !controls.held.load(Ordering::Relaxed)
                            || ui_time.elapsed() > Duration::from_millis(250)
                        {
                            return Err("Hold Space or the move button to start.".into());
                        }
                        let d = device.as_mut().ok_or("Device disconnected.")?;
                        if d.state.button_pressed {
                            return Err(
                                "Release the pen button before starting; it acts as Stop.".into()
                            );
                        }
                        if !volume.contains(s.position, if s.sweep_mode { 0.002 } else { 0. }) {
                            return Err(
                                "Return the pen inside the recorded volume before starting.".into(),
                            );
                        }
                        let goal = volume.zero().unwrap();
                        if s.sweep_mode {
                            approach
                                .as_ref()
                                .ok_or("No live taught route; saved evidence cannot arm output.")?
                                .validate_start(
                                    Instant::now(),
                                    s.position,
                                    goal,
                                    test,
                                    settings.force_n,
                                )
                                .map_err(str::to_owned)?;
                        }
                        let effect_radius_m = if s.sweep_mode {
                            volume.radius().min(0.02)
                        } else {
                            volume.radius()
                        };
                        let effect = Effect::new(
                            match test {
                                Test::Object(k) => k,
                                Test::Zero => Kind::Cube,
                            },
                            effect_radius_m,
                        );
                        let planner = if s.sweep_mode && test == Test::Zero {
                            Move::from_route(
                                approach
                                    .as_ref()
                                    .unwrap()
                                    .evidence
                                    .return_route_model_m
                                    .clone(),
                                settings,
                            )
                            .map_err(str::to_owned)?
                        } else {
                            Move::new(s.position, goal, settings)
                        };
                        d.guard
                            .set_limits(haptics_device::ForceLimits {
                                force_n: settings.force_n,
                                release_after: Some(Duration::from_millis(250)),
                                ..Default::default()
                            })
                            .map_err(str::to_owned)?;
                        d.enable_feedback().map_err(|e| e.to_string())?;
                        Ok(Attempt {
                            planner,
                            effect,
                            volume,
                            settings,
                            test,
                            start: Instant::now(),
                            started_unix_ns: SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_nanos()
                                .to_string(),
                            rows: Vec::with_capacity(90000),
                            reference,
                            reference_counts,
                            support: approach.as_ref().map(|a| Arc::clone(&a.cloud)),
                            approach_evidence: approach.as_ref().map(|a| Arc::clone(&a.evidence)),
                            authorization_expires: approach
                                .as_ref()
                                .map(|a| a.confirmed + teaching::MAX_AGE),
                            effect_radius_m,
                        })
                    })();
                    match result {
                        Ok(a) => {
                            attempt = Some(a);
                            s.active = true;
                            previous = None;
                            s.status = format!(
                                "{} · HOLD to continue. Release / Escape / pen button stops.",
                                test.name()
                            );
                        }
                        Err(e) => {
                            s.status = e;
                        }
                    }
                }
                _ => {}
            }
        }
        if quitting {
            break;
        }
        if let Some(d) = device.as_mut() {
            // sample() sends the previous request after reading fresh encoders.
            // Keep that demand/target beside its guarded command in the log;
            // the newly evaluated demand is explicitly the next servo request.
            let sent_request = s.requested;
            let sent_target = s.target;
            let sample_started = Instant::now();
            match d.sample() {
                Ok(packet) => {
                    let now = Instant::now();
                    s.samples += 1;
                    s.counts = d.state.counts;
                    s.accumulated = d.state.relative_counts;
                    let absolute = d.state.position_m();
                    let dt = previous
                        .map(|(t, _)| now.duration_since(t))
                        .unwrap_or(Duration::from_millis(2));
                    if let Some((_, p)) = previous {
                        let raw = mul(sub(absolute, p), 1. / dt.as_secs_f64().max(1e-6));
                        s.velocity = add(
                            s.velocity,
                            mul(
                                sub(raw, s.velocity),
                                dt.as_secs_f64() / (0.02 + dt.as_secs_f64()),
                            ),
                        );
                    } else {
                        s.velocity = [0.; 3];
                    }
                    previous = Some((now, absolute));
                    s.rate = 0.95 * s.rate + 0.05 / dt.as_secs_f64().max(1e-6);
                    history.push_back((now, absolute));
                    while history
                        .front()
                        .is_some_and(|(t, _)| now.duration_since(*t) > Duration::from_millis(400))
                    {
                        history.pop_front();
                    }
                    s.stable = stable(&history).is_some();
                    s.position = if s.reference {
                        sub(absolute, reference)
                    } else {
                        [0.; 3]
                    };
                    if s.sweeping {
                        Arc::make_mut(&mut s.sweep).record(s.position);
                        if let Some(extrema) = s.sweep.extrema() {
                            s.measured = extrema;
                        }
                        if s.sweep.points_model_m.len() >= crate::sweep::MAX_POINTS {
                            s.sweeping = false;
                            s.status =
                                "Sweep sample limit reached. Export this scan; motors remain OFF."
                                    .into();
                        }
                    }
                    if let Some(t) = trainer.as_mut() {
                        let result = if ui_time.elapsed() > Duration::from_millis(250)
                            || d.state.button_pressed
                        {
                            Err("Teaching stopped by UI timeout or pen button.")
                        } else {
                            t.sample(s.position, dt.as_secs_f64())
                        };
                        let result = result.map(|stage| {
                            if finish_teaching {
                                finish_teaching = false;
                                match t.finish(s.stable) {
                                    Ok(()) => return Stage::Complete,
                                    Err(e) => {
                                        s.status = e.into();
                                        s.route_status = s.status.clone();
                                    }
                                }
                            }
                            stage
                        });
                        match result {
                            Ok(Stage::Complete) => {
                                let evidence =
                                    t.evidence().expect("complete live trace has evidence");
                                match Approach::from_teaching(evidence, &s.sweep, now) {
                                    Ok(a) => {
                                        for r in &a.evidence.observations {
                                            Arc::make_mut(&mut s.sweep).record([r[1], r[2], r[3]]);
                                        }
                                        if let Some(e) = s.sweep.extrema() {
                                            s.measured = e;
                                        }
                                        Arc::make_mut(&mut s.sweep).path_review = None;
                                        s.return_route =
                                            Arc::new(a.evidence.return_route_model_m.clone());
                                        s.status = Stage::Complete.instruction().into();
                                        s.route_status = s.status.clone();
                                        approach = Some(a);
                                    }
                                    Err(e) => {
                                        s.status = e;
                                        s.route_status = s.status.clone();
                                    }
                                }
                                trainer = None;
                                s.teaching = false;
                                s.teaching_instruction.clear();
                            }
                            Ok(stage) => {
                                s.teaching_instruction = stage.instruction().into();
                                // Keep a premature Finish explanation visible until the next action.
                            }
                            Err(e) => {
                                trainer = None;
                                s.teaching = false;
                                s.teaching_instruction.clear();
                                s.status = e.into();
                                s.route_status = s.status.clone();
                                finish_teaching = false;
                            }
                        }
                    }
                    if let Some(a) = approach.as_ref() {
                        s.approach_ready = true;
                        s.approach_start = Some(a.evidence.start_model_m);
                        s.approach_remaining_s = teaching::MAX_AGE
                            .saturating_sub(now.saturating_duration_since(a.confirmed))
                            .as_secs();
                        s.current_supported = a.cloud.contains(s.position);
                    }
                    s.force = d.guard.applied_force;
                    s.offsets = d.guard.applied_motor_offsets;
                    let mut reason = None;
                    if let Some(a) = attempt.as_mut() {
                        let interlocks = interlock(
                            controls.held.load(Ordering::Relaxed),
                            ui_time.elapsed(),
                            dt,
                            d.state.button_pressed,
                            s.position,
                            s.velocity,
                            &a.volume,
                        );
                        let force = interlocks.and_then(|()| {
                            if a.authorization_expires
                                .is_some_and(|deadline| now >= deadline)
                            {
                                return Err("Manual route confirmation expired; output stopped.");
                            }
                            if a.support
                                .as_ref()
                                .is_some_and(|cloud| !cloud.contains(s.position))
                            {
                                return Err(
                                    "Pen left the sample-supported region; output stopped.",
                                );
                            }
                            if a.start.elapsed() > Duration::from_secs(180) {
                                return Err("Attempt timed out; output stopped.");
                            }
                            match a.test {
                                Test::Zero => {
                                    a.planner.tick(s.position, s.velocity, dt.as_secs_f64())?;
                                    s.target = a.planner.target;
                                    Ok(a.planner.demand)
                                }
                                Test::Object(_) => {
                                    let centre = a.volume.zero().unwrap();
                                    s.frame = a.effect.step(
                                        sub(s.position, centre),
                                        s.velocity,
                                        dt.as_secs_f64(),
                                    );
                                    s.target = add(centre, s.frame.proxy);
                                    Ok(s.frame.force)
                                }
                            }
                        });
                        match force {
                            Ok(force) => {
                                s.requested = force;
                                if let Err(error) = d.request(
                                    Wrench {
                                        force,
                                        torque: [0.; 3],
                                    },
                                    now,
                                ) {
                                    reason = Some(error.to_string());
                                }
                            }
                            Err(error) => reason = Some(error.into()),
                        }
                        a.rows.push(Row {
                            seconds: now.duration_since(a.start).as_secs_f64(),
                            interval_us: dt.as_micros(),
                            sample_io_us: now.duration_since(sample_started).as_micros(),
                            command_limited: d.guard.command_limited,
                            counts: s.counts,
                            accumulated: s.accumulated,
                            model: s.position,
                            working: a.volume.working(s.position).unwrap(),
                            target: sent_target,
                            requested: sent_request,
                            next_request: s.requested,
                            guarded: s.force,
                            offsets: s.offsets,
                            status: packet.raw[13],
                            sequence: packet.raw[14],
                            popped: s.frame.popped,
                            deformation: s.frame.deformation,
                        });
                    }
                    if let Some(reason) = reason {
                        stop(&mut device, &mut attempt, &mut s, &reason);
                    }
                }
                Err(error) => {
                    let force_fault = d.take_force_fault();
                    let reason = if force_fault.is_some() {
                        format!(
                            "Command limit reached: {error}. Release hold before a new attempt."
                        )
                    } else {
                        format!("Device I/O fault: {error}")
                    };
                    stop(&mut device, &mut attempt, &mut s, &reason);
                    if force_fault.is_none() {
                        invalidate_approach(&mut trainer, &mut approach, &mut s);
                        device = None;
                        s.connected = false;
                        s.reference = false;
                        s.stable = false;
                        s.measured = [None; 6];
                        s.sweeping = false;
                        s.sweep_mode = false;
                        s.sweep = Arc::new(Sweep::default());
                        history.clear();
                    }
                }
            }
        }
        if publish.elapsed() >= Duration::from_millis(12) {
            let _ = tx.try_send(s.clone());
            publish = Instant::now();
        }
        thread::sleep(Duration::from_millis(if s.connected { 2 } else { 10 }));
    }
    stop(
        &mut device,
        &mut attempt,
        &mut s,
        "Application closing; output stopped.",
    );
    if let Some(d) = device
        && let Err(e) = d.close()
    {
        eprintln!("Calibration close: {e}");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn sweep_and_stale_mapping_cannot_authorize_force() {
        let volume = crate::model::tests::volume();
        let mut snapshot = super::Snapshot {
            reference: true,
            measured: volume.measured,
            ..Default::default()
        };
        assert!(super::validate_attempt_mapping(&snapshot, &volume).is_ok());
        snapshot.sweep_mode = true;
        assert!(super::validate_attempt_mapping(&snapshot, &volume).is_err());
        snapshot.sweep_mode = false;
        snapshot.sweeping = true;
        assert!(super::validate_attempt_mapping(&snapshot, &volume).is_err());
        snapshot.sweeping = false;
        snapshot.measured[0] = Some(-1.);
        assert!(super::validate_attempt_mapping(&snapshot, &volume).is_err());
        snapshot.measured = volume.measured;
        snapshot.reference = false;
        assert!(super::validate_attempt_mapping(&snapshot, &volume).is_err());
    }

    use super::*;
    fn volume() -> Volume {
        Volume {
            bounds: [[-1., 1.]; 3],
            measured: [
                Some(-0.05),
                Some(0.05),
                Some(-0.05),
                Some(0.05),
                Some(-0.05),
                Some(0.05),
            ],
        }
    }
    #[test]
    fn hold_ui_sample_button_volume_and_speed_faults_stop_output() {
        let v = volume();
        let ms = Duration::from_millis;
        assert!(interlock(true, ms(20), ms(2), false, [0.; 3], [0.; 3], &v).is_ok());
        for result in [
            interlock(false, ms(0), ms(2), false, [0.; 3], [0.; 3], &v),
            interlock(true, ms(251), ms(2), false, [0.; 3], [0.; 3], &v),
            interlock(true, ms(0), ms(26), false, [0.; 3], [0.; 3], &v),
            interlock(true, ms(0), ms(2), true, [0.; 3], [0.; 3], &v),
            interlock(true, ms(0), ms(2), false, [0.06, 0., 0.], [0.; 3], &v),
            interlock(true, ms(0), ms(2), false, [0.; 3], [0.04, 0., 0.], &v),
        ] {
            assert!(result.is_err());
        }
    }
    #[test]
    fn curved_route_proof_does_not_require_or_accept_a_straight_shortcut() {
        let mut t = Teaching::new([0.; 3]).unwrap();
        for i in 1..=100 {
            t.sample([i as f64 * 0.0002, 0., 0.], 0.01).unwrap();
        }
        for i in 1..=100 {
            t.sample([0.02, i as f64 * 0.0002, 0.], 0.01).unwrap();
        }
        t.finish(true).unwrap();
        let e = t.evidence().unwrap();
        let a = Approach::from_teaching(e.clone(), &Sweep::default(), Instant::now()).unwrap();
        assert_ne!(
            a.cloud
                .review_segment(e.start_model_m, e.zero_model_m)
                .unwrap(),
            haptics_workspace::SegmentReview::Supported
        );
        let mut shortcut = e;
        shortcut.return_route_model_m = vec![shortcut.start_model_m, shortcut.zero_model_m];
        assert!(Approach::from_teaching(shortcut, &Sweep::default(), Instant::now()).is_err());
    }
    #[test]
    fn live_trace_authority_expires_and_rejects_changed_start_zero_and_force() {
        let start = [0.; 3];
        let zero = [0., 0.02, 0.];
        let mut trainer = Teaching::new(zero).unwrap();
        for i in (0..100).rev() {
            trainer.sample([0., i as f64 * 0.0002, 0.], 0.01).unwrap();
        }
        trainer.finish(true).unwrap();
        let now = Instant::now();
        let proof =
            Approach::from_teaching(trainer.evidence().unwrap(), &Sweep::default(), now).unwrap();
        assert!(
            proof
                .validate_start(now, start, zero, Test::Zero, 0.1)
                .is_ok()
        );
        assert!(
            proof
                .validate_start(now, [0., 0.01, 0.], zero, Test::Zero, 0.1)
                .is_err()
        );
        assert!(
            proof
                .validate_start(now, [0., 0.01, 0.], zero, Test::Object(Kind::Cube), 0.1)
                .is_ok()
        );
        assert!(
            proof
                .validate_start(now, [0.01, 0., 0.], zero, Test::Object(Kind::Cube), 0.1)
                .is_err()
        );
        assert!(
            proof
                .validate_start(now + teaching::MAX_AGE, start, zero, Test::Zero, 0.1)
                .is_err()
        );
        assert!(
            proof
                .validate_start(now, start, [0., 0.021, 0.], Test::Zero, 0.1)
                .is_err()
        );
        for force in [0.101, f64::NAN, f64::INFINITY, -0.1, 0.] {
            assert!(
                proof
                    .validate_start(now, start, zero, Test::Zero, force)
                    .is_err()
            );
        }
        let mut snapshot = Snapshot {
            approach_ready: true,
            teaching: true,
            approach_start: Some(start),
            current_supported: true,
            ..Default::default()
        };
        let mut proof = Some(proof);
        let mut trainer = Some(trainer);
        invalidate_approach(&mut trainer, &mut proof, &mut snapshot);
        assert!(proof.is_none() && trainer.is_none());
        assert!(!snapshot.approach_ready && !snapshot.teaching && !snapshot.current_supported);
        assert!(snapshot.approach_start.is_none());
    }

    #[test]
    fn capture_requires_quiet_history_and_sufficient_duration() {
        let start = Instant::now();
        let mut h = VecDeque::new();
        h.push_back((start, [0.; 3]));
        assert!(stable(&h).is_none());
        h.push_back((start + Duration::from_millis(300), [0.0001, 0., 0.]));
        assert!(stable(&h).is_some());
        h.push_back((start + Duration::from_millis(301), [0.001, 0., 0.]));
        assert!(stable(&h).is_none());
    }
}
