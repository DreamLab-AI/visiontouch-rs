//! Hardware-free servo policy: interlocks, the active step and stop reasons.
//!
//! [`Controller::tick`] consumes one sample and returns the demand to request. Any
//! stop returns a zero demand on the same tick. Time is injected through
//! [`TickInput::now`], so tests step a virtual clock.
use crate::evidence::{Auto, TelemetrySummary};
use crate::protocol::{Allowance, Class, Plan, StepRequest, score_sign};
use std::collections::VecDeque;
use std::time::Instant;
use visiontouch_device::{CONTINUOUS_FORCE_N, Demand, Telemetry};
use visiontouch_effects::math::{V, norm, sub};

/// Seconds an attempt may spend above [`CONTINUOUS_FORCE_N`].
pub const OVER_CAP_BUDGET_S: f64 = 10.;
/// Oscillation RMS threshold, metres per second.
pub const OSC_RMS_M_S: f64 = 0.015;
/// Interlock velocity window, seconds: four nominal 2.4 ms ticks (P-0065).
pub const VEL_WINDOW_S: f64 = 0.0096;
/// Shortest span over which a velocity is reported; below it the velocity is zero.
pub const VEL_MIN_SPAN_S: f64 = 0.002;
/// Oscillation analysis window, seconds.
pub const OSC_WINDOW_S: f64 = 0.5;
/// Lift-off is declared after this rise, metres.
pub const LIFTOFF_M: f64 = 0.002;
/// Interval between thermal confirmations, seconds.
pub const CONFIRM_EVERY_S: f64 = 30.;
/// Grace after a confirmation falls due, seconds.
pub const CONFIRM_GRACE_S: f64 = 10.;
/// Thermal rise over the first reading that stops the test, degrees Celsius.
pub const THERMAL_RISE_C: f64 = 10.;
/// Absolute thermal reading that stops the test, degrees Celsius.
pub const THERMAL_MAX_C: f64 = 50.;
/// Sign pulse time bound: the 1.0 s plan plus a 0.5 s margin.
pub const SIGN_BOUND_S: f64 = 1.5;
/// Longest tolerated sample interval, seconds.
pub const MAX_INTERVAL_S: f64 = 0.025;
/// Longest tolerated age of the interface heartbeat, milliseconds.
pub const MAX_UI_AGE_MS: f64 = 250.;

/// Interlock limits of a step class.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClassLimits {
    /// Filtered speed limit, metres per second.
    pub speed_m_s: f64,
    /// Excursion from the start pose, metres.
    pub excursion_m: f64,
    /// Time bound, seconds.
    pub bound_s: f64,
    /// Output requires a held key (Thermal uses Confirm instead).
    pub needs_hold: bool,
}
/// Limits per class, from spec 14 section 5.
pub fn class_limits(class: Class) -> ClassLimits {
    let (speed, excursion, bound, hold) = match class {
        Class::Backdrive => (0.4, 0.15, 60., false),
        Class::Liveness => (0.05, 0.015, 4., true),
        Class::Sign => (0.05, 0.015, SIGN_BOUND_S, true),
        Class::Ladder => (0.05, 0.003, 4., true),
        Class::Lift => (0.05, 0.025, 60., true),
        Class::Effects => (0.4, 0.15, 180., true),
        Class::Thermal => (0.05, 0.003, 660., false),
        Class::Wall => (0.4, 0.15, 60., true),
    };
    ClassLimits {
        speed_m_s: speed,
        excursion_m: excursion,
        bound_s: bound,
        needs_hold: hold,
    }
}

/// Why an attempt ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    /// The plan ran to its end.
    Completed,
    /// Lift test: the pen rose more than 2 mm. A normal end.
    Liftoff,
    /// The operator pressed Stop.
    Operator,
    /// The hold key was released.
    Released,
    /// The interface heartbeat was older than 250 ms.
    UiStale,
    /// A sample interval exceeded 25 ms.
    SampleGap,
    /// The pen button was pressed.
    Button,
    /// Filtered speed above the class limit.
    Speed,
    /// Excursion from the start pose above the class limit.
    Excursion,
    /// The class time bound passed.
    TimeBound,
    /// More than 10 s above 1.0 N.
    OverContinuousCap,
    /// Band-passed Y velocity oscillation.
    Oscillation,
    /// A thermal confirmation was missed.
    ConfirmMissed,
    /// Casing rose more than 10 C over the first reading.
    ThermalRise,
    /// A casing reading above 50 C.
    ThermalOver,
    /// A non-finite position.
    NonFinite,
    /// A Cartesian step without registration.
    Unregistered,
    /// The device refused the demand and shut output down.
    Rejected,
}
impl StopReason {
    /// Stable name, written to the evidence.
    pub fn name(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Liftoff => "lift-off",
            Self::Operator => "operator-stop",
            Self::Released => "hold-released",
            Self::UiStale => "ui-heartbeat-stale",
            Self::SampleGap => "sample-gap",
            Self::Button => "pen-button",
            Self::Speed => "speed",
            Self::Excursion => "excursion",
            Self::TimeBound => "time-bound",
            Self::OverContinuousCap => "over-continuous-cap",
            Self::Oscillation => "oscillation",
            Self::ConfirmMissed => "confirm-missed",
            Self::ThermalRise => "thermal-rise",
            Self::ThermalOver => "thermal-over",
            Self::NonFinite => "non-finite-position",
            Self::Unregistered => "unregistered",
            Self::Rejected => "request-rejected",
        }
    }
    /// True for an end the plan intended.
    pub fn is_normal(self) -> bool {
        matches!(self, Self::Completed | Self::Liftoff)
    }
}

/// One servo sample presented to the controller.
#[derive(Clone, Copy, Debug)]
pub struct TickInput {
    /// Monotonic time of the sample.
    pub now: Instant,
    /// Seconds since the previous sample.
    pub interval_s: f64,
    /// Microseconds spent in sample I/O.
    pub io_us: u64,
    /// Hold key currently down.
    pub held: bool,
    /// Age of the interface heartbeat, milliseconds.
    pub ui_age_ms: f64,
    /// Pen button pressed.
    pub button: bool,
    /// Raw counters.
    pub counts: [i16; 6],
    /// Unwrapped counters.
    pub continuous: [i64; 6],
    /// Counters relative to the reset pose, if registered.
    pub registered: Option<[i64; 6]>,
    /// Registered position, or the unregistered display position.
    pub position: V,
    /// Raw sample packet.
    pub raw: [u8; 16],
    /// Guard telemetry from the previous service.
    pub telemetry: Telemetry,
}

/// What the servo loop should do with this sample.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TickOutput {
    /// Demand to request (zero when stopping).
    pub demand: Demand,
    /// Plan segment name.
    pub phase: &'static str,
    /// Target pose for the record, if any.
    pub target: V,
    /// Seconds since the attempt began.
    pub elapsed_s: f64,
    /// Set on the tick the attempt ended.
    pub stop: Option<StopReason>,
    /// Filtered velocity used by the interlocks.
    pub velocity: V,
}

/// Everything known about a finished attempt.
#[derive(Clone, Debug)]
pub struct Outcome {
    /// What ran.
    pub request: StepRequest,
    /// Duration in seconds.
    pub duration_s: f64,
    /// Automatic measurements.
    pub auto: Auto,
    /// Per-tick summary.
    pub summary: TelemetrySummary,
    /// Detail for the stop record.
    pub detail: String,
}

/// Lock-in result per encoder.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LockInResult {
    /// Amplitude at the drive frequency, counts.
    pub amplitude: [f64; 3],
    /// Noise estimate from off-frequency references, counts (floored).
    pub noise: [f64; 3],
}
impl LockInResult {
    /// Best encoder amplitude over noise.
    pub fn best_ratio(&self) -> f64 {
        (0..3)
            .map(|i| self.amplitude[i] / self.noise[i])
            .fold(0., f64::max)
    }
    /// Any encoder exceeds five times its noise.
    pub fn pass(&self) -> bool {
        self.best_ratio() > 5.
    }
}
/// Noise floor, in counts, below which quantisation hides the estimate.
pub const NOISE_FLOOR_COUNTS: f64 = 0.5;
/// Score a liveness record: `(time, three encoder counts)` against a `drive_hz` square wave.
pub fn lockin(samples: &[(f64, [f64; 3])], drive_hz: f64) -> LockInResult {
    let n = samples.len().max(1) as f64;
    // Remove a least-squares line per encoder so slow hand drift cannot leak into the bins.
    let tm = samples.iter().map(|s| s.0).sum::<f64>() / n;
    let stt: f64 = samples.iter().map(|s| (s.0 - tm).powi(2)).sum();
    let mean: [f64; 3] = std::array::from_fn(|e| samples.iter().map(|s| s.1[e]).sum::<f64>() / n);
    let slope: [f64; 3] = std::array::from_fn(|e| {
        if stt > 0. {
            samples
                .iter()
                .map(|s| (s.0 - tm) * (s.1[e] - mean[e]))
                .sum::<f64>()
                / stt
        } else {
            0.
        }
    });
    let amp = |e: usize, f: f64| {
        let (mut i, mut q) = (0., 0.);
        for (t, c) in samples {
            let ph = std::f64::consts::TAU * f * t;
            let y = c[e] - mean[e] - slope[e] * (t - tm);
            i += y * ph.cos();
            q += y * ph.sin();
        }
        2. / n * i.hypot(q)
    };
    // Off-frequency references that are neither the drive nor an odd harmonic of it.
    let off = [0.5, 1.5, 4.5, 6.0, 7.5].map(|k| k * drive_hz / 3.);
    LockInResult {
        amplitude: std::array::from_fn(|e| amp(e, drive_hz)),
        noise: std::array::from_fn(|e| {
            (off.iter().map(|f| amp(e, *f)).sum::<f64>() / off.len() as f64).max(NOISE_FLOOR_COUNTS)
        }),
    }
}

/// Band-passed Y-velocity oscillation detector: 5 to 60 Hz, RMS over 0.5 s.
#[derive(Debug, Default)]
pub struct Oscillation {
    hp: [(f64, f64); 4],
    lp: [f64; 2],
    window: VecDeque<(f64, f64)>,
    sum: f64,
}
impl Oscillation {
    /// Push a velocity at time `t` with interval `dt`; true once the band RMS
    /// exceeds [`OSC_RMS_M_S`] over a full window.
    pub fn push(&mut self, t: f64, dt: f64, v: f64) -> bool {
        let dt = dt.clamp(1e-4, 0.05);
        let rc_hp = 1. / (std::f64::consts::TAU * 5.);
        let a = rc_hp / (rc_hp + dt);
        let mut x = v;
        for (prev_in, prev_out) in &mut self.hp {
            let y = a * (*prev_out + x - *prev_in);
            *prev_in = x;
            *prev_out = y;
            x = y;
        }
        let rc_lp = 1. / (std::f64::consts::TAU * 60.);
        let b = dt / (rc_lp + dt);
        for y in &mut self.lp {
            *y += b * (x - *y);
            x = *y;
        }
        self.window.push_back((t, x * x));
        self.sum += x * x;
        while self.window.front().is_some_and(|f| t - f.0 > OSC_WINDOW_S) {
            if let Some(f) = self.window.pop_front() {
                self.sum -= f.1;
            }
        }
        let full = self
            .window
            .front()
            .is_some_and(|f| t - f.0 >= OSC_WINDOW_S * 0.95);
        full && (self.sum.max(0.) / self.window.len() as f64).sqrt() > OSC_RMS_M_S
    }
}

struct Active {
    plan: Plan,
    req: StepRequest,
    class: Class,
    limits: ClassLimits,
    started: Instant,
    start_pos: V,
    vel_buf: VecDeque<(f64, V)>,
    over_cap_s: f64,
    osc: Oscillation,
    lock_rows: Vec<(f64, [f64; 3])>,
    sign_rows: Vec<(f64, V)>,
    summary: TelemetrySummary,
    auto: Auto,
    pending: Option<StopReason>,
    confirm_due: f64,
    first_temp_c: Option<f64>,
    last_phase: &'static str,
    last_force: V,
    last_applied_n: f64,
    registered_required: bool,
}

/// Servo policy for one attempt at a time.
#[derive(Default)]
pub struct Controller {
    active: Option<Active>,
    outcome: Option<Outcome>,
}

impl Controller {
    /// True while an attempt is running.
    pub fn active(&self) -> Option<StepRequest> {
        self.active.as_ref().map(|a| a.req)
    }
    /// Take the outcome of the attempt that has just ended.
    pub fn take_outcome(&mut self) -> Option<Outcome> {
        self.outcome.take()
    }
    /// Begin an attempt. `input` supplies the start pose; `centre` overrides the effect centre.
    pub fn start(
        &mut self,
        req: StepRequest,
        allowance: Allowance,
        input: &TickInput,
        centre: Option<V>,
    ) -> Result<(), String> {
        if self.active.is_some() {
            return Err("An attempt is already running.".into());
        }
        let class = req.class();
        let registered_required = !matches!(class, Class::Liveness | Class::Backdrive);
        if registered_required && input.registered.is_none() {
            return Err("Register the reset pose first.".into());
        }
        if !input.position.iter().all(|x| x.is_finite()) {
            return Err("The pen position is not finite.".into());
        }
        let limits = class_limits(class);
        if limits.needs_hold && !input.held {
            return Err("Hold the key to start; release it to stop.".into());
        }
        self.outcome = None;
        self.active = Some(Active {
            plan: Plan::new(req, allowance.cap_n, input.position, centre),
            req,
            class,
            limits,
            started: input.now,
            start_pos: input.position,
            vel_buf: VecDeque::new(),
            over_cap_s: 0.,
            osc: Oscillation::default(),
            lock_rows: Vec::new(),
            sign_rows: Vec::new(),
            summary: TelemetrySummary::default(),
            auto: Auto::default(),
            pending: None,
            confirm_due: CONFIRM_EVERY_S,
            first_temp_c: None,
            last_phase: "start",
            last_force: [0.; 3],
            last_applied_n: 0.,
            registered_required,
        });
        Ok(())
    }
    /// Operator Stop.
    pub fn stop(&mut self) {
        if let Some(a) = &mut self.active {
            a.pending.get_or_insert(StopReason::Operator);
        }
    }
    /// The device rejected a request: end the attempt, recorded as [`StopReason::Rejected`].
    pub fn reject(&mut self) {
        if let Some(a) = &mut self.active {
            a.pending.get_or_insert(StopReason::Rejected);
        }
    }
    /// Thermal confirmation. True if accepted (the window was open).
    pub fn confirm(&mut self, now: Instant) -> bool {
        let Some(a) = &mut self.active else {
            return false;
        };
        if a.class != Class::Thermal {
            return false;
        }
        let t = now.saturating_duration_since(a.started).as_secs_f64();
        if t >= a.confirm_due {
            a.confirm_due += CONFIRM_EVERY_S;
            true
        } else {
            false
        }
    }
    /// Seconds until the next thermal confirmation falls due (negative while overdue).
    pub fn confirm_in_s(&self, now: Instant) -> Option<f64> {
        let a = self.active.as_ref().filter(|a| a.class == Class::Thermal)?;
        Some(a.confirm_due - now.saturating_duration_since(a.started).as_secs_f64())
    }
    /// Feed a casing temperature entered during a thermal hold.
    pub fn note_temperature(&mut self, c: f64) {
        let Some(a) = self.active.as_mut().filter(|a| a.class == Class::Thermal) else {
            return;
        };
        let first = *a.first_temp_c.get_or_insert(c);
        if c > THERMAL_MAX_C {
            a.pending.get_or_insert(StopReason::ThermalOver);
        } else if c - first > THERMAL_RISE_C {
            a.pending.get_or_insert(StopReason::ThermalRise);
        }
    }
    /// The effect frame, wall stiffness and target of the running plan.
    pub fn plan_view(&self) -> Option<(&Plan, StepRequest)> {
        self.active.as_ref().map(|a| (&a.plan, a.req))
    }
    /// Advance one sample.
    pub fn tick(&mut self, input: &TickInput) -> Option<TickOutput> {
        let a = self.active.as_mut()?;
        let t = input.now.saturating_duration_since(a.started).as_secs_f64();
        let dt = input.interval_s.max(1e-4);
        // Interlock velocity: displacement over a fixed time window rather than a
        // mean of the last four samples. A four-sample mean spans 10 ms only at the
        // nominal 2.4 ms cadence; an unpaced loop at 0.28 ms shrank it to 1 ms and
        // turned encoder quantisation into a phantom 50 mm/s (P-0065). The window
        // is measured in seconds, so cadence cannot change what the interlock sees.
        if input.position.iter().all(|x| x.is_finite()) {
            a.vel_buf.push_back((t, input.position));
        }
        while a
            .vel_buf
            .front()
            .is_some_and(|(t0, _)| t - t0 > VEL_WINDOW_S && a.vel_buf.len() > 2)
        {
            a.vel_buf.pop_front();
        }
        let vel: V = match (a.vel_buf.front(), a.vel_buf.back()) {
            (Some((t0, p0)), Some((t1, p1))) if t1 - t0 >= VEL_MIN_SPAN_S => {
                sub(*p1, *p0).map(|d| d / (t1 - t0))
            }
            _ => [0.; 3],
        };

        // Interlocks, most specific first.
        let mut stop = a.pending;
        let mut check = |cond: bool, reason: StopReason| {
            if stop.is_none() && cond {
                stop = Some(reason);
            }
        };
        check(
            !input.position.iter().all(|x| x.is_finite()),
            StopReason::NonFinite,
        );
        check(
            a.registered_required && input.registered.is_none(),
            StopReason::Unregistered,
        );
        check(input.button, StopReason::Button);
        check(input.interval_s > MAX_INTERVAL_S, StopReason::SampleGap);
        check(input.ui_age_ms > MAX_UI_AGE_MS, StopReason::UiStale);
        check(a.limits.needs_hold && !input.held, StopReason::Released);
        check(
            a.class == Class::Thermal && t > a.confirm_due + CONFIRM_GRACE_S,
            StopReason::ConfirmMissed,
        );
        check(norm(vel) > a.limits.speed_m_s, StopReason::Speed);
        check(
            norm(sub(input.position, a.start_pos)) > a.limits.excursion_m,
            StopReason::Excursion,
        );
        let osc_axis = match a.req {
            StepRequest::Ladder { axis, .. } => axis.index(),
            _ => 1,
        };
        let oscillating = a.osc.push(t, dt, vel[osc_axis]);
        check(
            oscillating && matches!(a.class, Class::Wall | Class::Ladder),
            StopReason::Oscillation,
        );
        check(
            a.class == Class::Lift && input.position[1] - a.start_pos[1] > LIFTOFF_M,
            StopReason::Liftoff,
        );
        let applied = norm(input.telemetry.applied_force);
        a.last_applied_n = applied;
        if applied > CONTINUOUS_FORCE_N + 1e-9 {
            a.over_cap_s += dt;
        }
        check(
            a.over_cap_s > OVER_CAP_BUDGET_S,
            StopReason::OverContinuousCap,
        );
        let duration = a.plan.duration_s();
        check(duration.is_some_and(|d| t >= d), StopReason::Completed);
        check(t > a.limits.bound_s, StopReason::TimeBound);

        // Bookkeeping for the record.
        let s = &mut a.summary;
        s.samples += 1;
        s.max_interval_ms = s.max_interval_ms.max(input.interval_s * 1e3);
        s.stale_ticks += u64::from(input.telemetry.expired);
        s.force_capped_ticks += u64::from(input.telemetry.force_capped);
        s.dac_capped_ticks += u64::from(input.telemetry.dac_capped);
        for i in 0..3 {
            s.max_abs_offsets[i] =
                s.max_abs_offsets[i].max(input.telemetry.applied_motor_offsets[i].unsigned_abs());
        }
        if a.class == Class::Liveness {
            a.lock_rows
                .push((t, std::array::from_fn(|i| input.continuous[i] as f64)));
        }
        if a.class == Class::Sign {
            a.sign_rows.push((t, input.position));
        }

        let (planned, phase) = a.plan.demand(t, dt, input.position, vel);
        let zero = if a.class == Class::Liveness {
            Demand::Channels([0; 3])
        } else {
            Demand::Force([0.; 3])
        };
        let demand = if stop.is_some() { zero } else { planned };
        if let Demand::Force(f) = planned {
            a.last_force = f;
        }
        a.last_phase = phase;
        let target = a.plan.target().unwrap_or(a.start_pos);
        let out = TickOutput {
            demand,
            phase: if stop.is_some() { "zero" } else { phase },
            target,
            elapsed_s: t,
            stop,
            velocity: vel,
        };
        if let Some(reason) = stop {
            self.finish(reason, t, input);
        }
        Some(out)
    }
    fn finish(&mut self, reason: StopReason, t: f64, input: &TickInput) {
        let Some(mut a) = self.active.take() else {
            return;
        };
        a.auto.oscillation = reason == StopReason::Oscillation;
        if a.class == Class::Liveness {
            let r = lockin(&a.lock_rows, crate::protocol::LIVENESS_HZ);
            let best = (0..3)
                .max_by(|x, y| {
                    (r.amplitude[*x] / r.noise[*x]).total_cmp(&(r.amplitude[*y] / r.noise[*y]))
                })
                .unwrap_or(0);
            a.auto.lockin_amplitude_counts = Some(r.amplitude[best]);
            a.auto.noise_counts = Some(r.noise[best]);
            a.auto.lockin_encoder = Some(best);
            a.auto.lockin_pass = Some(reason.is_normal() && r.pass());
        }
        if a.class == Class::Sign
            && reason.is_normal()
            && let StepRequest::SignPulse { axis } = a.req
        {
            a.auto.sign_agrees = score_sign(axis, &a.sign_rows);
        }
        if a.class == Class::Lift && reason == StopReason::Liftoff {
            a.auto.liftoff_force_n = Some(a.last_applied_n);
        }
        if a.class == Class::Wall && reason == StopReason::Oscillation {
            a.auto.k_marginal_n_mm = a.plan.last_stable_k_n_mm();
        }
        let detail = format!(
            "phase {} at {:.2} s, position {:?}",
            a.last_phase, t, input.position
        );
        self.outcome = Some(Outcome {
            request: a.req,
            duration_s: t,
            auto: a.auto,
            summary: a.summary,
            detail,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Axis, Rung, Sign};
    use std::time::Duration;
    use visiontouch_device::ForceLimits;

    struct Rig {
        c: Controller,
        base: Instant,
        n: u32,
        step: Duration,
        pos: V,
    }
    impl Rig {
        fn new(step_ms: f64) -> Self {
            Self {
                c: Controller::default(),
                base: Instant::now(),
                n: 0,
                step: Duration::from_secs_f64(step_ms / 1000.),
                pos: [0.; 3],
            }
        }
        fn input(&self) -> TickInput {
            TickInput {
                now: self.base + self.step * self.n,
                interval_s: self.step.as_secs_f64(),
                io_us: 100,
                held: true,
                ui_age_ms: 10.,
                button: false,
                counts: [0; 6],
                continuous: [0; 6],
                registered: Some([0; 6]),
                position: self.pos,
                raw: [0; 16],
                telemetry: Telemetry::default(),
            }
        }
        fn start(&mut self, req: StepRequest, cap: f64) {
            let a = Allowance {
                limits: ForceLimits::default(),
                cap_n: cap,
            };
            self.c.start(req, a, &self.input(), None).unwrap();
        }
        /// Tick with a mutation applied to the input; returns the output.
        fn tick(&mut self, f: impl FnOnce(&mut TickInput)) -> TickOutput {
            let mut i = self.input();
            f(&mut i);
            self.pos = i.position;
            let o = self.c.tick(&i).unwrap();
            self.n += 1;
            o
        }
        fn run_until_stop(&mut self, mut f: impl FnMut(&mut TickInput, u32)) -> StopReason {
            for _ in 0..2_000_000 {
                let n = self.n;
                let o = self.tick(|i| f(i, n));
                if let Some(s) = o.stop {
                    assert!(
                        matches!(o.demand, Demand::Force(v) if v == [0.; 3])
                            || matches!(o.demand, Demand::Channels(c) if c == [0; 3]),
                        "demand must be zero on the stopping tick"
                    );
                    return s;
                }
            }
            panic!("never stopped");
        }
    }
    fn ladder(rung: Rung) -> StepRequest {
        StepRequest::Ladder {
            axis: Axis::Y,
            sign: Sign::Neg,
            rung,
        }
    }

    #[test]
    fn ladder_completes_not_times_out_at_four_seconds() {
        let mut r = Rig::new(2.4);
        r.start(ladder(Rung::N0_5), 0.5);
        assert_eq!(r.run_until_stop(|_, _| {}), StopReason::Completed);
        let o = r.c.take_outcome().unwrap();
        assert!((o.duration_s - 4.0).abs() < 0.01);
        assert!(o.summary.samples > 1600);
    }

    #[test]
    fn each_interlock_stops_with_its_named_reason() {
        type Case = (
            &'static str,
            StopReason,
            Box<dyn FnMut(&mut TickInput, u32)>,
        );
        let cases: Vec<Case> = vec![
            (
                "button",
                StopReason::Button,
                Box::new(|i, n| i.button = n == 20),
            ),
            (
                "gap",
                StopReason::SampleGap,
                Box::new(|i, n| {
                    if n == 20 {
                        i.interval_s = 0.03
                    }
                }),
            ),
            (
                "ui",
                StopReason::UiStale,
                Box::new(|i, n| {
                    if n >= 20 {
                        i.ui_age_ms = 251.
                    }
                }),
            ),
            (
                "hold",
                StopReason::Released,
                Box::new(|i, n| i.held = n < 20),
            ),
            (
                "speed",
                StopReason::Speed,
                Box::new(|i, n| i.position = [0., -0.0002 * f64::from(n.saturating_sub(20)), 0.]),
            ),
            (
                "excursion",
                StopReason::Excursion,
                Box::new(|i, n| i.position = [0., 0.0001 * f64::from(n.saturating_sub(20)), 0.]),
            ),
            (
                "unregistered",
                StopReason::Unregistered,
                Box::new(|i, n| {
                    if n >= 20 {
                        i.registered = None
                    }
                }),
            ),
            (
                "nan",
                StopReason::NonFinite,
                Box::new(|i, n| {
                    if n == 20 {
                        i.position = [f64::NAN, 0., 0.]
                    }
                }),
            ),
        ];
        for (name, expect, mut f) in cases {
            let mut r = Rig::new(2.4);
            r.start(ladder(Rung::N0_5), 0.5);
            let got = r.run_until_stop(|i, n| f(i, n));
            assert_eq!(got, expect, "{name}");
        }
    }

    #[test]
    fn excursion_limits_per_class() {
        assert_eq!(class_limits(Class::Ladder).excursion_m, 0.003);
        assert_eq!(class_limits(Class::Thermal).excursion_m, 0.003);
        assert_eq!(class_limits(Class::Liveness).excursion_m, 0.015);
        assert_eq!(class_limits(Class::Sign).excursion_m, 0.015);
        assert_eq!(class_limits(Class::Lift).excursion_m, 0.025);
        assert_eq!(class_limits(Class::Effects).excursion_m, 0.15);
        assert_eq!(class_limits(Class::Wall).excursion_m, 0.15);
        assert_eq!(class_limits(Class::Wall).speed_m_s, 0.4);
        assert_eq!(class_limits(Class::Effects).bound_s, 180.);
        assert_eq!(class_limits(Class::Wall).bound_s, 60.);
        assert_eq!(class_limits(Class::Sign).bound_s, 1.5);
        assert_eq!(class_limits(Class::Thermal).bound_s, 660.);
        assert!(!class_limits(Class::Thermal).needs_hold);
    }

    #[test]
    fn time_bound_stops_open_ended_effects() {
        let mut r = Rig::new(10.);
        r.start(
            StepRequest::Effect {
                kind: visiontouch_effects::Kind::Viscous,
            },
            0.5,
        );
        assert_eq!(r.run_until_stop(|_, _| {}), StopReason::TimeBound);
        let o = r.c.take_outcome().unwrap();
        assert!(o.duration_s > 180. && o.duration_s < 180.1);
    }

    #[test]
    fn over_continuous_cap_for_10_s_stops() {
        let mut r = Rig::new(10.);
        r.start(
            StepRequest::Effect {
                kind: visiontouch_effects::Kind::Viscous,
            },
            1.0,
        );
        let got = r.run_until_stop(|i, _| i.telemetry.applied_force = [0., 1.5, 0.]);
        assert_eq!(got, StopReason::OverContinuousCap);
        let o = r.c.take_outcome().unwrap();
        assert!(
            o.duration_s >= 10. - 1e-9 && o.duration_s < 10.1,
            "{}",
            o.duration_s
        );
        // Exactly 1.0 N never counts, even with rounding noise.
        let mut r = Rig::new(10.);
        r.start(StepRequest::Thermal, 1.0);
        let got = r.run_until_stop(|i, n| {
            i.telemetry.applied_force = [0., -1.0000000000000002, 0.];
            if n > 3000 {
                i.held = false;
            }
        });
        assert_ne!(got, StopReason::OverContinuousCap);
    }

    #[test]
    fn lift_to_one_and_a_half_newtons_completes_inside_the_over_cap_budget() {
        let mut r = Rig::new(10.);
        r.start(StepRequest::Lift { mass_g: None }, 1.5);
        // The plant delivers exactly the previous tick's demand.
        let mut last = [0.; 3];
        let got = loop {
            let o = r.tick(|i| i.telemetry.applied_force = last);
            if let Demand::Force(f) = o.demand {
                last = f;
            }
            if let Some(s) = o.stop {
                break s;
            }
        };
        assert_eq!(got, StopReason::Completed);
    }

    #[test]
    fn rejection_is_recorded_as_rejected_not_operator() {
        let mut r = Rig::new(10.);
        r.start(ladder(Rung::N0_5), 0.5);
        r.tick(|_| {});
        r.c.reject();
        let o = r.tick(|_| {});
        assert_eq!(o.stop, Some(StopReason::Rejected));
        assert_eq!(StopReason::Rejected.name(), "request-rejected");
        assert!(!StopReason::Rejected.is_normal());
    }

    #[test]
    fn sign_pulse_has_half_a_second_of_margin() {
        let mut r = Rig::new(2.4);
        r.start(StepRequest::SignPulse { axis: Axis::X }, 0.5);
        assert_eq!(r.run_until_stop(|_, _| {}), StopReason::Completed);
    }

    #[test]
    fn thermal_confirm_has_a_ten_second_grace_and_no_hold() {
        let mut r = Rig::new(10.);
        r.start(StepRequest::Thermal, 1.0);
        let mut confirmed = 0;
        let got = r.run_until_stop(|i, _| {
            i.held = false;
        });
        // Without Confirm the test ends 10 s after the first is due.
        assert_eq!(got, StopReason::ConfirmMissed);
        let o = r.c.take_outcome().unwrap();
        assert!(
            o.duration_s > 40. && o.duration_s < 40.1,
            "{}",
            o.duration_s
        );
        let mut r = Rig::new(10.);
        r.start(StepRequest::Thermal, 1.0);
        let mut stopped = None;
        for _ in 0..8000 {
            let now = r.input().now;
            if r.c.confirm_in_s(now).is_some_and(|s| s <= 0. && s > -9.) {
                assert!(r.c.confirm(now));
                confirmed += 1;
            }
            let o = r.tick(|i| i.held = false);
            if o.stop.is_some() {
                stopped = o.stop;
                break;
            }
        }
        assert_eq!(confirmed, 2, "confirmed at 30 s and 60 s of 80 s");
        assert_eq!(
            stopped, None,
            "80 s of confirmed thermal never needed a hold"
        );
        // Early confirmation is refused: the next one falls due at 90 s.
        let mut early = Rig::new(10.);
        early.start(StepRequest::Thermal, 1.0);
        early.tick(|i| i.held = false);
        assert!(!early.c.confirm(early.input().now), "not due yet");
    }

    #[test]
    fn thermal_temperature_stops() {
        for (readings, expect) in [
            (vec![25., 36.], StopReason::ThermalRise),
            (vec![25., 30., 51.], StopReason::ThermalOver),
        ] {
            let mut r = Rig::new(10.);
            r.start(StepRequest::Thermal, 1.0);
            r.tick(|_| {});
            for c in readings {
                r.c.note_temperature(c);
            }
            let o = r.tick(|_| {});
            assert_eq!(o.stop, Some(expect));
        }
        let mut r = Rig::new(10.);
        r.start(StepRequest::Thermal, 1.0);
        r.c.note_temperature(25.);
        r.c.note_temperature(34.9);
        assert_eq!(r.tick(|_| {}).stop, None);
    }

    #[test]
    fn lift_off_is_detected_above_two_millimetres() {
        let mut r = Rig::new(10.);
        r.start(StepRequest::Lift { mass_g: None }, 1.0);
        // The plant delivers 0.8 of the demand: the record must carry what was applied.
        let mut last = [0.; 3];
        let got = loop {
            let n = r.n;
            let o = r.tick(|i| {
                i.position = [0., 0.00005 * f64::from(n.saturating_sub(600)), 0.];
                i.telemetry.applied_force = last;
            });
            if let Demand::Force(f) = o.demand {
                last = f.map(|x| x * 0.8);
            }
            if let Some(s) = o.stop {
                break s;
            }
        };
        assert_eq!(got, StopReason::Liftoff);
        let o = r.c.take_outcome().unwrap();
        let f = o.auto.liftoff_force_n.unwrap();
        assert!((f - 0.8 * 0.05 * o.duration_s).abs() < 0.01, "{f}");
        let mut r = Rig::new(10.);
        r.start(StepRequest::Lift { mass_g: None }, 0.5);
        let got =
            r.run_until_stop(|i, n| i.position = [0., 0.0019 * (f64::from(n) / 100.).min(1.), 0.]);
        assert_eq!(got, StopReason::Completed, "1.9 mm is not lift-off");
        assert!(r.c.take_outcome().unwrap().auto.liftoff_force_n.is_none());
    }

    fn osc_stops(hz: f64, amp_m: f64, counts: bool) -> Option<f64> {
        let mut o = Oscillation::default();
        let dt = 0.0024;
        let mut prev = 0.;
        let mut buf = VecDeque::new();
        for i in 0..2000 {
            let t = f64::from(i) * dt;
            let mut y = amp_m * (std::f64::consts::TAU * hz * t).sin();
            if counts {
                y = (y / 3.0e-5).round() * 3.0e-5;
            }
            buf.push_back((y - prev) / dt);
            if buf.len() > 4 {
                buf.pop_front();
            }
            prev = y;
            let v = buf.iter().sum::<f64>() / buf.len() as f64;
            if o.push(t, dt, v) {
                return Some(t);
            }
        }
        None
    }
    #[test]
    fn oscillation_fires_on_20_hz_not_on_hand_motion_or_flicker() {
        assert!(osc_stops(20., 0.001, true).is_some(), "1 mm at 20 Hz");
        assert!(osc_stops(20., 0.0003, true).is_some(), "0.3 mm at 20 Hz");
        assert!(
            osc_stops(2., 0.010, true).is_none(),
            "2 Hz hand motion, 10 mm"
        );
        assert!(osc_stops(1., 0.020, true).is_none());
        // One-count flicker, alternating every tick, and a single step.
        let mut o = Oscillation::default();
        let mut prev = 0.;
        let mut buf = VecDeque::new();
        for i in 0..2000 {
            let y = if i % 2 == 0 { 3.0e-5 } else { 0. };
            buf.push_back((y - prev) / 0.0024);
            if buf.len() > 4 {
                buf.pop_front();
            }
            prev = y;
            let v = buf.iter().sum::<f64>() / buf.len() as f64;
            assert!(!o.push(f64::from(i) * 0.0024, 0.0024, v), "flicker at {i}");
        }
    }

    #[test]
    fn wall_oscillation_stops_the_attempt_and_records_k() {
        let mut r = Rig::new(2.4);
        r.start(StepRequest::WallRamp, 1.0);
        let got = r.run_until_stop(|i, n| {
            let t = f64::from(n) * 0.0024;
            let y = 0.0008 * (std::f64::consts::TAU * 20. * t).sin();
            i.position = [0., (y / 3.0e-5).round() * 3.0e-5, 0.];
        });
        assert_eq!(got, StopReason::Oscillation);
        let o = r.c.take_outcome().unwrap();
        assert!(
            o.auto.oscillation && o.auto.k_marginal_n_mm.is_none(),
            "oscillating at the first level leaves no stable stiffness"
        );
    }

    #[test]
    fn k_marginal_is_the_last_stable_level_not_the_oscillating_one() {
        let mut r = Rig::new(2.4);
        r.start(StepRequest::WallRamp, 1.0);
        // Quiet until 6.5 s (level index 2, 0.3 N/mm), then a 20 Hz buzz.
        let got = r.run_until_stop(|i, n| {
            let t = f64::from(n) * 0.0024;
            if t > 6.5 {
                let y = 0.0008 * (std::f64::consts::TAU * 20. * t).sin();
                i.position = [0., (y / 3.0e-5).round() * 3.0e-5, 0.];
            }
        });
        assert_eq!(got, StopReason::Oscillation);
        let k = r.c.take_outcome().unwrap().auto.k_marginal_n_mm.unwrap();
        assert!((k - 0.2).abs() < 1e-9, "{k}");
    }

    fn drive(r: &mut Rig, response: impl Fn(f64, [i16; 3]) -> [f64; 3]) -> Outcome {
        r.start(StepRequest::Liveness { channel: 0 }, 0.);
        loop {
            let t = f64::from(r.n) * r.step.as_secs_f64();
            // Mirror the demand the plan has just produced into the virtual plant.
            let hi = ((t * 6.).floor() as i64) % 2 == 0;
            let drive = [if hi { 80 } else { -80 }, 0, 0];
            let resp = response(t, drive);
            let o = r.tick(|i| {
                i.continuous = [resp[0] as i64, resp[1] as i64, resp[2] as i64, 0, 0, 0];
            });
            if o.stop.is_some() {
                return r.c.take_outcome().unwrap();
            }
        }
    }
    #[test]
    fn liveness_passes_on_a_swapped_channel_map_and_fails_on_a_dead_one() {
        let sq = |t: f64| {
            if ((t * 6.).floor() as i64) % 2 == 0 {
                1.
            } else {
                -1.
            }
        };
        // Channel 0 drives encoder 2 (swapped map), 30 counts.
        let mut r = Rig::new(2.4);
        let o = drive(&mut r, |t, _| [0., 0., 30. * sq(t)]);
        assert_eq!(o.auto.lockin_pass, Some(true));
        assert_eq!(o.auto.lockin_encoder, Some(2));
        // A dead channel with one-count flicker.
        let mut r = Rig::new(2.4);
        let o = drive(&mut r, |t, _| [0., (t * 1000.).sin().signum().max(0.), 0.]);
        assert_eq!(o.auto.lockin_pass, Some(false), "{:?}", o.auto);
        // Perfectly still: the noise floor stops 0/0 passing.
        let mut r = Rig::new(2.4);
        let o = drive(&mut r, |_, _| [0.; 3]);
        assert_eq!(o.auto.lockin_pass, Some(false));
    }

    #[test]
    fn start_requires_registration_and_hold_and_refuses_double_start() {
        let mut c = Controller::default();
        let r = Rig::new(2.4);
        let a = Allowance {
            limits: ForceLimits::default(),
            cap_n: 0.5,
        };
        let mut i = r.input();
        i.registered = None;
        assert!(c.start(ladder(Rung::N0_1), a, &i, None).is_err());
        assert!(
            c.start(StepRequest::Liveness { channel: 1 }, a, &i, None)
                .is_ok(),
            "liveness needs no registration"
        );
        assert!(
            c.start(StepRequest::Backdrive, a, &i, None).is_err(),
            "already running"
        );
        let mut c = Controller::default();
        let mut i = r.input();
        i.held = false;
        assert!(
            c.start(ladder(Rung::N0_1), a, &i, None)
                .unwrap_err()
                .contains("Hold")
        );
    }

    #[test]
    fn sign_pulse_outcome_scores_the_displacement() {
        let mut r = Rig::new(2.4);
        r.start(StepRequest::SignPulse { axis: Axis::X }, 0.5);
        let got = r.run_until_stop(|i, n| {
            let t = f64::from(n) * 0.0024;
            let x = if t < 0.5 {
                0.001 * t
            } else {
                0.0005 - 0.001 * (t - 0.5)
            };
            i.position = [x, 0., 0.];
        });
        assert_eq!(got, StopReason::Completed);
        assert_eq!(r.c.take_outcome().unwrap().auto.sign_agrees, Some(true));
    }
}
