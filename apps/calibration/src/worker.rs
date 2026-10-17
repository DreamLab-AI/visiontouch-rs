//! The servo engine and its thread.
//!
//! [`Engine`] is generic over [`Hardware`], so the same orchestration drives the real
//! device, the in-process demo and the headless smoke test. Per sample it reads,
//! ticks the [`Controller`], requests the demand only while armed, and on any stop
//! disarms *before* a single byte of evidence is written.
use crate::controller::{Controller, Outcome, StopReason, TickInput, TickOutput};
use crate::coverage::Coverage;
use crate::evidence::{
    self, AttemptRecord, OutputOff, Recorder, RegistrationRecord, SessionRecord, Software,
    VerdictRecord,
};
use crate::protocol::{
    Axis, Mark, RULE, Reading, Session, StepKind, StepRequest, Unit, Verdict, score_rung,
};
use crate::registration::{GravityWatch, Stillness};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};
use visiontouch_device::{
    Demand, Device, EncoderState, ForceGuard, ForceLimits, Registration, RequestError, Telemetry,
};
use visiontouch_effects::Frame;
use visiontouch_effects::math::V;
use visiontouch_hal::device::Sample;

/// Parallel port used by the physical device.
pub const PORT: &str = "/dev/parport0";
/// Where evidence is written, relative to the repository root.
pub const EVIDENCE_DIR: &str = "captures/calibration";

/// The device surface the engine needs. Implemented by the real [`Device`] and by the
/// simulator.
pub trait Hardware {
    /// Monotonic time. The simulator uses a virtual clock.
    fn now(&self) -> Instant;
    /// Read one packet and service the guard.
    fn sample(&mut self) -> std::io::Result<Sample>;
    /// Encoder state.
    fn state(&self) -> &EncoderState;
    /// Guard, for limits and telemetry.
    fn guard(&self) -> &ForceGuard;
    /// Capture the reset pose; turns output off.
    fn register_reset(&mut self) -> Result<Registration, &'static str>;
    /// Forget the registration; turns output off.
    fn clear_registration(&mut self);
    /// Configure limits while disarmed.
    fn set_limits(&mut self, l: ForceLimits) -> Result<(), &'static str>;
    /// Begin the motor session and arm.
    fn enable_feedback(&mut self) -> std::io::Result<()>;
    /// Disarm and shut the motor session down.
    fn disable_feedback(&mut self) -> std::io::Result<()>;
    /// Whether output is armed.
    fn feedback_enabled(&self) -> bool;
    /// Queue a demand.
    fn request(&mut self, d: Demand, t: Instant) -> Result<(), RequestError>;
}

/// The physical device.
pub struct DeviceHw(pub Device);
impl Hardware for DeviceHw {
    fn now(&self) -> Instant {
        Instant::now()
    }
    fn sample(&mut self) -> std::io::Result<Sample> {
        self.0.sample()
    }
    fn state(&self) -> &EncoderState {
        &self.0.state
    }
    fn guard(&self) -> &ForceGuard {
        &self.0.guard
    }
    fn register_reset(&mut self) -> Result<Registration, &'static str> {
        self.0.register_reset()
    }
    fn clear_registration(&mut self) {
        self.0.clear_registration();
    }
    fn set_limits(&mut self, l: ForceLimits) -> Result<(), &'static str> {
        self.0.guard.set_limits(l)
    }
    fn enable_feedback(&mut self) -> std::io::Result<()> {
        self.0.enable_feedback()
    }
    fn disable_feedback(&mut self) -> std::io::Result<()> {
        self.0.disable_feedback()
    }
    fn feedback_enabled(&self) -> bool {
        self.0.feedback_enabled()
    }
    fn request(&mut self, d: Demand, t: Instant) -> Result<(), RequestError> {
        self.0.request(d, t)
    }
}

/// What kind of operator entry an attempt is waiting for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    /// A gauge, scale or temperature reading.
    Reading,
    /// Felt and heard.
    Observation,
}

/// Messages from the interface to the engine.
#[derive(Debug, Clone)]
pub enum Command {
    /// Open the device with output off.
    Connect,
    /// Close the device.
    Disconnect,
    /// Capture the reset pose from a still pen.
    RegisterReset,
    /// Arm the gravity-drop sign check for one drop. Nothing else records a drop.
    ArmGravityDrop,
    /// Capture a position mark from a still pen.
    CaptureStill(Mark),
    /// Enter the ruler distance for an axis.
    SubmitRuler {
        /// Axis slid along.
        axis: Axis,
        /// Signed ruler millimetres.
        measured_mm: f64,
    },
    /// Switch the coverage cloud on or off.
    Coverage(bool),
    /// Write the coverage cloud as a viewer capture.
    ExportCoverage,
    /// Start a step.
    Start(StepRequest),
    /// Thermal confirmation.
    Confirm,
    /// Operator readings for an attempt.
    SubmitReading {
        /// Attempt id.
        attempt_id: u64,
        /// Readings.
        readings: Vec<Reading>,
    },
    /// Operator observation for an attempt.
    SubmitObservation {
        /// Attempt id.
        attempt_id: u64,
        /// Felt something.
        felt: Option<bool>,
        /// Heard something.
        heard: Option<bool>,
        /// Free note.
        note: String,
    },
    /// Stop the running step.
    Stop,
    /// Shut the worker down.
    Quit,
}

/// Interface heartbeat and hold state shared with the worker.
#[derive(Default)]
pub struct Controls {
    /// A hold key is down.
    pub held: AtomicBool,
    /// Incremented by the interface every frame.
    pub pulse: AtomicU64,
}

/// Summary of the last finished attempt.
#[derive(Clone, Debug, Default)]
pub struct LastResult {
    /// Attempt id.
    pub attempt_id: u64,
    /// Step label.
    pub label: String,
    /// Stop reason name.
    pub reason: String,
    /// Verdict kind.
    pub verdict: String,
    /// Ended as intended.
    pub normal: bool,
}

/// Everything the interface needs, copied out of the engine.
#[derive(Clone, Default)]
pub struct Snapshot {
    /// Device open.
    pub connected: bool,
    /// Simulated plant.
    pub simulated: bool,
    /// Output armed.
    pub feedback: bool,
    /// Reset pose captured.
    pub registered: bool,
    /// The gravity-drop check is waiting for a drop.
    pub gravity_armed: bool,
    /// Registration counters.
    pub registration: Option<Registration>,
    /// Pen still for 250 ms.
    pub stable: bool,
    /// Running step.
    pub active: Option<StepRequest>,
    /// Running attempt id.
    pub attempt_id: Option<u64>,
    /// Seconds into the step.
    pub elapsed_s: f64,
    /// Seconds left of a fixed-length step.
    pub remaining_s: Option<f64>,
    /// Registered pen position, metres.
    pub position: Option<V>,
    /// Power-on-relative position, display only.
    pub unregistered_position: V,
    /// Filtered velocity, metres per second.
    pub velocity: V,
    /// Raw counters.
    pub counts: [i16; 6],
    /// Unwrapped counters.
    pub continuous: [i64; 6],
    /// Raw packet.
    pub raw: [u8; 16],
    /// Guard telemetry.
    pub telemetry: Telemetry,
    /// Configured limits.
    pub limits: ForceLimits,
    /// Effect frame.
    pub effect_frame: Option<Frame>,
    /// Effect proxy in registered metres.
    pub proxy: Option<V>,
    /// Wall or effect target.
    pub target: Option<V>,
    /// Effect centre, registered metres.
    pub centre: Option<V>,
    /// Wall stiffness level, N/m.
    pub wall_k_n_m: Option<f64>,
    /// Session unlock state.
    pub session: Session,
    /// Steps unlocked now.
    pub unlocked: Vec<StepKind>,
    /// Lift cap, newtons.
    pub lift_cap_n: f64,
    /// Effects cap, newtons.
    pub effects_cap_n: f64,
    /// Last finished attempt.
    pub last_result: Option<LastResult>,
    /// Entry the interface should ask for.
    pub awaiting_entry: Option<(u64, EntryKind)>,
    /// One-line status.
    pub status: String,
    /// Recent log lines.
    pub log: Vec<String>,
    /// Sample rate.
    pub rate_hz: f64,
    /// Stale ticks reported by the guard in the current attempt.
    pub stale_ticks: u64,
    /// Coverage cloud points.
    pub coverage: Arc<Vec<V>>,
    /// Coverage recording.
    pub coverage_on: bool,
    /// Seconds to the next thermal confirmation.
    pub confirm_in_s: Option<f64>,
    /// Pen button.
    pub button: bool,
    /// Evidence directory.
    pub session_dir: Option<String>,
}

struct Running {
    record: AttemptRecord,
    rows: Recorder,
    req: StepRequest,
    pending: Vec<Reading>,
}
struct Done {
    record: AttemptRecord,
    req: StepRequest,
    reason: StopReason,
}

/// A ladder dwell runs from 0.5 s to 2.5 s; a reading is only meaningful once it is over.
const LADDER_DWELL_END_S: f64 = 2.5;

/// The reading unit an attempt of this kind may take, or `None` when it takes none.
fn unit_allowed(req: StepRequest, unit: Unit) -> bool {
    match req {
        StepRequest::Ladder { .. } | StepRequest::Lift { .. } | StepRequest::Backdrive => {
            matches!(unit, Unit::N | Unit::G)
        }
        StepRequest::Thermal => unit == Unit::C,
        _ => false,
    }
}
fn unit_rule(req: StepRequest) -> &'static str {
    match req {
        StepRequest::Ladder { .. } | StepRequest::Lift { .. } | StepRequest::Backdrive => "N or g",
        StepRequest::Thermal => "°C",
        _ => "no reading",
    }
}

/// Servo engine over some [`Hardware`].
pub struct Engine<H: Hardware> {
    hw: Option<H>,
    simulated: bool,
    root: PathBuf,
    dir: Option<PathBuf>,
    session_id: String,
    connected_utc: String,
    registration_utc: Option<String>,
    software: Software,
    /// Unlock state of this connection.
    pub session: Session,
    controller: Controller,
    stillness: Stillness,
    gravity: GravityWatch,
    gravity_armed: bool,
    coverage: Coverage,
    coverage_on: bool,
    cov_points: Arc<Vec<V>>,
    cov_len: usize,
    cov_at: Option<Instant>,
    cov_first: Option<Instant>,
    epoch: Option<Instant>,
    last_now: Option<Instant>,
    last_input: Option<TickInput>,
    last_out: Option<TickOutput>,
    running: Option<Running>,
    done: Vec<Done>,
    attempt_counter: u64,
    last_result: Option<LastResult>,
    awaiting: Option<(u64, EntryKind)>,
    status: String,
    log: VecDeque<String>,
    rate_hz: f64,
    stale_ticks: u64,
}

impl<H: Hardware> Engine<H> {
    /// A disconnected engine writing evidence below `root`.
    pub fn new(root: PathBuf, simulated: bool) -> Self {
        Self {
            hw: None,
            simulated,
            root,
            dir: None,
            session_id: String::new(),
            connected_utc: String::new(),
            registration_utc: None,
            software: Software::current(),
            session: Session::default(),
            controller: Controller::default(),
            stillness: Stillness::default(),
            gravity: GravityWatch::default(),
            gravity_armed: false,
            coverage: Coverage::default(),
            coverage_on: false,
            cov_points: Arc::new(Vec::new()),
            cov_len: 0,
            cov_at: None,
            cov_first: None,
            epoch: None,
            last_now: None,
            last_input: None,
            last_out: None,
            running: None,
            done: Vec::new(),
            attempt_counter: 0,
            last_result: None,
            awaiting: None,
            status: "Not connected. Output is off.".into(),
            log: VecDeque::new(),
            rate_hz: 0.,
            stale_ticks: 0,
        }
    }
    /// Whether a device is open.
    pub fn connected(&self) -> bool {
        self.hw.is_some()
    }
    /// Direct access to the hardware, for the simulator.
    pub fn hardware_mut(&mut self) -> Option<&mut H> {
        self.hw.as_mut()
    }
    /// The evidence directory, once created.
    pub fn session_dir(&self) -> Option<&PathBuf> {
        self.dir.as_ref()
    }
    /// The latest status line.
    pub fn status(&self) -> &str {
        &self.status
    }
    /// A running step, if any.
    pub fn active(&self) -> Option<StepRequest> {
        self.controller.active()
    }
    /// Seconds to the next thermal confirmation.
    pub fn confirm_in_s(&self) -> Option<f64> {
        let now = self.last_now?;
        self.controller.confirm_in_s(now)
    }
    fn say(&mut self, text: impl Into<String>) {
        let t = text.into();
        self.log.push_back(t.clone());
        while self.log.len() > 14 {
            self.log.pop_front();
        }
        self.status = t;
    }
    /// Open a connection. A new connection forgets every unlock.
    pub fn connect(&mut self, hw: H) {
        self.disconnect();
        self.hw = Some(hw);
        self.session = Session::connected();
        self.session_id = evidence::new_session_id();
        self.connected_utc = evidence::utc_now();
        self.dir = None;
        self.done.clear();
        self.attempt_counter = 0;
        self.registration_utc = None;
        self.stillness.clear();
        self.gravity = GravityWatch::default();
        self.gravity_armed = false;
        self.coverage.clear();
        self.last_now = None;
        self.epoch = None;
        self.last_result = None;
        self.awaiting = None;
        self.say("Connected with motor output off. Register the reset pose to begin.");
    }
    /// Close the connection after shutting output down.
    pub fn disconnect(&mut self) {
        self.controller.stop();
        if let Some(hw) = self.hw.as_mut() {
            let _ = hw.disable_feedback();
        }
        self.running = None;
        self.controller = Controller::default();
        if self.hw.take().is_some() {
            self.say("Disconnected. Output is off.");
        }
        self.session.disconnect();
        self.last_input = None;
    }
    /// A transport fault: output is shut down and the connection dropped.
    pub fn fault(&mut self, message: &str) {
        self.disconnect();
        self.say(format!(
            "Transport fault: {message}. Output is off; reconnect."
        ));
    }

    fn ensure_dir(&mut self, off: &OutputOff) -> std::io::Result<PathBuf> {
        if let Some(d) = &self.dir {
            return Ok(d.clone());
        }
        let d = evidence::create_session_dir(
            off,
            &self.root,
            &evidence::compact(evidence::unix_now()),
            &self.session_id,
        )?;
        self.dir = Some(d.clone());
        Ok(d)
    }
    fn write_session_file(&mut self, off: &OutputOff) -> std::io::Result<()> {
        let dir = self.ensure_dir(off)?;
        let registration = self
            .hw
            .as_ref()
            .and_then(|h| h.state().registration())
            .zip(self.registration_utc.clone())
            .map(|(r, t)| RegistrationRecord::new(r, t));
        let rec = SessionRecord {
            schema: evidence::SCHEMA,
            kind: "haptics-verification-session".into(),
            session_id: self.session_id.clone(),
            connected_utc: self.connected_utc.clone(),
            registration,
            checks: self.session.checks.clone(),
            unlock_history: self.session.history.clone(),
            attempts: self.done.iter().map(|d| d.record.stem()).collect(),
            claim: evidence::CLAIM.into(),
        };
        evidence::write_session(off, &dir, &rec)
    }
    fn off(&self) -> Result<OutputOff, String> {
        OutputOff::confirm(self.hw.as_ref().is_some_and(Hardware::feedback_enabled))
    }
    fn save_session(&mut self) {
        match self
            .off()
            .and_then(|o| self.write_session_file(&o).map_err(|e| e.to_string()))
        {
            Ok(()) => {}
            Err(e) => self.say(format!("Session file not written: {e}")),
        }
    }

    /// Handle an interface command.
    pub fn handle(&mut self, cmd: Command) {
        match cmd {
            Command::Connect | Command::Quit => {}
            Command::Disconnect => self.disconnect(),
            Command::RegisterReset => self.register_reset(),
            Command::ArmGravityDrop => self.arm_gravity_drop(),
            Command::CaptureStill(mark) => self.capture(mark),
            Command::SubmitRuler { axis, measured_mm } => {
                let before = self.session.unlocked();
                match self.session.checks.submit_ruler(axis, measured_mm) {
                    Ok(r) => {
                        let warn = if r.scale_warning {
                            " Scale outside 0.8 to 1.25: recorded, not a stop."
                        } else {
                            ""
                        };
                        let verdict = if r.ok() {
                            "ok"
                        } else {
                            "FAILED: sign or off-axis"
                        };
                        self.say(format!(
                            "Ruler {}: model {:.1} mm, ruler {:.1} mm, scale {:.2}, off-axis {:.0}% {verdict}.{warn}",
                            axis.name(), r.model_mm, r.measured_mm, r.scale, r.off_axis * 100.
                        ));
                    }
                    Err(e) => self.say(e),
                }
                self.after_checks(&before);
            }
            Command::Coverage(on) => {
                self.coverage_on = on;
                if on {
                    self.cov_first.get_or_insert_with(Instant::now);
                }
                self.say(if on {
                    "Coverage recording on."
                } else {
                    "Coverage recording off."
                });
            }
            Command::ExportCoverage => self.export_coverage(),
            Command::Start(req) => self.start(req),
            Command::Confirm => {
                let now = self.last_now.unwrap_or_else(Instant::now);
                if self.controller.confirm(now) {
                    self.say("Thermal confirmation accepted.");
                } else {
                    self.say("No confirmation is due yet.");
                }
            }
            Command::SubmitReading {
                attempt_id,
                readings,
            } => self.submit_reading(attempt_id, readings),
            Command::SubmitObservation {
                attempt_id,
                felt,
                heard,
                note,
            } => {
                self.submit_observation(attempt_id, felt, heard, note);
            }
            Command::Stop => {
                self.controller.stop();
                if self.controller.active().is_none() {
                    self.say("Nothing is running. Output is off.");
                }
            }
        }
    }

    fn after_checks(&mut self, before: &[StepKind]) {
        let new = self.session.checks_changed(before);
        if !new.is_empty() {
            self.say(format!("Unlocked: {}.", new.join(", ")));
        }
        self.save_session();
    }
    fn register_reset(&mut self) {
        if self.controller.active().is_some() {
            return self.say("Stop the running step before registering.");
        }
        if !self.stillness.is_still() {
            return self.say("Hold the pen still for a quarter of a second, then register.");
        }
        let Some(hw) = self.hw.as_mut() else {
            return self.say("Connect first.");
        };
        match hw.register_reset() {
            Ok(_) => {
                self.registration_utc = Some(evidence::utc_now());
                self.session.record_registered(false);
                let new = self.session.record_registered(true);
                self.stillness.clear();
                self.gravity = GravityWatch::default();
                self.gravity_armed = false;
                self.coverage.clear();
                let tail = if new.is_empty() {
                    String::new()
                } else {
                    format!(" Unlocked: {}.", new.join(", "))
                };
                self.say(format!(
                    "Reset pose registered. Now: move away and return, drop onto your palm, and slide the ruler on X, Y, Z.{tail}"
                ));
                self.save_session();
            }
            Err(e) => self.say(format!("Registration refused: {e}")),
        }
    }
    fn arm_gravity_drop(&mut self) {
        if self.hw.is_none() || !self.session.registered {
            return self.say("Register the reset pose first.");
        }
        if self.controller.active().is_some()
            || self.hw.as_ref().is_some_and(Hardware::feedback_enabled)
        {
            return self.say("Arm the gravity drop only while output is off and no step runs.");
        }
        if self.session.checks.gravity == Some(false) {
            return self.say(
                "Gravity drop already failed: the Y sign is wrong. Re-register the reset pose after the model is corrected.",
            );
        }
        self.gravity = GravityWatch::default();
        self.gravity_armed = true;
        self.say(
            "Gravity drop armed: hold the pen still mid-height, then let it fall onto your palm.",
        );
    }
    fn capture(&mut self, mark: Mark) {
        let Some(hw) = self.hw.as_ref() else {
            return self.say("Connect first.");
        };
        let Some(p) = hw.state().position_m() else {
            return self.say("Register the reset pose first.");
        };
        let still = self.stillness.is_still();
        let before = self.session.unlocked();
        match self.session.checks.capture(mark, p, still) {
            Ok(()) => {
                let text = match mark {
                    Mark::ReturnToReset => format!(
                        "Return to reset captured: {:.2} mm from the origin.",
                        self.session.checks.return_error_mm.unwrap_or(0.)
                    ),
                    other => format!(
                        "Captured {other:?} at {:.1} {:.1} {:.1} mm.",
                        p[0] * 1e3,
                        p[1] * 1e3,
                        p[2] * 1e3
                    ),
                };
                self.say(text);
                self.after_checks(&before);
            }
            Err(e) => self.say(e),
        }
    }
    fn export_coverage(&mut self) {
        let duration = self.cov_first.map_or(0., |t| t.elapsed().as_secs_f64());
        let json = match self.coverage.export(duration) {
            Ok(j) => j,
            Err(e) => return self.say(e),
        };
        let off = match self.off() {
            Ok(o) => o,
            Err(e) => return self.say(e),
        };
        let result = self.ensure_dir(&off).and_then(|d| {
            let p = d.join("coverage.json");
            std::fs::write(
                &p,
                serde_json::to_vec_pretty(&json).map_err(std::io::Error::other)?,
            )?;
            Ok(p)
        });
        match result {
            Ok(p) => self.say(format!("Coverage exported to {}.", p.display())),
            Err(e) => self.say(format!("Coverage export failed: {e}")),
        }
    }

    fn start(&mut self, req: StepRequest) {
        let allowance = match self.session.check(req) {
            Ok(a) => a,
            Err(e) => return self.say(e),
        };
        if self.controller.active().is_some() {
            return self.say("A step is already running.");
        }
        let (Some(hw), Some(input)) = (self.hw.as_mut(), self.last_input) else {
            return self.say("Connect first.");
        };
        let centre = self.session.checks.effect_centre;
        if let Err(e) = self.controller.start(req, allowance, &input, centre) {
            return self.say(e);
        }
        self.gravity_armed = false;
        // Explicit limits for every attempt: never inherit the previous one.
        let armed = req != StepRequest::Backdrive;
        let set = hw
            .disable_feedback()
            .map_err(|e| e.to_string())
            .and_then(|()| hw.set_limits(allowance.limits).map_err(String::from))
            .and_then(|()| {
                if armed {
                    hw.enable_feedback().map_err(|e| e.to_string())
                } else {
                    Ok(())
                }
            });
        if let Err(e) = set {
            self.controller.stop();
            let _ = self.controller.tick(&input);
            let _ = self.controller.take_outcome();
            return self.say(format!("Could not arm: {e}"));
        }
        self.attempt_counter += 1;
        let registration = hw
            .state()
            .registration()
            .zip(self.registration_utc.clone())
            .map(|(r, t)| RegistrationRecord::new(r, t));
        let record = AttemptRecord::new(
            &self.session_id,
            self.attempt_counter,
            req,
            evidence::utc_now(),
            allowance.limits,
            registration,
            self.software.clone(),
        );
        let keep = if req == StepRequest::Thermal { 10 } else { 1 };
        let mut record = record;
        record.params["csv_row_every"] = serde_json::json!(keep);
        self.running = Some(Running {
            record,
            rows: Recorder::new(keep),
            req,
            pending: Vec::new(),
        });
        self.stale_ticks = 0;
        self.awaiting = None;
        self.say(format!(
            "{} running (attempt {}). {}",
            req.label(),
            self.attempt_counter,
            if crate::controller::class_limits(req.class()).needs_hold {
                "Release the hold key to stop."
            } else {
                "Press Stop to end."
            }
        ));
    }

    /// One servo iteration: read, decide, request. An error is a transport fault.
    pub fn step(&mut self, held: bool, ui_age_ms: f64) -> Result<(), String> {
        let Some(hw) = self.hw.as_mut() else {
            return Ok(());
        };
        let before = Instant::now();
        let sample = match hw.sample() {
            Ok(s) => s,
            Err(e) => {
                let m = e.to_string();
                self.fault(&m);
                return Err(m);
            }
        };
        let io_us = before.elapsed().as_micros() as u64;
        let now = hw.now();
        let epoch = *self.epoch.get_or_insert(now);
        let interval = self
            .last_now
            .map_or(0.0024, |p| now.saturating_duration_since(p).as_secs_f64());
        self.last_now = Some(now);
        self.rate_hz = if interval > 0. {
            0.98 * self.rate_hz + 0.02 / interval
        } else {
            self.rate_hz
        };
        let st = hw.state();
        let registered_pos = st.position_m();
        let input = TickInput {
            now,
            interval_s: interval,
            io_us,
            held,
            ui_age_ms,
            button: st.button_pressed,
            counts: st.counts,
            continuous: st.continuous_counts(),
            registered: st.registered_counts(),
            position: registered_pos.unwrap_or_else(|| st.unregistered_position_m()),
            raw: sample.raw,
            telemetry: hw.guard().telemetry(),
        };
        self.last_input = Some(input);
        let t = now.saturating_duration_since(epoch).as_secs_f64();
        self.stillness.push(t, input.position);
        if let Some(p) = registered_pos {
            if self.coverage_on {
                self.coverage.record(p);
                let due = self
                    .cov_at
                    .is_none_or(|a| now.saturating_duration_since(a) > Duration::from_millis(200));
                if due && self.coverage.len() != self.cov_len {
                    self.cov_points = Arc::new(self.coverage.points());
                    self.cov_len = self.coverage.len();
                    self.cov_at = Some(now);
                }
            }
            if self.gravity_armed && self.controller.active().is_none() && !hw.feedback_enabled() {
                let still = self.stillness.is_still();
                if let Some(ok) = self.gravity.update(t, p, still) {
                    self.gravity_armed = false;
                    let before = self.session.unlocked();
                    self.session.checks.record_gravity(ok);
                    self.say(if ok {
                        "Gravity drop: the pen moved toward -Y. Sign confirmed."
                    } else {
                        "Gravity drop: the pen moved toward +Y. The Y sign is wrong; do not continue."
                    });
                    self.after_checks(&before);
                }
            }
        }
        if let Some(out) = self.controller.tick(&input) {
            self.last_out = Some(out);
            self.stale_ticks += u64::from(input.telemetry.expired);
            if let Some(run) = self.running.as_mut() {
                run.rows.push(&input, &out);
            }
            let hw = self.hw.as_mut().expect("hardware present");
            if hw.feedback_enabled() {
                match hw.request(out.demand, now) {
                    Ok(()) => {}
                    Err(RequestError::Rejected(why)) => {
                        self.controller.reject();
                        self.say(format!("Request rejected ({why}); output shut down."));
                    }
                    Err(RequestError::ShutdownFailed(e)) => {
                        let m = e.to_string();
                        self.fault(&m);
                        return Err(m);
                    }
                }
            }
            if let Some(reason) = out.stop {
                self.finish(reason);
            }
        }
        Ok(())
    }

    fn finish(&mut self, reason: StopReason) {
        // Output off first, always.
        if let Some(hw) = self.hw.as_mut()
            && let Err(e) = hw.disable_feedback()
        {
            self.fault(&format!("shutdown failed: {e}"));
            return;
        }
        let (Some(outcome), Some(mut run)) = (self.controller.take_outcome(), self.running.take())
        else {
            return;
        };
        let id = run.record.attempt_id;
        let Outcome {
            request,
            duration_s,
            auto,
            summary,
            detail,
            ..
        } = outcome;
        run.record.duration_s = duration_s;
        run.record.auto = auto.clone();
        run.record.telemetry_summary = summary;
        run.record.stop = evidence::StopRecord {
            reason: reason.name().into(),
            detail,
        };
        run.record.operator.readings.append(&mut run.pending);
        let mut unlocks = Vec::new();
        let mut kind = String::from("recorded");
        let mut pass = None;
        let mut entry = None;
        match request {
            StepRequest::Liveness { channel } => {
                let p = auto.lockin_pass == Some(true);
                unlocks = self.session.record_liveness(channel, p);
                kind = if p { "auto-pass" } else { "auto-fail" }.into();
                pass = Some(p);
                entry = Some(EntryKind::Observation);
            }
            StepRequest::SignPulse { axis } => {
                let p = auto.sign_agrees == Some(true);
                unlocks = self.session.record_sign(axis, p);
                kind = match auto.sign_agrees {
                    Some(true) => "pass",
                    Some(false) => "fail",
                    None => "inconclusive",
                }
                .into();
                pass = Some(p);
                entry = Some(EntryKind::Observation);
            }
            StepRequest::Ladder { axis, sign, rung } => {
                if auto.oscillation {
                    unlocks = self.session.record_rung(rung, axis, sign, Verdict::Over);
                    kind = "over".into();
                    pass = Some(false);
                } else if reason == StopReason::Completed && duration_s >= LADDER_DWELL_END_S {
                    kind = "pending-reading".into();
                } else {
                    // No dwell was delivered: nothing to score, nothing to unlock.
                    kind = "aborted".into();
                    pass = Some(false);
                }
                entry = Some(EntryKind::Reading);
            }
            StepRequest::Lift { mass_g } => {
                if let Some(f) = auto.liftoff_force_n {
                    self.session.record_lift(mass_g.unwrap_or(0.), f);
                    kind = "liftoff".into();
                } else {
                    kind = "no-liftoff".into();
                }
            }
            StepRequest::WallRamp => {
                self.session.k_marginal_n_mm = auto.k_marginal_n_mm;
                kind = if auto.oscillation {
                    "oscillation"
                } else {
                    "stable"
                }
                .into();
                entry = Some(EntryKind::Observation);
            }
            StepRequest::Thermal => {
                self.session.thermal = Some(reason.is_normal());
                kind = if reason.is_normal() {
                    "completed"
                } else {
                    "stopped"
                }
                .into();
                entry = Some(EntryKind::Reading);
            }
            StepRequest::Backdrive => entry = Some(EntryKind::Reading),
            StepRequest::Effect { .. } => entry = Some(EntryKind::Observation),
        }
        run.record.verdict = VerdictRecord {
            kind: kind.clone(),
            pass,
            rule: RULE.into(),
            unlocks: unlocks.clone(),
        };
        self.awaiting = entry.map(|e| (id, e));
        self.last_result = Some(LastResult {
            attempt_id: id,
            label: request.label(),
            reason: reason.name().into(),
            verdict: kind,
            normal: reason.is_normal(),
        });
        let mut text = format!(
            "{} ended: {}. Output is off.",
            request.label(),
            reason.name()
        );
        if !unlocks.is_empty() {
            text.push_str(&format!(" Unlocked: {}.", unlocks.join(", ")));
        }
        if entry.is_some() {
            text.push_str(" Enter your reading below.");
        }
        match self.off() {
            Ok(off) => {
                let written = self.ensure_dir(&off).and_then(|dir| {
                    evidence::write_attempt(&off, &dir, &mut run.record, &run.rows)
                });
                if let Err(e) = written {
                    text.push_str(&format!(" Evidence NOT written: {e}"));
                }
            }
            Err(e) => text.push_str(&format!(" {e}")),
        }
        self.done.push(Done {
            record: run.record,
            req: run.req,
            reason,
        });
        self.save_session();
        self.say(text);
    }

    fn submit_reading(&mut self, id: u64, readings: Vec<Reading>) {
        if readings.iter().any(|r| !r.value.is_finite()) {
            return self.say("Readings must be finite numbers.");
        }
        if let Some(run) = self.running.as_mut().filter(|r| r.record.attempt_id == id) {
            let req = run.req;
            if let Some(bad) = readings.iter().find(|r| !unit_allowed(req, r.unit)) {
                let msg = format!(
                    "{} takes {} readings; a {:?} reading was refused.",
                    req.label(),
                    unit_rule(req),
                    bad.unit
                );
                return self.say(msg);
            }
            for r in readings {
                if r.unit == Unit::C {
                    self.controller.note_temperature(r.value);
                }
                run.pending.push(r);
            }
            return self.say("Reading recorded.");
        }
        if self.controller.active().is_some() {
            return self
                .say("Entries for finished attempts are accepted only while output is off.");
        }
        let off = match self.off() {
            Ok(o) => o,
            Err(e) => return self.say(e),
        };
        let Some(i) = self.done.iter().position(|d| d.record.attempt_id == id) else {
            return self.say(format!("No finished attempt {id}."));
        };
        let req = self.done[i].req;
        if let Some(bad) = readings.iter().find(|r| !unit_allowed(req, r.unit)) {
            let (unit, rule) = (bad.unit, unit_rule(req));
            return self.say(format!(
                "{} takes {rule} readings; a {unit:?} reading was refused.",
                req.label()
            ));
        }
        let mut unlocks = Vec::new();
        let mut text = String::from("Reading recorded.");
        {
            let done = &mut self.done[i];
            done.record.operator.readings.extend(readings);
            let measured = done
                .record
                .operator
                .readings
                .iter()
                .rev()
                .find_map(Reading::newtons);
            match (done.req, measured) {
                (StepRequest::Ladder { .. }, Some(_)) if done.record.verdict.kind == "aborted" => {
                    text = format!(
                        "Reading recorded, not scored: the attempt ended {} before a full dwell.",
                        done.reason.name()
                    );
                }
                (StepRequest::Ladder { axis, sign, rung }, Some(m)) => {
                    let c = rung.newtons();
                    let mut v = score_rung(c, m, done.record.auto.oscillation);
                    if done.record.verdict.kind == "over" {
                        v = Verdict::Over; // Over is sticky; later readings are recorded only.
                    }
                    unlocks = self.session.record_rung(rung, axis, sign, v);
                    done.record.verdict = VerdictRecord {
                        kind: format!("{v:?}").to_lowercase(),
                        pass: Some(v == Verdict::Pass),
                        rule: format!("{RULE}; m {m:.3} N of c {c:.2} N, slope {:.2}", m / c),
                        unlocks: unlocks.clone(),
                    };
                    text = format!(
                        "Rung {} {}{}: {v:?} (measured {m:.3} N, slope {:.2}).",
                        rung.name(),
                        sign.name(),
                        axis.name(),
                        m / c
                    );
                }
                (StepRequest::Backdrive, Some(m)) if m > 0.2 => {
                    text =
                        format!("Backdrive {m:.2} N is above 0.2 N: stop and inspect the cables.");
                }
                _ => {}
            }
        }
        if !unlocks.is_empty() {
            text.push_str(&format!(" Unlocked: {}.", unlocks.join(", ")));
        }
        let record = self.done[i].record.clone();
        let wrote = self
            .ensure_dir(&off)
            .and_then(|d| evidence::rewrite_attempt(&off, &d, &record));
        if let Err(e) = wrote {
            text.push_str(&format!(" Evidence NOT written: {e}"));
        }
        if self
            .awaiting
            .is_some_and(|(a, k)| a == id && k == EntryKind::Reading)
        {
            self.awaiting = None;
        }
        self.save_session();
        self.say(text);
    }
    fn submit_observation(
        &mut self,
        id: u64,
        felt: Option<bool>,
        heard: Option<bool>,
        note: String,
    ) {
        if self.controller.active().is_some() {
            return self.say("Observations are accepted only while output is off.");
        }
        let off = match self.off() {
            Ok(o) => o,
            Err(e) => return self.say(e),
        };
        let Some(i) = self.done.iter().position(|d| d.record.attempt_id == id) else {
            return self.say(format!("No finished attempt {id}."));
        };
        let mut unlocks = Vec::new();
        {
            let done = &mut self.done[i];
            done.record.operator.felt = felt;
            done.record.operator.heard = heard;
            done.record.operator.note = note;
            if let StepRequest::Liveness { channel } = done.req {
                let auto = done.record.auto.lockin_pass == Some(true);
                // A felt or heard tick counts only when the drive ran to its end.
                let observed = done.reason == StopReason::Completed
                    && (felt == Some(true) || heard == Some(true));
                let p = auto || observed;
                unlocks = self.session.record_liveness(channel, p);
                done.record.verdict.kind = if p { "pass" } else { "fail" }.into();
                done.record.verdict.pass = Some(p);
                done.record.verdict.unlocks = unlocks.clone();
            }
        }
        let record = self.done[i].record.clone();
        let mut text = String::from("Observation recorded.");
        if !unlocks.is_empty() {
            text.push_str(&format!(" Unlocked: {}.", unlocks.join(", ")));
        }
        if let Err(e) = self
            .ensure_dir(&off)
            .and_then(|d| evidence::rewrite_attempt(&off, &d, &record))
        {
            text.push_str(&format!(" Evidence NOT written: {e}"));
        }
        if self
            .awaiting
            .is_some_and(|(a, k)| a == id && k == EntryKind::Observation)
        {
            self.awaiting = None;
        }
        self.save_session();
        self.say(text);
    }

    /// Copy everything the interface displays.
    pub fn snapshot(&self) -> Snapshot {
        let mut s = Snapshot {
            simulated: self.simulated,
            session: self.session.clone(),
            unlocked: self.session.unlocked(),
            lift_cap_n: self.session.lift_cap_n(),
            effects_cap_n: self.session.effects_cap_n(),
            last_result: self.last_result.clone(),
            awaiting_entry: self.awaiting,
            status: self.status.clone(),
            log: self.log.iter().cloned().collect(),
            rate_hz: self.rate_hz,
            stale_ticks: self.stale_ticks,
            coverage: self.cov_points.clone(),
            coverage_on: self.coverage_on,
            gravity_armed: self.gravity_armed,
            confirm_in_s: self.confirm_in_s(),
            session_dir: self.dir.as_ref().map(|d| d.display().to_string()),
            ..Snapshot::default()
        };
        let Some(hw) = self.hw.as_ref() else {
            return s;
        };
        let st = hw.state();
        s.connected = true;
        s.feedback = hw.feedback_enabled();
        s.registered = st.registration().is_some();
        s.registration = st.registration();
        s.stable = self.stillness.is_still();
        s.position = st.position_m();
        s.unregistered_position = st.unregistered_position_m();
        s.counts = st.counts;
        s.continuous = st.continuous_counts();
        s.button = st.button_pressed;
        s.telemetry = hw.guard().telemetry();
        s.limits = hw.guard().limits();
        if let Some(i) = &self.last_input {
            s.raw = i.raw;
        }
        if let Some(o) = &self.last_out {
            s.velocity = o.velocity;
        }
        if let Some((plan, req)) = self.controller.plan_view() {
            s.active = Some(req);
            s.attempt_id = self.running.as_ref().map(|r| r.record.attempt_id);
            s.elapsed_s = self.last_out.map_or(0., |o| o.elapsed_s);
            s.remaining_s = plan.duration_s().map(|d| (d - s.elapsed_s).max(0.));
            s.effect_frame = plan.frame;
            s.target = plan.target();
            s.centre = Some(plan.centre());
            s.wall_k_n_m = plan.wall_k_n_m;
            s.proxy = plan
                .frame
                .map(|f| visiontouch_effects::math::add(plan.centre(), f.proxy));
        }
        s
    }
}

/// Handle to the worker thread.
pub struct Worker {
    /// Shared hold state and heartbeat.
    pub controls: Arc<Controls>,
    commands: mpsc::Sender<Command>,
    /// Latest snapshots; the worker drops one if the interface is behind.
    pub snapshots: mpsc::Receiver<Snapshot>,
    join: Option<thread::JoinHandle<()>>,
}
impl Worker {
    /// Spawn the servo thread. Nothing opens until [`Command::Connect`].
    pub fn spawn(root: PathBuf) -> Self {
        let (commands, rx) = mpsc::channel();
        let (tx, snapshots) = mpsc::sync_channel(1);
        let controls = Arc::new(Controls::default());
        let shared = controls.clone();
        let join = thread::spawn(move || run(rx, tx, shared, root));
        Self {
            controls,
            commands,
            snapshots,
            join: Some(join),
        }
    }
    /// Send a command.
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

// P-0065: real I/O must not turn a four-tick filter into a 1 ms window.
const SERVO_PERIOD: Duration = Duration::from_micros(2400);

fn run(
    rx: mpsc::Receiver<Command>,
    tx: mpsc::SyncSender<Snapshot>,
    controls: Arc<Controls>,
    root: PathBuf,
) {
    let mut engine: Engine<DeviceHw> = Engine::new(root, false);
    let mut last_pulse = 0_u64;
    let mut pulse_at = Instant::now();
    let mut last_snapshot = Instant::now();
    loop {
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                Command::Quit => {
                    engine.disconnect();
                    return;
                }
                Command::Connect => match Device::connect(PORT) {
                    Ok(d) => engine.connect(DeviceHw(d)),
                    Err(e) => engine.say(format!("Cannot open {PORT}: {e}")),
                },
                other => engine.handle(other),
            }
        }
        let pulse = controls.pulse.load(Ordering::Relaxed);
        if pulse != last_pulse {
            last_pulse = pulse;
            pulse_at = Instant::now();
        }
        let cycle_started = Instant::now();
        if engine.connected() {
            let age = pulse_at.elapsed().as_secs_f64() * 1e3;
            let _ = engine.step(controls.held.load(Ordering::Relaxed), age);
        } else {
            thread::sleep(Duration::from_millis(15));
        }
        if last_snapshot.elapsed() >= Duration::from_millis(16) {
            last_snapshot = Instant::now();
            let _ = tx.try_send(engine.snapshot());
        }
        if engine.connected() {
            // Account for work already spent. Never catch up after an overrun;
            // Engine::step still measures the real interval for the gap interlock.
            thread::sleep(SERVO_PERIOD.saturating_sub(cycle_started.elapsed()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::{SimHw, SimPlant};

    #[test]
    fn captured_liveness_traces_no_longer_trip_speed_at_any_cadence() {
        let captures = [
            include_str!("../../../docs/evidence/liveness-cadence-20261017/0003-liveness.csv"),
            include_str!("../../../docs/evidence/liveness-cadence-20261017/0004-liveness.csv"),
            include_str!("../../../docs/evidence/liveness-cadence-20261017/0005-liveness.csv"),
        ];
        for (channel, csv) in captures.into_iter().enumerate() {
            let header: Vec<_> = csv.lines().next().unwrap().split(',').collect();
            let index = |name| header.iter().position(|c| *c == name).unwrap();
            let rows: Vec<(f64, V)> = csv
                .lines()
                .skip(1)
                .map(|line| {
                    let cols: Vec<_> = line.split(',').collect();
                    (
                        cols[index("t_s")].parse().unwrap(),
                        ["x_m", "y_m", "z_m"].map(|name| cols[index(name)].parse().unwrap()),
                    )
                })
                .collect();
            for paced in [false, true] {
                let base = Instant::now();
                let input = |t, p, dt| TickInput {
                    now: base + Duration::from_secs_f64(t),
                    interval_s: dt,
                    io_us: 280,
                    held: true,
                    ui_age_ms: 0.,
                    button: false,
                    counts: [0; 6],
                    continuous: [0; 6],
                    registered: None,
                    position: p,
                    raw: [0; 16],
                    telemetry: Default::default(),
                };
                let first = input(rows[0].0, rows[0].1, 0.0024);
                let mut c = Controller::default();
                c.start(
                    StepRequest::Liveness { channel },
                    crate::protocol::Allowance {
                        limits: ForceLimits::default(),
                        cap_n: 0.5,
                    },
                    &first,
                    None,
                )
                .unwrap();
                let mut previous = None;
                let mut stop = None;
                for &(t, p) in &rows {
                    let dt = previous.map_or(0.0024, |prev: f64| t - prev);
                    if paced && previous.is_some() && dt < SERVO_PERIOD.as_secs_f64() {
                        continue;
                    }
                    previous = Some(t);
                    let out = c.tick(&input(t, p, dt)).unwrap();
                    if out.stop.is_some() {
                        stop = out.stop;
                        break;
                    }
                }
                // P-0065: the original 0.28 ms traces tripped a four-sample
                // velocity mean. With a time-window velocity neither the original
                // nor the resampled trace trips, because the arm barely moved.
                assert_eq!(stop, None, "channel {channel}: paced={paced}");
            }
        }
        // These short replays do not assert that a physical two-second trial passes.
    }

    fn run_liveness(e: &mut Engine<SimHw>, channel: usize) {
        e.handle(Command::Start(StepRequest::Liveness { channel }));
        assert!(e.active().is_some(), "{}", e.status());
        while e.active().is_some() {
            e.step(true, 0.).unwrap();
        }
    }

    #[test]
    fn entries_change_only_the_attempt_they_name() {
        let root = std::env::temp_dir().join(format!("hc-engine-{}", evidence::new_session_id()));
        let mut e: Engine<SimHw> = Engine::new(root.clone(), true);
        e.connect(SimHw::new(SimPlant::default()));
        for _ in 0..50 {
            e.step(true, 0.).unwrap();
        }
        run_liveness(&mut e, 0);
        run_liveness(&mut e, 1);
        e.handle(Command::SubmitObservation {
            attempt_id: 2,
            felt: Some(true),
            heard: Some(false),
            note: "second".into(),
        });
        let first = &e.done[0].record;
        let second = &e.done[1].record;
        assert_eq!((first.attempt_id, second.attempt_id), (1, 2));
        assert_eq!(first.operator.felt, None, "attempt 1 untouched");
        assert!(first.operator.note.is_empty());
        assert_eq!(second.operator.felt, Some(true));
        assert_eq!(second.operator.note, "second");
        // A reading for an unknown attempt is refused, not misfiled.
        e.handle(Command::SubmitReading {
            attempt_id: 9,
            readings: vec![Reading {
                label: "x".into(),
                value: 1.,
                unit: Unit::N,
            }],
        });
        assert!(e.status().contains("No finished attempt 9"));
        assert!(e.done.iter().all(|d| d.record.operator.readings.is_empty()));
        let _ = std::fs::remove_dir_all(root);
    }

    use crate::protocol::{Rung, Sign};

    fn ready_engine(tag: &str) -> (Engine<SimHw>, PathBuf) {
        let root = std::env::temp_dir().join(format!("hc-{tag}-{}", evidence::new_session_id()));
        let mut e: Engine<SimHw> = Engine::new(root.clone(), true);
        e.connect(SimHw::new(SimPlant::default()));
        for _ in 0..300 {
            e.step(true, 0.).unwrap();
        }
        e.handle(Command::RegisterReset);
        assert!(e.snapshot().registered, "{}", e.status());
        e.step(true, 0.).unwrap();
        e.session = crate::protocol::tests::unlocked_session();
        for a in Axis::ALL {
            e.session.record_sign(a, true);
        }
        (e, root)
    }
    fn gauge(e: &mut Engine<SimHw>, id: u64, value: f64, unit: Unit) {
        e.handle(Command::SubmitReading {
            attempt_id: id,
            readings: vec![Reading {
                label: "g".into(),
                value,
                unit,
            }],
        });
    }
    const LADDER_Y: StepRequest = StepRequest::Ladder {
        axis: Axis::Y,
        sign: Sign::Neg,
        rung: Rung::N0_5,
    };

    #[test]
    fn released_ladder_with_a_zero_reading_clears_nothing() {
        let (mut e, root) = ready_engine("f1");
        e.handle(Command::Start(LADDER_Y));
        assert!(e.active().is_some(), "{}", e.status());
        for _ in 0..8 {
            e.step(true, 0.).unwrap(); // about 20 ms
        }
        e.step(false, 0.).unwrap();
        assert!(e.active().is_none());
        assert_eq!(e.done[0].record.stop.reason, "hold-released");
        gauge(&mut e, 1, 0., Unit::N);
        assert_eq!(e.done[0].record.verdict.kind, "aborted");
        assert!(e.done[0].record.verdict.unlocks.is_empty());
        assert!(
            e.session.rungs[Rung::N0_5.index()][Axis::Y.index()][Sign::Neg.index()].is_none(),
            "no verdict recorded for an aborted dwell"
        );
        assert!(!e.session.rung_cleared(Rung::N0_5, Axis::Y));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn completed_ladder_with_a_dwell_is_still_scored() {
        let (mut e, root) = ready_engine("f1ok");
        e.hardware_mut().unwrap().plant.brace();
        e.handle(Command::Start(LADDER_Y));
        while e.active().is_some() {
            e.step(true, 0.).unwrap();
        }
        assert_eq!(e.done[0].record.stop.reason, "completed");
        gauge(&mut e, 1, 0.5, Unit::N);
        assert_eq!(e.done[0].record.verdict.kind, "pass");
        assert!(e.session.rung_cleared(Rung::N0_5, Axis::Y));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn felt_or_heard_on_an_aborted_liveness_attempt_cannot_pass_it() {
        let (mut e, root) = ready_engine("f4");
        e.session.liveness = [None; 3];
        e.handle(Command::Start(StepRequest::Liveness { channel: 1 }));
        for _ in 0..3 {
            e.step(true, 0.).unwrap();
        }
        e.step(false, 0.).unwrap();
        assert_eq!(e.done[0].record.stop.reason, "hold-released");
        e.handle(Command::SubmitObservation {
            attempt_id: 1,
            felt: Some(true),
            heard: Some(true),
            note: "noise".into(),
        });
        assert_eq!(e.session.liveness[1], Some(false));
        assert_eq!(e.done[0].record.verdict.kind, "fail");
        let _ = std::fs::remove_dir_all(root);
    }

    fn settle(e: &mut Engine<SimHw>, secs: f64) {
        let dt = e.hardware_mut().unwrap().dt.as_secs_f64();
        for _ in 0..(secs / dt).ceil() as usize {
            e.step(true, 0.).unwrap();
        }
    }

    #[test]
    fn gravity_drop_is_captured_only_when_armed() {
        let (mut e, root) = ready_engine("f3a");
        e.session.checks.gravity = None;
        settle(&mut e, 0.6);
        let plant = &mut e.hardware_mut().unwrap().plant;
        plant.fixture = crate::sim::Fixture::Dropped { floor_y: -0.02 };
        settle(&mut e, 1.0);
        assert_eq!(
            e.session.checks.gravity, None,
            "an unarmed drop records nothing"
        );
        // Arm, hold still, drop again from the reset height.
        e.hardware_mut().unwrap().plant.teleport([0.; 3]);
        e.hardware_mut().unwrap().plant.float();
        settle(&mut e, 0.8);
        e.handle(Command::ArmGravityDrop);
        assert!(e.snapshot().gravity_armed, "{}", e.status());
        e.hardware_mut().unwrap().plant.fixture = crate::sim::Fixture::Dropped { floor_y: -0.02 };
        settle(&mut e, 1.0);
        assert_eq!(e.session.checks.gravity, Some(true));
        assert!(!e.snapshot().gravity_armed, "one drop per arming");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_failed_gravity_drop_survives_a_later_pass_until_reregistration() {
        let (mut e, root) = ready_engine("f3b");
        e.session.checks.gravity = None;
        settle(&mut e, 0.8);
        e.handle(Command::ArmGravityDrop);
        settle(&mut e, 0.5); // the watch anchors on the first still sample after arming
        // A quick rise reads as a +Y move: the sign is wrong.
        let plant = &mut e.hardware_mut().unwrap().plant;
        plant.teleport([0., 0.02, 0.]);
        plant.float();
        settle(&mut e, 0.6);
        assert_eq!(e.session.checks.gravity, Some(false));
        // Arming again is refused, and a real drop cannot overwrite the fail.
        e.handle(Command::ArmGravityDrop);
        assert!(!e.snapshot().gravity_armed, "{}", e.status());
        e.hardware_mut().unwrap().plant.fixture = crate::sim::Fixture::Dropped { floor_y: -0.02 };
        settle(&mut e, 1.0);
        assert_eq!(e.session.checks.gravity, Some(false));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn readings_of_the_wrong_unit_are_refused_for_the_attempt_class() {
        let (mut e, root) = ready_engine("l4");
        e.hardware_mut().unwrap().plant.brace();
        e.handle(Command::Start(LADDER_Y));
        // Running attempt: even a newton reading is held back from the dwell record.
        gauge(&mut e, 1, 20., Unit::C);
        assert!(e.status().contains("N or g"), "{}", e.status());
        while e.active().is_some() {
            e.step(true, 0.).unwrap();
        }
        gauge(&mut e, 1, 24., Unit::C);
        assert!(e.status().contains("N or g"), "{}", e.status());
        gauge(&mut e, 1, 12., Unit::Mm);
        assert!(e.done[0].record.operator.readings.is_empty());
        gauge(&mut e, 1, 0.5, Unit::N);
        assert_eq!(e.done[0].record.verdict.kind, "pass");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_rejected_request_is_recorded_as_rejected() {
        let (mut e, root) = ready_engine("l2");
        e.hardware_mut().unwrap().plant.brace();
        e.handle(Command::Start(LADDER_Y));
        for _ in 0..5 {
            e.step(true, 0.).unwrap();
        }
        // Drop the SDK registration under the running attempt: the next request is refused.
        e.hardware_mut().unwrap().clear_registration();
        for _ in 0..20 {
            if e.active().is_none() {
                break;
            }
            e.step(true, 0.).unwrap();
        }
        assert!(e.active().is_none());
        let r = &e.done[0].record.stop.reason;
        assert!(r == "request-rejected" || r == "unregistered", "{r}");
        let _ = std::fs::remove_dir_all(root);
    }
}
