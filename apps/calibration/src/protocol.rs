//! Steps, step plans, rung scoring and the per-session unlock ladder.
//!
//! The ladder is design section 2. Nothing persists across connections:
//! [`Session::reconnect`] forgets every verdict. A refused start names the first
//! missing prerequisite. Everything here is hardware-free.
use crate::registration::Checks;
use serde::{Deserialize, Serialize};
use visiontouch_device::{
    CONTINUOUS_FORCE_N, DEFAULT_DAC_OFFSET, DEFAULT_FORCE_N, Demand, ForceLimits,
    MAX_CHANNEL_TEST_OFFSET, MAX_DAC_OFFSET, MAX_SLEW_N_S,
};
use visiontouch_effects::math::{V, add, limit, sub};
use visiontouch_effects::{Effect, Frame, Kind, Tuning};

/// Standard gravity, for converting gram-force readings.
pub const G_M_S2: f64 = 9.80665;
/// Liveness drive amplitude in DAC counts (below [`MAX_CHANNEL_TEST_OFFSET`]).
pub const LIVENESS_COUNTS: i16 = 80;
/// Liveness square-wave frequency.
pub const LIVENESS_HZ: f64 = 3.;
/// Liveness duration in seconds.
pub const LIVENESS_S: f64 = 2.;
/// Sign pulse magnitude in model newtons.
pub const SIGN_PULSE_N: f64 = 0.25;
/// Lift ramp rate in N/s up to [`CONTINUOUS_FORCE_N`].
pub const LIFT_RATE_N_S: f64 = 0.05;
/// Lift ramp rate in N/s above [`CONTINUOUS_FORCE_N`], so the 10 s budget above 1.0 N holds.
pub const LIFT_RATE_ABOVE_N_S: f64 = 0.1;
/// Largest lift test force once the 1.0 N rung is cleared, newtons.
pub const LIFT_CAP_MAX_N: f64 = 1.5;
/// Thermal load, in model newtons toward -Y.
pub const THERMAL_N: f64 = 1.0;
/// Thermal duration in seconds.
pub const THERMAL_S: f64 = 600.;
/// Seconds per wall stiffness level.
pub const WALL_LEVEL_S: f64 = 3.;
/// Wall stiffness levels in N/m (0.1 to 1.5 N/mm in 0.1 N/mm steps), with their labels.
pub const WALL_LEVELS: [(f64, &str); 15] = [
    (100., "k=0.1"),
    (200., "k=0.2"),
    (300., "k=0.3"),
    (400., "k=0.4"),
    (500., "k=0.5"),
    (600., "k=0.6"),
    (700., "k=0.7"),
    (800., "k=0.8"),
    (900., "k=0.9"),
    (1000., "k=1.0"),
    (1100., "k=1.1"),
    (1200., "k=1.2"),
    (1300., "k=1.3"),
    (1400., "k=1.4"),
    (1500., "k=1.5"),
];
/// Rule shown in the evidence for ladder verdicts.
pub const RULE: &str = "pass: m >= 0.8c-0.05; over: m > 1.2c+0.05 or oscillation; else under";
const _: () = assert!(LIVENESS_COUNTS <= MAX_CHANNEL_TEST_OFFSET);

/// A Cartesian axis of the device kinematic frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Axis {
    /// Model X.
    X,
    /// Model Y, vertical: gravity acts toward -Y.
    Y,
    /// Model Z.
    Z,
}
impl Axis {
    /// All axes in order.
    pub const ALL: [Self; 3] = [Self::X, Self::Y, Self::Z];
    /// Index into a vector.
    pub fn index(self) -> usize {
        self as usize
    }
    /// Upper-case label.
    pub fn name(self) -> &'static str {
        ["X", "Y", "Z"][self.index()]
    }
    /// Unit vector along the axis.
    pub fn unit(self) -> V {
        let mut v = [0.; 3];
        v[self.index()] = 1.;
        v
    }
}

/// Direction along an axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Sign {
    /// Positive axis direction.
    Pos,
    /// Negative axis direction.
    Neg,
}
impl Sign {
    /// Both signs.
    pub const ALL: [Self; 2] = [Self::Pos, Self::Neg];
    /// +1 or -1.
    pub fn factor(self) -> f64 {
        if self == Self::Pos { 1. } else { -1. }
    }
    /// Index 0 or 1.
    pub fn index(self) -> usize {
        self as usize
    }
    /// Label.
    pub fn name(self) -> &'static str {
        ["+", "-"][self.index()]
    }
}

/// Commanded force levels of the ladder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Rung {
    /// 0.1 N.
    N0_1,
    /// 0.25 N.
    N0_25,
    /// 0.5 N.
    N0_5,
    /// 1.0 N.
    N1_0,
    /// 2.0 N.
    N2_0,
}
impl Rung {
    /// All rungs, lowest first.
    pub const ALL: [Self; 5] = [Self::N0_1, Self::N0_25, Self::N0_5, Self::N1_0, Self::N2_0];
    /// Commanded model newtons.
    pub fn newtons(self) -> f64 {
        [0.1, 0.25, 0.5, 1.0, 2.0][self as usize]
    }
    /// Index into [`Rung::ALL`].
    pub fn index(self) -> usize {
        self as usize
    }
    /// Label.
    pub fn name(self) -> &'static str {
        ["0.1 N", "0.25 N", "0.5 N", "1.0 N", "2.0 N"][self as usize]
    }
}

/// A position capture taken from a still pen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// Ruler slide start.
    RulerStart,
    /// Ruler slide end.
    RulerEnd,
    /// Return to the reset pose after moving away.
    ReturnToReset,
    /// Centre for effects.
    EffectCentre,
}

/// What the operator asks the controller to run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StepRequest {
    /// Motors off; gauge entry only.
    Backdrive,
    /// 80 count, 3 Hz square wave on one raw channel for 2 s.
    Liveness {
        /// Channel 0 to 2.
        channel: usize,
    },
    /// +0.25 N for 300 ms, 200 ms zero, -0.25 N for 300 ms.
    SignPulse {
        /// Axis tested.
        axis: Axis,
    },
    /// 0.5 s ramp, 2 s dwell, 0.5 s ramp, 1 s zero.
    Ladder {
        /// Axis loaded.
        axis: Axis,
        /// Direction.
        sign: Sign,
        /// Commanded level.
        rung: Rung,
    },
    /// +Y ramp at 0.05 N/s to 1.0 N, then 0.1 N/s to the unlocked cap, until lift-off.
    Lift {
        /// Hung mass in grams, if any.
        mass_g: Option<f64>,
    },
    /// Virtual plane, normal +Y through the start pose, with stiffness steps.
    WallRamp,
    /// -Y at 1.0 N for 600 s, braced, with a Confirm every 30 s.
    Thermal,
    /// One effect at the unlocked cap (never above 1.0 N).
    Effect {
        /// Which effect.
        kind: Kind,
    },
}

/// Steps that unlock independently, as listed in the session view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StepKind {
    /// Backdrive reading.
    Backdrive,
    /// Channel liveness.
    Liveness,
    /// Sign pulses.
    SignPulse,
    /// One ladder rung.
    Ladder(Rung),
    /// Lift test.
    Lift,
    /// Effects.
    Effect,
    /// Wall ramp.
    WallRamp,
    /// Thermal.
    Thermal,
}
impl StepKind {
    /// Operator-facing name.
    pub fn name(self) -> String {
        match self {
            Self::Backdrive => "Backdrive".into(),
            Self::Liveness => "Liveness".into(),
            Self::SignPulse => "Sign pulses".into(),
            Self::Ladder(r) => format!("Rung {}", r.name()),
            Self::Lift => "Lift test".into(),
            Self::Effect => "Effects".into(),
            Self::WallRamp => "Wall ramp".into(),
            Self::Thermal => "Thermal".into(),
        }
    }
}

/// Interlock class of a step; see `controller`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// Motors off.
    Backdrive,
    /// Raw channel test.
    Liveness,
    /// Sign pulse.
    Sign,
    /// Ladder rung.
    Ladder,
    /// Lift test.
    Lift,
    /// Effects.
    Effects,
    /// Wall ramp.
    Wall,
    /// Thermal.
    Thermal,
}

impl StepRequest {
    /// The unlock entry this request belongs to.
    pub fn kind(self) -> StepKind {
        match self {
            Self::Backdrive => StepKind::Backdrive,
            Self::Liveness { .. } => StepKind::Liveness,
            Self::SignPulse { .. } => StepKind::SignPulse,
            Self::Ladder { rung, .. } => StepKind::Ladder(rung),
            Self::Lift { .. } => StepKind::Lift,
            Self::Effect { .. } => StepKind::Effect,
            Self::WallRamp => StepKind::WallRamp,
            Self::Thermal => StepKind::Thermal,
        }
    }
    /// Interlock class.
    pub fn class(self) -> Class {
        match self {
            Self::Backdrive => Class::Backdrive,
            Self::Liveness { .. } => Class::Liveness,
            Self::SignPulse { .. } => Class::Sign,
            Self::Ladder { .. } => Class::Ladder,
            Self::Lift { .. } => Class::Lift,
            Self::Effect { .. } => Class::Effects,
            Self::WallRamp => Class::Wall,
            Self::Thermal => Class::Thermal,
        }
    }
    /// File-name slug, also the evidence `step` field.
    pub fn slug(self) -> &'static str {
        match self {
            Self::Backdrive => "backdrive",
            Self::Liveness { .. } => "liveness",
            Self::SignPulse { .. } => "sign",
            Self::Ladder { .. } => "ladder",
            Self::Lift { .. } => "lift",
            Self::Effect { .. } => "effect",
            Self::WallRamp => "wall",
            Self::Thermal => "thermal",
        }
    }
    /// Short label for the interface.
    pub fn label(self) -> String {
        match self {
            Self::Backdrive => "Backdrive".into(),
            Self::Liveness { channel } => format!("Liveness ch{channel}"),
            Self::SignPulse { axis } => format!("Sign pulse {}", axis.name()),
            Self::Ladder { axis, sign, rung } => {
                format!("Rung {} {}{}", rung.name(), sign.name(), axis.name())
            }
            Self::Lift { mass_g: None } => "Lift, bare".into(),
            Self::Lift { mass_g: Some(m) } => format!("Lift, {m:.0} g"),
            Self::WallRamp => "Wall ramp".into(),
            Self::Thermal => "Thermal".into(),
            Self::Effect { kind } => format!("Effect: {}", kind.name()),
        }
    }
    /// Parameters for the evidence file.
    pub fn params(self) -> serde_json::Value {
        use serde_json::json;
        match self {
            Self::Backdrive => json!({}),
            Self::Liveness { channel } => json!({
                "channel": channel, "counts": LIVENESS_COUNTS, "hz": LIVENESS_HZ, "duration_s": LIVENESS_S
            }),
            Self::SignPulse { axis } => json!({
                "axis": axis, "pulse_n": SIGN_PULSE_N, "pulse_s": 0.3, "gap_s": 0.2
            }),
            Self::Ladder { axis, sign, rung } => json!({
                "axis": axis, "sign": sign, "rung_n": rung.newtons(),
                "ramp_s": 0.5, "dwell_s": 2.0
            }),
            Self::Lift { mass_g } => json!({
                "mass_g": mass_g, "rate_n_s": LIFT_RATE_N_S,
                "rate_above_1n_n_s": LIFT_RATE_ABOVE_N_S
            }),
            Self::WallRamp => json!({
                "levels_n_m": WALL_LEVELS.map(|l| l.0), "level_s": WALL_LEVEL_S
            }),
            Self::Thermal => json!({ "force_n": THERMAL_N, "duration_s": THERMAL_S }),
            Self::Effect { kind } => json!({ "kind": kind.name() }),
        }
    }
}

/// Outcome of a rung.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    /// Within the acceptance window.
    Pass,
    /// Above the window or oscillating: blocks the next rung on that axis.
    Over,
    /// Below the window: recorded with its slope, not blocking.
    Under,
}

/// Score a rung: `m` measured, `c` commanded, both in newtons.
pub fn score_rung(commanded_n: f64, measured_n: f64, oscillation: bool) -> Verdict {
    if oscillation || measured_n > 1.2 * commanded_n + 0.05 + 1e-9 {
        Verdict::Over
    } else if measured_n >= 0.8 * commanded_n - 0.05 - 1e-9 {
        Verdict::Pass
    } else {
        Verdict::Under
    }
}

/// Unit of an operator reading.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unit {
    /// Newtons.
    N,
    /// Grams (gram-force on a scale).
    G,
    /// Degrees Celsius.
    C,
    /// Millimetres.
    Mm,
}

/// One labelled operator reading.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reading {
    /// What was read, for example "dwell".
    pub label: String,
    /// Numeric value in `unit`.
    pub value: f64,
    /// Unit.
    pub unit: Unit,
}
impl Reading {
    /// Force in newtons for N and G readings.
    pub fn newtons(&self) -> Option<f64> {
        match self.unit {
            Unit::N => Some(self.value),
            Unit::G => Some(self.value * 1e-3 * G_M_S2),
            _ => None,
        }
    }
}

/// Score a sign pulse from `(time, position)` rows: the positive pulse must move the
/// pen toward +axis and the negative pulse back. `None` means too small to tell.
pub fn score_sign(axis: Axis, rows: &[(f64, V)]) -> Option<bool> {
    let first = rows.first()?;
    let last = rows.last()?;
    let mid = rows.iter().find(|r| r.0 >= 0.5).unwrap_or(last);
    let i = axis.index();
    let (d_pos, d_neg) = (mid.1[i] - first.1[i], last.1[i] - mid.1[i]);
    let diff = d_pos - d_neg;
    (diff.abs() >= 0.0002).then_some(diff > 0.)
}

/// Limits and cap granted by a successful [`Session::check`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Allowance {
    /// Limits to configure while disarmed.
    pub limits: ForceLimits,
    /// Largest Cartesian force the plan may command, newtons.
    pub cap_n: f64,
}

fn limits_for(force_n: f64) -> ForceLimits {
    if force_n <= DEFAULT_FORCE_N + 1e-9 {
        ForceLimits {
            force_n: DEFAULT_FORCE_N,
            slew_n_s: MAX_SLEW_N_S,
            dac_counts: DEFAULT_DAC_OFFSET,
        }
    } else {
        ForceLimits {
            force_n,
            slew_n_s: MAX_SLEW_N_S,
            dac_counts: MAX_DAC_OFFSET,
        }
    }
}

/// Lift results: F_0 per hung mass, and the gain derived from them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LiftLog {
    /// `(mass_g, liftoff_force_n)` entries; bare is mass 0.
    pub entries: Vec<(f64, f64)>,
}
impl LiftLog {
    /// Bare lift-off force.
    pub fn f0(&self) -> Option<f64> {
        self.entries.iter().find(|e| e.0 == 0.).map(|e| e.1)
    }
    /// Mean commanded newtons per newton of added weight.
    pub fn gain(&self) -> Option<f64> {
        let f0 = self.f0()?;
        let g: Vec<f64> = self
            .entries
            .iter()
            .filter(|e| e.0 > 0.)
            .map(|e| (e.1 - f0) / (e.0 * 1e-3 * G_M_S2))
            .collect();
        (!g.is_empty()).then(|| g.iter().sum::<f64>() / g.len() as f64)
    }
}

/// Per-connection unlock state.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Session {
    /// Device connection is open.
    pub connected: bool,
    /// A reset pose has been registered on this connection.
    pub registered: bool,
    /// Liveness verdicts per channel (felt, heard or lock-in).
    pub liveness: [Option<bool>; 3],
    /// Registration checks.
    pub checks: Checks,
    /// Sign pulse verdicts per axis.
    pub sign: [Option<bool>; 3],
    /// Rung verdicts indexed `[rung][axis][sign]`.
    pub rungs: [[[Option<Verdict>; 2]; 3]; 5],
    /// Lift test log.
    pub lift: LiftLog,
    /// Wall stiffness at which oscillation began, N/mm.
    pub k_marginal_n_mm: Option<f64>,
    /// Thermal test finished without a stop.
    pub thermal: Option<bool>,
    /// Unlock events in order, for the session file.
    pub history: Vec<String>,
}

impl Session {
    /// A new connected session.
    pub fn connected() -> Self {
        Self {
            connected: true,
            ..Self::default()
        }
    }
    /// Forget every verdict: nothing unlocked survives a reconnect.
    pub fn reconnect(&mut self) {
        *self = Self::connected();
    }
    /// Close the session.
    pub fn disconnect(&mut self) {
        *self = Self::default();
    }
    /// A rung is cleared on an axis when at least one sign has a non-Over verdict and
    /// none is Over. Under is recorded, not blocking.
    pub fn rung_cleared(&self, rung: Rung, axis: Axis) -> bool {
        let v = &self.rungs[rung.index()][axis.index()];
        v.iter().flatten().all(|x| *x != Verdict::Over) && v.iter().any(Option::is_some)
    }
    /// A rung is cleared on all three axes.
    pub fn rung_cleared_all(&self, rung: Rung) -> bool {
        Axis::ALL.iter().all(|a| self.rung_cleared(rung, *a))
    }
    fn rung_missing(&self, rung: Rung) -> Option<String> {
        for a in Axis::ALL {
            let v = &self.rungs[rung.index()][a.index()];
            if v.iter().flatten().any(|x| *x == Verdict::Over) {
                return Some(format!(
                    "{} rung is Over on {}; it blocks the next rung",
                    rung.name(),
                    a.name()
                ));
            }
            if !self.rung_cleared(rung, a) {
                return Some(format!(
                    "{} rung on {} (one sign minimum)",
                    rung.name(),
                    a.name()
                ));
            }
        }
        None
    }
    /// The session-wide lock: any Over verdict on any rung, axis or sign stops every
    /// force step until the reset pose is registered again or the device reconnects.
    fn over_lock(&self) -> Option<String> {
        for rung in Rung::ALL {
            for axis in Axis::ALL {
                let v = &self.rungs[rung.index()][axis.index()];
                if v.iter().flatten().any(|x| *x == Verdict::Over) {
                    return Some(format!(
                        "no Over verdict: the {} rung is Over on {}; the session is locked until re-registration",
                        rung.name(),
                        axis.name()
                    ));
                }
            }
        }
        None
    }
    /// Liveness, registration and registration checks; the base of every force step.
    fn base_missing(&self) -> Option<String> {
        if let Some(m) = self.over_lock() {
            return Some(m);
        }
        if !self.registered {
            return Some("reset-pose registration".into());
        }
        if let Some(c) = (0..3).find(|c| self.liveness[*c] != Some(true)) {
            return Some(format!("liveness pass on channel {c}"));
        }
        self.checks.missing()
    }
    fn sign_missing(&self) -> Option<String> {
        self.base_missing().or_else(|| {
            Axis::ALL
                .iter()
                .find(|a| self.sign[a.index()] != Some(true))
                .map(|a| format!("sign pulse pass on {}", a.name()))
        })
    }
    /// Largest unlocked force for the lift test, newtons; zero while locked.
    pub fn lift_cap_n(&self) -> f64 {
        if self.sign_missing().is_some() {
            0.
        } else if self.rung_missing(Rung::N0_5).is_some() {
            DEFAULT_FORCE_N
        } else if self.rung_missing(Rung::N1_0).is_some() {
            CONTINUOUS_FORCE_N
        } else {
            LIFT_CAP_MAX_N
        }
    }
    /// Largest unlocked force for effects, wall and thermal: never above 1.0 N.
    pub fn effects_cap_n(&self) -> f64 {
        if self.sign_missing().is_some() {
            0.
        } else if self.rung_missing(Rung::N1_0).is_some() {
            DEFAULT_FORCE_N
        } else {
            CONTINUOUS_FORCE_N
        }
    }
    /// Decide whether a step may start now. The error names the missing prerequisite.
    pub fn check(&self, req: StepRequest) -> Result<Allowance, String> {
        if !self.connected {
            return Err("Connect first.".into());
        }
        let need = |m: Option<String>| m.map_or(Ok(()), |m| Err(format!("Locked: needs {m}.")));
        match req {
            StepRequest::Backdrive => Ok(Allowance {
                limits: ForceLimits::default(),
                cap_n: 0.,
            }),
            StepRequest::Liveness { channel } => {
                if channel > 2 {
                    return Err("Channel must be 0, 1 or 2.".into());
                }
                Ok(Allowance {
                    limits: ForceLimits::default(),
                    cap_n: 0.,
                })
            }
            StepRequest::SignPulse { .. } => {
                need(self.base_missing())?;
                Ok(Allowance {
                    limits: limits_for(DEFAULT_FORCE_N),
                    cap_n: DEFAULT_FORCE_N,
                })
            }
            StepRequest::Ladder { rung, .. } => {
                need(self.sign_missing())?;
                match rung {
                    Rung::N1_0 => need(self.rung_missing(Rung::N0_5))?,
                    Rung::N2_0 => {
                        need(self.rung_missing(Rung::N0_5))?;
                        need(self.rung_missing(Rung::N1_0))?;
                    }
                    _ => {}
                }
                Ok(Allowance {
                    limits: limits_for(rung.newtons().max(DEFAULT_FORCE_N)),
                    cap_n: rung.newtons(),
                })
            }
            StepRequest::Lift { .. } => {
                need(self.sign_missing())?;
                let cap = self.lift_cap_n();
                Ok(Allowance {
                    limits: limits_for(cap),
                    cap_n: cap,
                })
            }
            StepRequest::Effect { .. } => {
                need(self.sign_missing())?;
                let cap = self.effects_cap_n();
                Ok(Allowance {
                    limits: limits_for(cap),
                    cap_n: cap,
                })
            }
            StepRequest::WallRamp | StepRequest::Thermal => {
                need(self.sign_missing())?;
                need(self.rung_missing(Rung::N0_5))?;
                need(self.rung_missing(Rung::N1_0))?;
                Ok(Allowance {
                    limits: limits_for(CONTINUOUS_FORCE_N),
                    cap_n: CONTINUOUS_FORCE_N,
                })
            }
        }
    }
    /// Every step kind that would be accepted now.
    pub fn unlocked(&self) -> Vec<StepKind> {
        let probes = [
            StepRequest::Backdrive,
            StepRequest::Liveness { channel: 0 },
            StepRequest::SignPulse { axis: Axis::X },
            StepRequest::Ladder {
                axis: Axis::Y,
                sign: Sign::Neg,
                rung: Rung::N0_1,
            },
            StepRequest::Ladder {
                axis: Axis::Y,
                sign: Sign::Neg,
                rung: Rung::N0_25,
            },
            StepRequest::Ladder {
                axis: Axis::Y,
                sign: Sign::Neg,
                rung: Rung::N0_5,
            },
            StepRequest::Ladder {
                axis: Axis::Y,
                sign: Sign::Neg,
                rung: Rung::N1_0,
            },
            StepRequest::Ladder {
                axis: Axis::Y,
                sign: Sign::Neg,
                rung: Rung::N2_0,
            },
            StepRequest::Lift { mass_g: None },
            StepRequest::Effect { kind: Kind::Cube },
            StepRequest::WallRamp,
            StepRequest::Thermal,
        ];
        probes
            .into_iter()
            .filter(|p| self.check(*p).is_ok())
            .map(StepRequest::kind)
            .collect()
    }
    fn log_unlocks(&mut self, before: &[StepKind], why: &str) -> Vec<String> {
        let new: Vec<String> = self
            .unlocked()
            .into_iter()
            .filter(|k| !before.contains(k))
            .map(StepKind::name)
            .collect();
        for n in &new {
            self.history.push(format!("{n} unlocked by {why}"));
        }
        new
    }
    /// Record a liveness verdict. Returns newly unlocked step names.
    pub fn record_liveness(&mut self, channel: usize, pass: bool) -> Vec<String> {
        let before = self.unlocked();
        if channel < 3 {
            self.liveness[channel] = Some(pass);
        }
        self.log_unlocks(&before, &format!("liveness ch{channel}"))
    }
    /// Mark registration captured on this connection.
    pub fn record_registered(&mut self, registered: bool) -> Vec<String> {
        let before = self.unlocked();
        self.registered = registered;
        if !registered {
            self.checks = Checks::default();
            self.sign = [None; 3];
            self.rungs = Default::default();
        }
        self.log_unlocks(&before, "registration")
    }
    /// Re-evaluate unlocks after the registration checks changed.
    pub fn checks_changed(&mut self, before: &[StepKind]) -> Vec<String> {
        self.log_unlocks(before, "registration checks")
    }
    /// Record a sign pulse verdict.
    pub fn record_sign(&mut self, axis: Axis, pass: bool) -> Vec<String> {
        let before = self.unlocked();
        self.sign[axis.index()] = Some(pass);
        self.log_unlocks(&before, &format!("sign pulse {}", axis.name()))
    }
    /// Record a rung verdict.
    pub fn record_rung(&mut self, rung: Rung, axis: Axis, sign: Sign, v: Verdict) -> Vec<String> {
        let before = self.unlocked();
        // Over is sticky for the connection: a retest or a later reading cannot erase it.
        let slot = &mut self.rungs[rung.index()][axis.index()][sign.index()];
        if *slot != Some(Verdict::Over) {
            *slot = Some(v);
        }
        self.log_unlocks(
            &before,
            &format!("rung {} {}{}", rung.name(), sign.name(), axis.name()),
        )
    }
    /// Record a lift-off force for a hung mass (0 for bare).
    pub fn record_lift(&mut self, mass_g: f64, f0_n: f64) {
        self.lift.entries.retain(|e| e.0 != mass_g);
        self.lift.entries.push((mass_g, f0_n));
    }
}

/// A force-versus-time profile for one step. Time is seconds from the start.
pub struct Plan {
    req: StepRequest,
    cap_n: f64,
    start: V,
    centre: V,
    effect: Option<Effect>,
    /// The latest effect frame, for the interface.
    pub frame: Option<Frame>,
    /// Wall stiffness level in force, N/m.
    pub wall_k_n_m: Option<f64>,
    wall_level: Option<usize>,
}

/// Lift force at time `t`: 0.05 N/s up to 1.0 N, then 0.1 N/s.
fn lift_force_at(t: f64) -> f64 {
    let knee_s = CONTINUOUS_FORCE_N / LIFT_RATE_N_S;
    if t <= knee_s {
        t * LIFT_RATE_N_S
    } else {
        CONTINUOUS_FORCE_N + (t - knee_s) * LIFT_RATE_ABOVE_N_S
    }
}
/// Time at which the lift ramp reaches `cap_n`.
fn lift_ramp_end_s(cap_n: f64) -> f64 {
    let knee_s = CONTINUOUS_FORCE_N / LIFT_RATE_N_S;
    if cap_n <= CONTINUOUS_FORCE_N {
        cap_n / LIFT_RATE_N_S
    } else {
        knee_s + (cap_n - CONTINUOUS_FORCE_N) / LIFT_RATE_ABOVE_N_S
    }
}

impl Plan {
    /// Build the plan. `start` is the registered pen position when the attempt begins;
    /// `centre` overrides the effect centre.
    pub fn new(req: StepRequest, cap_n: f64, start: V, centre: Option<V>) -> Self {
        let effect = match req {
            StepRequest::Effect { kind } => Some(Effect::new(kind, 0.0225, Tuning::default())),
            _ => None,
        };
        Self {
            req,
            cap_n,
            start,
            centre: centre.unwrap_or(start),
            effect,
            frame: None,
            wall_k_n_m: None,
            wall_level: None,
        }
    }
    /// Index into [`WALL_LEVELS`] of the stiffness in force, once the wall runs.
    #[cfg(test)]
    pub fn wall_level(&self) -> Option<usize> {
        self.wall_level
    }
    /// Stiffness in N/mm of the last level that held without oscillation, if any.
    pub fn last_stable_k_n_mm(&self) -> Option<f64> {
        let level = self.wall_level?.checked_sub(1)?;
        Some(WALL_LEVELS[level].0 / 1000.)
    }
    /// Fixed duration; `None` for open-ended steps (backdrive and effects).
    pub fn duration_s(&self) -> Option<f64> {
        match self.req {
            StepRequest::Backdrive | StepRequest::Effect { .. } => None,
            StepRequest::Liveness { .. } => Some(LIVENESS_S),
            StepRequest::SignPulse { .. } => Some(1.0),
            StepRequest::Ladder { .. } => Some(4.0),
            StepRequest::Lift { .. } => Some(lift_ramp_end_s(self.cap_n) + 1.),
            StepRequest::WallRamp => Some(WALL_LEVEL_S * WALL_LEVELS.len() as f64),
            StepRequest::Thermal => Some(THERMAL_S),
        }
    }
    /// Effect centre, registered metres.
    pub fn centre(&self) -> V {
        self.centre
    }
    /// The target the pen is being driven toward or held against, if any.
    pub fn target(&self) -> Option<V> {
        match self.req {
            StepRequest::Effect { .. } => self.frame.map(|f| add(self.centre, f.proxy)),
            StepRequest::WallRamp => Some(self.start),
            _ => None,
        }
    }
    /// The demand and the phase name at time `t`. `pos` and `vel` are registered
    /// metres and metres per second; `dt` is the servo interval.
    pub fn demand(&mut self, t: f64, dt: f64, pos: V, vel: V) -> (Demand, &'static str) {
        let zero = Demand::Force([0.; 3]);
        match self.req {
            StepRequest::Backdrive => (zero, "backdrive"),
            StepRequest::Liveness { channel } => {
                if t >= LIVENESS_S {
                    return (Demand::Channels([0; 3]), "zero");
                }
                let hi = ((t * LIVENESS_HZ * 2.).floor() as i64) % 2 == 0;
                let mut c = [0_i16; 3];
                c[channel.min(2)] = if hi {
                    LIVENESS_COUNTS
                } else {
                    -LIVENESS_COUNTS
                };
                (
                    Demand::Channels(c),
                    if hi { "square_hi" } else { "square_lo" },
                )
            }
            StepRequest::SignPulse { axis } => {
                let (f, phase) = if t < 0.3 {
                    (SIGN_PULSE_N, "pulse_pos")
                } else if t < 0.5 {
                    (0., "zero")
                } else if t < 0.8 {
                    (-SIGN_PULSE_N, "pulse_neg")
                } else {
                    (0., "settle")
                };
                (Demand::Force(axis.unit().map(|u| u * f)), phase)
            }
            StepRequest::Ladder { axis, sign, rung } => {
                let c = rung.newtons() * sign.factor();
                let (f, phase) = if t < 0.5 {
                    (t / 0.5, "ramp")
                } else if t < 2.5 {
                    (1., "dwell")
                } else if t < 3.0 {
                    ((3.0 - t) / 0.5, "ramp_down")
                } else {
                    (0., "zero")
                };
                (Demand::Force(axis.unit().map(|u| u * c * f)), phase)
            }
            StepRequest::Lift { .. } => {
                let f = lift_force_at(t).min(self.cap_n);
                let phase = if f < self.cap_n { "ramp" } else { "hold" };
                (Demand::Force([0., f, 0.]), phase)
            }
            StepRequest::WallRamp => {
                let level = ((t / WALL_LEVEL_S).floor() as usize).min(WALL_LEVELS.len() - 1);
                let (k, phase) = WALL_LEVELS[level];
                self.wall_k_n_m = Some(k);
                self.wall_level = Some(level);
                let penetration = (self.start[1] - pos[1]).max(0.);
                let f = (k * penetration).min(self.cap_n);
                (Demand::Force([0., f, 0.]), phase)
            }
            StepRequest::Thermal => {
                if t < 1. {
                    (Demand::Force([0., -THERMAL_N * t, 0.]), "ramp")
                } else {
                    (Demand::Force([0., -THERMAL_N, 0.]), "hold")
                }
            }
            StepRequest::Effect { .. } => {
                let Some(effect) = self.effect.as_mut() else {
                    return (zero, "zero");
                };
                let frame = effect.step(sub(pos, self.centre), vel, dt.clamp(1e-4, 0.025));
                self.frame = Some(frame);
                (Demand::Force(limit(frame.force, self.cap_n)), "effect")
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use visiontouch_effects::math::norm;

    fn demand_at(p: &mut Plan, t: f64) -> (Demand, &'static str) {
        p.demand(t, 0.0024, [0.; 3], [0.; 3])
    }
    fn force(d: Demand) -> V {
        match d {
            Demand::Force(f) => f,
            Demand::Channels(_) => panic!("channels"),
        }
    }
    fn unlocked_unlocked_to_one() -> Session {
        let mut s = unlocked_session();
        for a in Axis::ALL {
            s.record_sign(a, true);
            s.record_rung(Rung::N0_5, a, Sign::Pos, Verdict::Pass);
            s.record_rung(Rung::N1_0, a, Sign::Pos, Verdict::Pass);
        }
        s
    }
    pub(crate) fn unlocked_session() -> Session {
        let mut s = Session::connected();
        s.record_registered(true);
        for c in 0..3 {
            s.record_liveness(c, true);
        }
        s.checks.returns.push([0.; 3]);
        s.checks.return_error_mm = Some(0.5);
        s.checks.gravity = Some(true);
        for a in Axis::ALL {
            s.checks.ruler[a.index()] = Some(crate::registration::RulerResult {
                axis: a,
                measured_mm: 50.,
                model_mm: 50.,
                scale: 1.,
                off_axis: 0.,
                sign_ok: true,
                off_axis_ok: true,
                scale_warning: false,
            });
        }
        s
    }

    #[test]
    fn liveness_is_a_3_hz_square_wave_of_80_counts_for_2_s() {
        let mut p = Plan::new(StepRequest::Liveness { channel: 1 }, 0., [0.; 3], None);
        assert_eq!(p.duration_s(), Some(2.));
        assert_eq!(
            demand_at(&mut p, 0.01),
            (Demand::Channels([0, 80, 0]), "square_hi")
        );
        assert_eq!(
            demand_at(&mut p, 0.2),
            (Demand::Channels([0, -80, 0]), "square_lo")
        );
        assert_eq!(
            demand_at(&mut p, 0.34),
            (Demand::Channels([0, 80, 0]), "square_hi")
        );
        assert_eq!(demand_at(&mut p, 2.1).0, Demand::Channels([0; 3]));
    }

    #[test]
    fn sign_pulse_profile() {
        let mut p = Plan::new(StepRequest::SignPulse { axis: Axis::Z }, 0.5, [0.; 3], None);
        assert_eq!(force(demand_at(&mut p, 0.1).0), [0., 0., 0.25]);
        assert_eq!(demand_at(&mut p, 0.4).1, "zero");
        assert_eq!(force(demand_at(&mut p, 0.6).0), [0., 0., -0.25]);
        assert_eq!(force(demand_at(&mut p, 0.9).0), [0.; 3]);
        assert_eq!(p.duration_s(), Some(1.0));
    }

    #[test]
    fn ladder_profile_is_ramp_dwell_ramp_zero() {
        let req = StepRequest::Ladder {
            axis: Axis::Y,
            sign: Sign::Neg,
            rung: Rung::N0_5,
        };
        let mut p = Plan::new(req, 0.5, [0.; 3], None);
        assert_eq!(p.duration_s(), Some(4.0));
        assert!((force(demand_at(&mut p, 0.25).0)[1] + 0.25).abs() < 1e-12);
        assert_eq!(
            demand_at(&mut p, 1.0),
            (Demand::Force([0., -0.5, 0.]), "dwell")
        );
        assert!((force(demand_at(&mut p, 2.75).0)[1] + 0.25).abs() < 1e-12);
        assert_eq!(demand_at(&mut p, 3.5), (Demand::Force([0.; 3]), "zero"));
    }

    #[test]
    fn lift_ramps_at_005_n_s_to_the_cap() {
        let mut p = Plan::new(StepRequest::Lift { mass_g: None }, 0.5, [0.; 3], None);
        assert_eq!(p.duration_s(), Some(11.));
        assert!((force(demand_at(&mut p, 4.).0)[1] - 0.2).abs() < 1e-12);
        assert_eq!(
            demand_at(&mut p, 10.5),
            (Demand::Force([0., 0.5, 0.]), "hold")
        );
    }

    #[test]
    fn wall_pushes_only_below_the_plane_and_steps_stiffness() {
        let mut p = Plan::new(StepRequest::WallRamp, 1.0, [0., 0.05, 0.], None);
        let above = p.demand(1., 0.0024, [0., 0.051, 0.], [0.; 3]);
        assert_eq!(force(above.0), [0.; 3]);
        let (d, phase) = p.demand(7., 0.0024, [0., 0.049, 0.], [0.; 3]);
        assert_eq!(phase, "k=0.3");
        assert!((force(d)[1] - 0.3).abs() < 1e-9, "1 mm at 300 N/m");
        let (d, phase) = p.demand(31., 0.0024, [0., 0.049, 0.], [0.; 3]);
        assert_eq!(phase, "k=1.1");
        assert!((force(d)[1] - 1.0).abs() < 1e-9, "capped at 1 N");
    }

    #[test]
    fn thermal_is_minus_y_one_newton_for_600_s() {
        let mut p = Plan::new(StepRequest::Thermal, 1.0, [0.; 3], None);
        assert_eq!(p.duration_s(), Some(600.));
        assert_eq!(
            demand_at(&mut p, 5.),
            (Demand::Force([0., -1., 0.]), "hold")
        );
    }

    #[test]
    fn effects_are_capped_at_the_unlocked_cap() {
        let mut p = Plan::new(StepRequest::Effect { kind: Kind::Cube }, 0.5, [0.; 3], None);
        let (d, _) = p.demand(1.1, 0.002, [0.01, 0.01, 0.01], [0.; 3]);
        assert!(norm(force(d)) <= 0.5 + 1e-12);
        assert!(p.frame.is_some());
    }

    #[test]
    fn rung_scoring_rule() {
        assert_eq!(score_rung(0.5, 0.47, false), Verdict::Pass);
        assert_eq!(
            score_rung(0.5, 0.35, false),
            Verdict::Pass,
            "0.8c-0.05 boundary"
        );
        assert_eq!(score_rung(0.5, 0.34, false), Verdict::Under);
        assert_eq!(score_rung(0.5, 0.65, false), Verdict::Pass);
        assert_eq!(score_rung(0.5, 0.66, false), Verdict::Over);
        assert_eq!(score_rung(0.5, 0.45, true), Verdict::Over, "oscillation");
        assert_eq!(
            Reading {
                label: "x".into(),
                value: 50.,
                unit: Unit::G
            }
            .newtons()
            .map(|n| (n * 1000.).round()),
            Some(490.)
        );
    }

    #[test]
    fn sign_scoring_needs_a_visible_difference() {
        let rows: Vec<(f64, V)> = (0..=100)
            .map(|i| {
                let t = f64::from(i) * 0.01;
                (
                    t,
                    [
                        0.,
                        0.,
                        if t < 0.5 {
                            0.001 * t
                        } else {
                            0.0005 - 0.001 * (t - 0.5)
                        },
                    ],
                )
            })
            .collect();
        assert_eq!(score_sign(Axis::Z, &rows), Some(true));
        let flat: Vec<(f64, V)> = (0..=100).map(|i| (f64::from(i) * 0.01, [0.; 3])).collect();
        assert_eq!(score_sign(Axis::Z, &flat), None);
        let inverted: Vec<(f64, V)> = rows.iter().map(|r| (r.0, [0., 0., -r.1[2]])).collect();
        assert_eq!(score_sign(Axis::Z, &inverted), Some(false));
    }

    #[test]
    fn refusals_name_the_missing_prerequisite() {
        let mut s = Session::connected();
        let ladder = |rung| StepRequest::Ladder {
            axis: Axis::Y,
            sign: Sign::Neg,
            rung,
        };
        let e = s.check(ladder(Rung::N0_1)).unwrap_err();
        assert!(e.contains("registration"), "{e}");
        assert!(
            s.check(StepRequest::Liveness { channel: 0 }).is_ok(),
            "liveness needs only a connection"
        );
        assert!(
            Session::default()
                .check(StepRequest::Backdrive)
                .unwrap_err()
                .contains("Connect")
        );
        s.record_registered(true);
        let e = s
            .check(StepRequest::SignPulse { axis: Axis::X })
            .unwrap_err();
        assert!(e.contains("liveness pass on channel 0"), "{e}");
        s.record_liveness(0, true);
        s.record_liveness(1, true);
        s.record_liveness(2, false);
        let e = s
            .check(StepRequest::SignPulse { axis: Axis::X })
            .unwrap_err();
        assert!(e.contains("channel 2"), "{e}");
        s.record_liveness(2, true);
        let e = s
            .check(StepRequest::SignPulse { axis: Axis::X })
            .unwrap_err();
        assert!(e.contains("return-to-reset"), "{e}");
    }

    #[test]
    fn unlock_ladder_follows_section_two_exactly() {
        let mut s = unlocked_session();
        let ladder = |rung| StepRequest::Ladder {
            axis: Axis::Y,
            sign: Sign::Neg,
            rung,
        };
        // Sign pulses unlock after liveness, registration and checks.
        let a = s.check(StepRequest::SignPulse { axis: Axis::X }).unwrap();
        assert_eq!((a.limits.force_n, a.limits.dac_counts), (0.5, 96));
        assert!(
            s.check(ladder(Rung::N0_5))
                .unwrap_err()
                .contains("sign pulse pass on X")
        );
        assert!(s.check(StepRequest::Effect { kind: Kind::Cube }).is_err());
        for a in Axis::ALL {
            s.record_sign(a, true);
        }
        for r in [Rung::N0_1, Rung::N0_25, Rung::N0_5] {
            assert!(s.check(ladder(r)).is_ok());
        }
        assert_eq!(
            s.check(StepRequest::Effect { kind: Kind::Cube })
                .unwrap()
                .cap_n,
            0.5
        );
        // Cannot reach 1.0 N without three 0.5 N passes.
        let e = s.check(ladder(Rung::N1_0)).unwrap_err();
        assert!(e.contains("0.5 N rung on X"), "{e}");
        s.record_rung(Rung::N0_5, Axis::X, Sign::Pos, Verdict::Pass);
        s.record_rung(Rung::N0_5, Axis::Y, Sign::Neg, Verdict::Under);
        assert!(s.check(ladder(Rung::N1_0)).unwrap_err().contains("on Z"));
        let new = s.record_rung(Rung::N0_5, Axis::Z, Sign::Pos, Verdict::Pass);
        assert!(new.iter().any(|n| n.contains("1.0 N")), "{new:?}");
        let a = s.check(ladder(Rung::N1_0)).unwrap();
        assert_eq!((a.limits.force_n, a.limits.dac_counts), (1.0, 300));
        assert_eq!(s.lift_cap_n(), 1.0);
        assert!(s.check(ladder(Rung::N2_0)).is_err());
        assert!(s.check(StepRequest::WallRamp).is_err());
        assert!(s.check(StepRequest::Thermal).is_err());
        assert_eq!(s.effects_cap_n(), 0.5);
        // 1.0 N rung on all axes unlocks 2.0 N, wall, thermal and 1.0 N effects.
        for a in Axis::ALL {
            s.record_rung(Rung::N1_0, a, Sign::Neg, Verdict::Pass);
        }
        let a = s.check(ladder(Rung::N2_0)).unwrap();
        assert_eq!((a.limits.force_n, a.limits.dac_counts), (2.0, 300));
        assert_eq!(s.lift_cap_n(), 1.5);
        assert_eq!(s.effects_cap_n(), 1.0, "effects never above 1.0 N");
        assert_eq!(s.check(StepRequest::WallRamp).unwrap().cap_n, 1.0);
        assert!(s.check(StepRequest::Thermal).is_ok());
        assert!(s.unlocked().contains(&StepKind::Ladder(Rung::N2_0)));
    }

    #[test]
    fn over_blocks_the_next_rung_and_reconnect_resets_everything() {
        let mut s = unlocked_session();
        for a in Axis::ALL {
            s.record_sign(a, true);
            s.record_rung(Rung::N0_5, a, Sign::Pos, Verdict::Pass);
        }
        s.record_rung(Rung::N0_5, Axis::Z, Sign::Neg, Verdict::Over);
        let e = s
            .check(StepRequest::Ladder {
                axis: Axis::X,
                sign: Sign::Pos,
                rung: Rung::N1_0,
            })
            .unwrap_err();
        assert!(e.contains("Over on Z"), "{e}");
        assert!(!s.history.is_empty(), "unlock events were logged");
        s.reconnect();
        assert!(
            s.history.is_empty()
                && s.connected
                && !s.registered
                && s.sign == [None; 3]
                && s.liveness == [None; 3]
        );
        assert!(s.rungs.iter().flatten().flatten().all(Option::is_none));
        assert_eq!(s.unlocked(), vec![StepKind::Backdrive, StepKind::Liveness]);
    }

    #[test]
    fn any_over_latches_every_force_step_until_reregistration() {
        let mut s = unlocked_session();
        for a in Axis::ALL {
            s.record_sign(a, true);
        }
        s.record_rung(Rung::N0_25, Axis::Y, Sign::Neg, Verdict::Over);
        let steps = [
            StepRequest::SignPulse { axis: Axis::X },
            StepRequest::Lift { mass_g: None },
            StepRequest::Effect { kind: Kind::Cube },
            StepRequest::WallRamp,
            StepRequest::Thermal,
        ];
        for step in steps {
            let e = s.check(step).unwrap_err();
            assert!(
                e.contains("0.25 N") && e.contains("Over on Y"),
                "{step:?}: {e}"
            );
        }
        for axis in Axis::ALL {
            for rung in Rung::ALL {
                let e = s
                    .check(StepRequest::Ladder {
                        axis,
                        sign: Sign::Pos,
                        rung,
                    })
                    .unwrap_err();
                assert!(e.contains("Over on Y"), "{axis:?} {rung:?}: {e}");
            }
        }
        assert_eq!((s.lift_cap_n(), s.effects_cap_n()), (0., 0.));
        // Liveness and backdrive stay available so the operator can still diagnose.
        assert!(s.check(StepRequest::Backdrive).is_ok());
        s.record_registered(false);
        assert!(s.rungs.iter().flatten().flatten().all(Option::is_none));
    }

    #[test]
    fn lift_ramp_is_slow_to_one_newton_then_faster_to_the_cap() {
        let mut p = Plan::new(StepRequest::Lift { mass_g: None }, 1.5, [0.; 3], None);
        assert_eq!(p.duration_s(), Some(26.));
        assert!((force(demand_at(&mut p, 10.).0)[1] - 0.5).abs() < 1e-12);
        assert!((force(demand_at(&mut p, 20.).0)[1] - 1.0).abs() < 1e-12);
        assert!((force(demand_at(&mut p, 23.).0)[1] - 1.3).abs() < 1e-12);
        assert_eq!(demand_at(&mut p, 25.5).1, "hold");
        let s = unlocked_unlocked_to_one();
        assert_eq!(s.lift_cap_n(), 1.5);
    }

    #[test]
    fn wall_steps_are_tenths_of_a_newton_per_mm_for_three_seconds() {
        assert_eq!(WALL_LEVELS.len(), 15);
        assert_eq!(WALL_LEVEL_S, 3.);
        let mut p = Plan::new(StepRequest::WallRamp, 1.0, [0., 0.05, 0.], None);
        assert_eq!(p.duration_s(), Some(45.));
        let (_, phase) = p.demand(0., 0.0024, [0., 0.05, 0.], [0.; 3]);
        assert_eq!(phase, "k=0.1");
        let (_, phase) = p.demand(44.9, 0.0024, [0., 0.05, 0.], [0.; 3]);
        assert_eq!(phase, "k=1.5");
        assert_eq!(p.wall_level(), Some(14));
    }

    #[test]
    fn over_is_sticky_for_the_connection() {
        let mut s = unlocked_session();
        s.record_rung(Rung::N0_5, Axis::X, Sign::Pos, Verdict::Over);
        s.record_rung(Rung::N0_5, Axis::X, Sign::Pos, Verdict::Pass);
        assert_eq!(s.rungs[Rung::N0_5.index()][0][0], Some(Verdict::Over));
        assert!(!s.rung_cleared(Rung::N0_5, Axis::X));
    }

    #[test]
    fn lift_gain_from_masses() {
        let mut s = Session::connected();
        s.record_lift(0., 0.5);
        assert_eq!(s.lift.gain(), None);
        s.record_lift(100., 0.5 + 0.5 * 0.1 * G_M_S2);
        let g = s.lift.gain().unwrap();
        assert!((g - 0.5).abs() < 1e-9, "{g}");
    }
}
