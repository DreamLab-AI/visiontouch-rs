//! Reusable device state and bounded force feedback. See docs/spec/12-force-feedback.md.
//! The reference kinematic model is not a physical calibration of this unit.
//!
//! # Lifecycle
//!
//! Connect with output disabled, acquire/validate the pose, configure stricter
//! limits if needed, then explicitly enable feedback. A worker must call
//! [`Device::sample`] continuously, even when the UI has no new force. Every
//! request carries its original [`Instant`]; never refresh a stale request's time.
//! Read/acknowledgement errors and configured command-limit trips end the session.
//!
//! ```no_run
//! use haptics_device::{Device, ForceLimits, Wrench};
//! use std::time::{Duration, Instant};
//! # fn main() -> std::io::Result<()> {
//! let mut device = Device::connect("/dev/parport0")?;
//! device.sample()?; // input only; no output session yet
//! device.guard.set_limits(ForceLimits {
//!     force_n: 0.1,
//!     release_after: Some(Duration::from_millis(250)),
//!     ..Default::default()
//! }).map_err(std::io::Error::other)?;
//! device.enable_feedback()?; // observed zero-only arming handshake
//! // A real application repeats this on a dedicated worker with operator,
//! // pose, timing and travel interlocks; this example sends zero once.
//! device.request(Wrench::default(), Instant::now())?;
//! device.sample()?;
//! device.close()?;
//! # Ok(()) }
//! ```
//!
//! # Coordinates and limits
//!
//! Main-arm kinematics use reference-model metres; forces use model newtons.
//! All six encoder channels are preserved but direct wrist torque is unsupported.
//! [`UNIT_DERATING`] applies once to guarded output across all applications.
//! [`ForceLimits`] may only lower the absolute SDK ceilings. A saturation fault
//! measures command demand, not motor current, hand force, temperature or a
//! decoded hardware overload signal. Independent surface-temperature observations
//! belong in session records. Software cleanup cannot cover SIGKILL or a kernel
//! scheduling stall; no hardware watchdog behavior is claimed.
#![warn(missing_docs)]
pub mod shutdown;
use haptics_hal::device::Sample;
use std::time::{Duration, Instant};

/// Maximum request age and serviced-tick gap before the software guard returns zero.
/// Only evaluated when the worker runs; this is not an autonomous hardware watchdog.
pub const COMMAND_TIMEOUT: Duration = Duration::from_millis(25);
/// Maximum pre-derating model-force magnitude in newtons, not a measured rating.
pub const MAX_FORCE_N: f64 = 0.5;
/// Maximum pre-derating absolute per-channel command offset from neutral.
pub const MAX_DAC_OFFSET: i16 = 96;

/// One unit-wide gain, applied exactly once by the guard to force, slew and DAC
/// ceilings. This scales the already conservative SDK envelope, not a historical
/// manufacturer rating. Finalise from logged motion, operator sound observations
/// and measured motor-surface temperatures. Keep in (0, 1].
pub const UNIT_DERATING: f64 = 1.0;
const _: () = assert!(UNIT_DERATING > 0. && UNIT_DERATING <= 1.);

/// Optional stricter application limits. Values may only lower SDK ceilings.
/// Saturation release observes command demand, never measured physical load.
#[derive(Clone, Copy, Debug)]
pub struct ForceLimits {
    /// Pre-derating vector magnitude ceiling in model newtons, in (0, 0.5].
    pub force_n: f64,
    /// Pre-derating force-vector slew ceiling in model N/s, in (0, 2].
    pub slew_n_s: f64,
    /// Pre-derating absolute per-channel DAC offset ceiling, in 1..=96.
    pub dac_counts: i16,
    /// Optional continuous command-limit duration before a latched release.
    /// Must be positive and at most one second; None disables this extra policy.
    pub release_after: Option<Duration>,
}
impl Default for ForceLimits {
    fn default() -> Self {
        Self {
            force_n: MAX_FORCE_N,
            slew_n_s: 2.,
            dac_counts: MAX_DAC_OFFSET,
            release_after: None,
        }
    }
}
impl ForceLimits {
    fn valid(self) -> bool {
        self.force_n.is_finite()
            && self.force_n > 0.
            && self.force_n <= MAX_FORCE_N
            && self.slew_n_s.is_finite()
            && self.slew_n_s > 0.
            && self.slew_n_s <= 2.
            && self.dac_counts > 0
            && self.dac_counts <= MAX_DAC_OFFSET
            && self
                .release_after
                .is_none_or(|d| !d.is_zero() && d <= Duration::from_secs(1))
    }
}

/// Latched software fault. This is not a sensor or decoded amplifier fault.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForceFault {
    /// Demand remained over the force or DAC limit for the configured duration.
    SustainedCommandLimit,
}
impl ForceFault {
    /// Operator-facing description of the software trip.
    pub fn message(self) -> &'static str {
        match self {
            Self::SustainedCommandLimit => {
                "sustained command limit; output released, explicit rearming required"
            }
        }
    }
}

/// Force and torque requested at the tool in the device kinematic frame, in
/// model N and N·m. Applications must transform scene axes into this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Wrench {
    /// Cartesian force in model newtons in the device kinematic coordinate frame.
    pub force: [f64; 3],
    /// Torque in N·m. Only the zero vector is accepted until wrist actuation is verified.
    pub torque: [f64; 3],
}

/// Retains every measured axis, including the three wrist/pen counters.
#[derive(Default)]
pub struct EncoderState {
    previous: Option<[i16; 6]>,
    continuous_counts: [i64; 6],
    /// Latest raw signed 16-bit counters for all six measured channels.
    pub counts: [i16; 6],
    /// Accumulated count deltas since first sample or the last relative recenter.
    pub relative_counts: [i64; 6],
    /// Latest decoded active-low pen button state.
    pub button_pressed: bool,
}
impl EncoderState {
    /// Consume a packet and unwrap successive count deltas.
    /// Motion between samples must be less than half the 16-bit counter range.
    pub fn update(&mut self, sample: Sample) {
        self.counts = sample.encoder_counts();
        if let Some(previous) = self.previous {
            for (i, old) in previous.into_iter().enumerate() {
                let delta = i64::from(self.counts[i].wrapping_sub(old));
                self.relative_counts[i] += delta;
                self.continuous_counts[i] += delta;
            }
        }
        if self.previous.is_none() {
            self.continuous_counts = self.counts.map(i64::from);
        }
        self.previous = Some(self.counts);
        self.button_pressed = sample.stylus_button_pressed();
    }
    /// Zero display-relative deltas without changing absolute kinematic history.
    pub fn recenter(&mut self) {
        self.relative_counts = [0; 6];
    }
    /// Unwrapped absolute counters, unaffected by relative-display recentering.
    /// Successive samples must be less than half a counter revolution apart.
    pub fn continuous_counts(&self) -> [i64; 6] {
        self.continuous_counts
    }
    /// Compute model position in metres from continuous absolute main-arm counters.
    pub fn position_m(&self) -> [f64; 3] {
        position_from_counts(std::array::from_fn(|i| self.continuous_counts[i] as f64))
    }
}

fn angles(counts: [f64; 3]) -> [f64; 3] {
    [
        counts[0] * 0.00018075138,
        -counts[2] * 0.00021136618,
        -counts[1] * 0.00021136618,
    ]
}

/// Raw-counter convenience position in model metres. For moving devices use
/// [`EncoderState::position_m`] to preserve continuity across signed counter wrap.
pub fn position_m(counts: [i16; 6]) -> [f64; 3] {
    position_from_counts([counts[0] as f64, counts[1] as f64, counts[2] as f64])
}

/// Accepts accumulated counts so a signed 16-bit rollover does not jump the pose.
pub fn position_from_counts(counts: [f64; 3]) -> [f64; 3] {
    let [a, b, c] = angles(counts);
    let r = 0.1397 * (b.cos() + c.sin());
    [
        -r * a.sin(),
        0.1397 * (b.sin() - c.cos() + 1.),
        r * a.cos() - 0.1397,
    ]
}

/// Jacobian columns in main-joint order. Force is transformed by J^T.
pub fn jacobian(counts: [i16; 6]) -> [[f64; 3]; 3] {
    jacobian_from_counts(std::array::from_fn(|i| counts[i] as f64))
}

/// Continuous-count counterpart of [`jacobian`], sharing the pose model history.
pub fn jacobian_from_counts(counts: [f64; 3]) -> [[f64; 3]; 3] {
    let [a, b, c] = angles(counts);
    let l = 0.1397;
    let r = l * (b.cos() + c.sin());
    [
        [-r * a.cos(), 0., -r * a.sin()],
        [l * b.sin() * a.sin(), l * b.cos(), -l * b.sin() * a.cos()],
        [-l * c.cos() * a.sin(), l * c.sin(), l * c.cos() * a.cos()],
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}
fn limited(v: [f64; 3], max: f64) -> [f64; 3] {
    let n = norm(v);
    v.map(|x| x * if n > max { max / n } else { 1. })
}

/// Pure conversion; callers must apply force limits and an armed output lifecycle.
pub fn motor_offsets(counts: [i16; 6], force: [f64; 3]) -> [i16; 3] {
    raw_motor_offsets(std::array::from_fn(|i| counts[i] as f64), force).map(|x| {
        x.round()
            .clamp(-f64::from(MAX_DAC_OFFSET), f64::from(MAX_DAC_OFFSET)) as i16
    })
}

/// Unclamped model demand, used only to detect command saturation. These values
/// must never be sent directly to the transport.
fn raw_motor_offsets(counts: [f64; 3], force: [f64; 3]) -> [f64; 3] {
    let j = jacobian_from_counts(counts);
    [
        -825. * dot(j[0], force),
        -960. * dot(j[2], force),
        -960. * dot(j[1], force),
    ]
}

/// Software deadman, vector clamp and slew limit. Torque output remains unsupported
/// until the owner's possible wrist actuators and their protocol are verified.
#[derive(Default)]
pub struct ForceGuard {
    armed: bool,
    request: Option<(Wrench, Instant)>,
    last_tick: Option<Instant>,
    limits: ForceLimits,
    overload_since: Option<Instant>,
    /// True when the latest demand exceeded the configured force or DAC cap.
    pub command_limited: bool,
    /// Set until explicit disarm/rearm after a configured release trip.
    pub fault: Option<ForceFault>,
    /// Exact bounded offsets returned on the latest tick, before transport I/O.
    pub applied_motor_offsets: [i16; 3],
    /// Guarded model force before DAC conversion/clamping; not a force measurement.
    pub applied_force: [f64; 3],
}
impl ForceGuard {
    /// Configure while disarmed. Applications cannot raise any built-in ceiling.
    pub fn set_limits(&mut self, limits: ForceLimits) -> Result<(), &'static str> {
        if self.armed {
            return Err("disarm before changing output limits");
        }
        if !limits.valid() {
            return Err("limits must be finite, positive and no greater than SDK ceilings");
        }
        self.limits = limits;
        Ok(())
    }
    /// Reset pending demand and faults, then arm this software guard.
    /// This performs no I/O; use [`Device::enable_feedback`] for a device session.
    pub fn arm(&mut self) {
        self.disarm();
        self.armed = true;
    }
    /// Clear demand, faults and output state while retaining configured limits.
    /// This performs no transport shutdown; use [`Device::disable_feedback`] for hardware.
    pub fn disarm(&mut self) {
        let limits = self.limits;
        *self = Self {
            limits,
            ..Self::default()
        };
    }
    /// Queue a finite force request with its monotonic creation timestamp.
    /// Non-finite input or nonzero torque disarms the guard and returns an error.
    /// Queuing does not arm output; freshness is checked when the guard is serviced.
    pub fn request(&mut self, wrench: Wrench, time: Instant) -> Result<(), &'static str> {
        if wrench
            .force
            .iter()
            .chain(&wrench.torque)
            .any(|x| !x.is_finite())
        {
            self.disarm();
            return Err("non-finite wrench");
        }
        if wrench.torque.iter().any(|x| *x != 0.) {
            self.disarm();
            return Err("rotational motor output is not verified");
        }
        self.request = Some((wrench, time));
        Ok(())
    }
    /// Stateless raw-counter convenience entry point. Call [`Self::tick_continuous`]
    /// with unwrapped history for motion that can cross a signed counter boundary.
    /// [`Device`] always uses that continuous entry point.
    pub fn tick(&mut self, now: Instant, counts: [i16; 6]) -> [i16; 3] {
        self.tick_continuous(now, counts.map(i64::from))
    }
    /// Service output using the same unwrapped absolute history as device pose.
    pub fn tick_continuous(&mut self, now: Instant, counts: [i64; 6]) -> [i16; 3] {
        let counts = std::array::from_fn(|i| counts[i] as f64);
        self.applied_motor_offsets = [0; 3];
        let dt = self.last_tick.and_then(|t| now.checked_duration_since(t));
        self.last_tick = Some(now);
        if self.fault.is_some() {
            self.applied_force = [0.; 3];
            return [0; 3];
        }
        let valid = self.armed
            && self.request.is_some_and(|(_, t)| {
                now.checked_duration_since(t)
                    .is_some_and(|age| age <= COMMAND_TIMEOUT)
            });
        if !valid || dt.is_some_and(|d| d > COMMAND_TIMEOUT) {
            self.applied_force = [0.; 3];
            self.overload_since = None;
            self.command_limited = false;
            return [0; 3];
        }
        let demand = self.request.unwrap().0.force;
        let desired = limited(demand, self.limits.force_n).map(|x| x * UNIT_DERATING);
        let dac_limit = (f64::from(self.limits.dac_counts) * UNIT_DERATING).floor();
        self.command_limited = norm(demand) > self.limits.force_n + 1e-12
            || raw_motor_offsets(counts, desired)
                .iter()
                .any(|x| x.abs() > dac_limit);
        if self.command_limited {
            let since = *self.overload_since.get_or_insert(now);
            if self
                .limits
                .release_after
                .is_some_and(|delay| now.saturating_duration_since(since) >= delay)
            {
                self.fault = Some(ForceFault::SustainedCommandLimit);
                self.armed = false;
                self.applied_force = [0.; 3];
                return [0; 3];
            }
        } else {
            self.overload_since = None;
        }
        if desired == [0.; 3] {
            self.applied_force = [0.; 3];
            return [0; 3];
        }
        let delta = std::array::from_fn(|i| desired[i] - self.applied_force[i]);
        let step = limited(
            delta,
            self.limits.slew_n_s * UNIT_DERATING * dt.unwrap_or_default().as_secs_f64(),
        );
        for (f, d) in self.applied_force.iter_mut().zip(step) {
            *f += d;
        }
        self.applied_motor_offsets = raw_motor_offsets(counts, self.applied_force)
            .map(|x| x.round().clamp(-dac_limit, dac_limit) as i16);
        self.applied_motor_offsets
    }
}

/// Device-level force lifecycle, independent of graphics or world physics.
/// Call `sample` continuously on a worker; it services the deadman even when the
/// application stops supplying requests. Drop performs zero/shutdown cleanup.
pub struct Device {
    reader: haptics_hal::device::DeviceReader,
    /// Latest measured input and continuous counter history, updated by sample.
    pub state: EncoderState,
    /// Software output policy and command telemetry. Configure limits while disarmed.
    pub guard: ForceGuard,
    feedback: bool,
    last_force_fault: Option<ForceFault>,
}
impl Device {
    /// Claim and initialize the ppdev connection without enabling feedback.
    /// Performs active transport I/O; errors propagate from the HAL.
    pub fn connect(path: impl AsRef<std::path::Path>) -> std::io::Result<Self> {
        Ok(Self {
            reader: haptics_hal::device::DeviceReader::connect(path)?,
            state: EncoderState::default(),
            guard: ForceGuard::default(),
            feedback: false,
            last_force_fault: None,
        })
    }
    /// Begin the motor-session handshake and arm the guard with cleared demand.
    /// Idempotent while feedback is already enabled. Call only after application
    /// calibration, operator intent and other interlocks have been checked.
    pub fn enable_feedback(&mut self) -> std::io::Result<()> {
        if !self.feedback {
            self.reader.begin_force_session()?;
            self.guard.arm();
            self.feedback = true;
        }
        Ok(())
    }
    /// Disarm locally and attempt the transport neutral/shutdown sequence.
    /// An error means physical shutdown is unconfirmed; close and reconnect.
    pub fn disable_feedback(&mut self) -> std::io::Result<()> {
        self.guard.disarm();
        self.feedback = false;
        self.reader.end_force_session()
    }
    /// Queue demand for the next serviced sample; this does not enable feedback.
    /// Invalid demand triggers shutdown. A shutdown I/O error takes precedence
    /// over the invalid-demand error because physical release is then unconfirmed.
    pub fn request(&mut self, wrench: Wrench, time: Instant) -> std::io::Result<()> {
        if let Err(e) = self.guard.request(wrench, time) {
            self.disable_feedback()?;
            return Err(std::io::Error::other(e));
        }
        Ok(())
    }
    /// Read inputs, update kinematics and service the previously queued force.
    /// A newly read position reaches application interlocks only after that service.
    /// On failure, attempt shutdown and return an error; inspect [`Self::take_force_fault`]
    /// before another sample to distinguish a software release from an I/O failure.
    pub fn sample(&mut self) -> std::io::Result<Sample> {
        self.last_force_fault = None;
        let result = (|| {
            let s = self.reader.sample()?;
            self.state.update(s);
            if self.feedback {
                let offsets = self
                    .guard
                    .tick_continuous(Instant::now(), self.state.continuous_counts());
                if let Some(fault) = self.guard.fault {
                    self.last_force_fault = Some(fault);
                    return Err(std::io::Error::other(fault.message()));
                }
                self.reader.write_motor_offsets(offsets)?;
            }
            Ok(s)
        })();
        if result.is_err() {
            let shutdown = self.disable_feedback();
            finish_failed_sample(result, shutdown, &mut self.last_force_fault)
        } else {
            result
        }
    }
    /// Classify a failed sample before the next sample. A command-limit release
    /// has ended output but leaves a healthy input connection usable. An I/O
    /// failure returns `None` and requires closing/reconnecting. Consumption
    /// prevents a later transport fault being mistaken for an old force trip.
    pub fn take_force_fault(&mut self) -> Option<ForceFault> {
        self.last_force_fault.take()
    }
    /// Consume the connection, attempt shutdown and release the port.
    /// Propagates cleanup errors; Drop also attempts cleanup on an early error.
    pub fn close(mut self) -> std::io::Result<()> {
        self.disable_feedback()?;
        self.reader.close()
    }
}

// A failed neutral/shutdown transaction outranks a recoverable command trip.
fn finish_failed_sample<T>(
    result: std::io::Result<T>,
    shutdown: std::io::Result<()>,
    fault: &mut Option<ForceFault>,
) -> std::io::Result<T> {
    match shutdown {
        Ok(()) => result,
        Err(error) => {
            *fault = None;
            Err(std::io::Error::new(
                error.kind(),
                format!("output shutdown failed: {error}"),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reference_position_and_force_fixtures() {
        let c = [6298, -1052, 1100, 911, -123, -222];
        let p = position_m(c);
        for (a, b) in p
            .into_iter()
            .zip([-0.1514001923, -0.0287494202, -0.0698199844])
        {
            assert!((a - b).abs() < 0.00001, "{p:?}");
        }
        // Independent +1 N reference outputs, reduced to 0.5 N to avoid the
        // deliberately smaller per-channel limit in this implementation.
        for (f, expected) in [
            ([0.5, 0., 0.], [29, 60, 14]),
            ([0., 0.5, 0.], [0, -15, -65]),
            ([0., 0., 0.5], [63, -27, -6]),
        ] {
            let out = motor_offsets(c, f);
            for (a, b) in out.into_iter().zip(expected) {
                assert!((a - b).abs() <= 1, "{out:?}");
            }
        }
    }
    #[test]
    fn all_six_axes_wrap_and_recenter() {
        let packet = |counts: [i16; 6]| {
            let mut raw = [0; 16];
            for (i, v) in counts.into_iter().enumerate() {
                let w = [2, 0, 1, 3, 4, 5][i];
                raw[w * 2..w * 2 + 2].copy_from_slice(&v.to_le_bytes());
            }
            raw[13] = 0x1f;
            Sample { raw }
        };
        let mut s = EncoderState::default();
        s.update(packet([32767; 6]));
        s.update(packet([-32768; 6]));
        assert_eq!(s.relative_counts, [1; 6]);
        assert_eq!(s.continuous_counts(), [32768; 6]);
        let before = s.position_m();
        s.recenter();
        assert_eq!(s.position_m(), before);
        s.update(packet([-32767; 6]));
        assert_eq!(s.relative_counts, [1; 6]);
        assert_eq!(s.continuous_counts(), [32769; 6]);
        for (a, b) in before.into_iter().zip(s.position_m()) {
            assert!((a - b).abs() < 0.0002);
        }
        let j0 = jacobian_from_counts([32767.; 3]);
        let j1 = jacobian_from_counts([32768.; 3]);
        for (a, b) in j0.into_iter().flatten().zip(j1.into_iter().flatten()) {
            assert!((a - b).abs() < 0.0002);
        }
    }

    #[test]
    fn device_force_transform_stays_continuous_at_signed_rollover() {
        let t = Instant::now();
        let mut guard = ForceGuard::default();
        guard.arm();
        let wrench = Wrench {
            force: [0.3, 0.2, 0.1],
            ..Default::default()
        };
        let mut before = [0; 3];
        for i in 0..=500 {
            let now = t + Duration::from_millis(i);
            guard.request(wrench, now).unwrap();
            before = guard.tick_continuous(now, [32767; 6]);
        }
        let now = t + Duration::from_millis(501);
        guard.request(wrench, now).unwrap();
        let after = guard.tick_continuous(now, [32768; 6]);
        for (a, b) in before.into_iter().zip(after) {
            assert!((a - b).abs() <= 1);
        }
        assert_ne!(
            after,
            motor_offsets([-32768; 6], wrench.force),
            "fixture must distinguish wrapped from continuous transforms"
        );
    }

    #[test]
    fn shutdown_failure_overrides_recoverable_force_trip() {
        let mut fault = Some(ForceFault::SustainedCommandLimit);
        let result: std::io::Result<()> = Err(std::io::Error::other("command trip"));
        let error = finish_failed_sample(
            result,
            Err(std::io::Error::from(std::io::ErrorKind::TimedOut)),
            &mut fault,
        )
        .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert_eq!(fault, None);
        let mut fault = Some(ForceFault::SustainedCommandLimit);
        let result: std::io::Result<()> = Err(std::io::Error::other("command trip"));
        assert!(finish_failed_sample(result, Ok(()), &mut fault).is_err());
        assert_eq!(fault, Some(ForceFault::SustainedCommandLimit));
    }
    #[test]
    fn force_requires_arm_and_expires_to_zero() {
        let t = Instant::now();
        let mut g = ForceGuard::default();
        let c = [0; 6];
        let w = Wrench {
            force: [0., -100., 0.],
            ..Wrench::default()
        };
        g.request(w, t).unwrap();
        assert_eq!(g.tick(t, c), [0; 3]);
        g.arm();
        g.request(w, t).unwrap();
        g.tick(t, c);
        for i in 1..=500 {
            let now = t + Duration::from_millis(i);
            g.request(w, now).unwrap();
            g.tick(now, c);
        }
        assert!(norm(g.applied_force) <= MAX_FORCE_N + 1e-10);
        assert!(norm(g.applied_force) > 0.49);
        assert_eq!(g.tick(t + Duration::from_millis(526), c), [0; 3]);
        assert_eq!(g.applied_force, [0.; 3]);
        g.request(
            Wrench {
                torque: [0., 0., 1.],
                ..Wrench::default()
            },
            t,
        )
        .unwrap_err();
        g.request(
            Wrench {
                force: [f64::NAN, 0., 0.],
                ..Wrench::default()
            },
            t,
        )
        .unwrap_err();
    }

    #[test]
    fn stricter_limits_release_sustained_demand_and_latch_until_rearm() {
        let mut g = ForceGuard::default();
        let limits = ForceLimits {
            force_n: 0.1,
            release_after: Some(Duration::from_millis(250)),
            ..Default::default()
        };
        g.set_limits(limits).unwrap();
        g.arm();
        assert!(g.set_limits(limits).is_err());
        let t = Instant::now();
        for i in 0..=250 {
            let now = t + Duration::from_millis(i);
            g.request(
                Wrench {
                    force: [0., 1., 0.],
                    ..Default::default()
                },
                now,
            )
            .unwrap();
            let offsets = g.tick(now, [0; 6]);
            assert!(norm(g.applied_force) <= 0.1 + 1e-12);
            if i == 250 {
                assert_eq!(offsets, [0; 3]);
                assert_eq!(g.fault, Some(ForceFault::SustainedCommandLimit));
            }
        }
        let now = t + Duration::from_millis(251);
        g.request(
            Wrench {
                force: [0., 0.05, 0.],
                ..Default::default()
            },
            now,
        )
        .unwrap();
        assert_eq!(g.tick(now, [0; 6]), [0; 3]);
        g.arm();
        assert!(g.fault.is_none());
        g.request(
            Wrench {
                force: [0., 1., 0.],
                ..Default::default()
            },
            now,
        )
        .unwrap();
        g.tick(now, [0; 6]);
        g.request(
            Wrench {
                force: [0., 1., 0.],
                ..Default::default()
            },
            now + Duration::from_millis(1),
        )
        .unwrap();
        g.tick(now + Duration::from_millis(1), [0; 6]);
        assert!(g.applied_force[1] > 0. && g.applied_force[1] < 0.1);
    }

    #[test]
    fn saturation_timer_resets_and_invalid_limits_cannot_raise_sdk_ceiling() {
        let mut g = ForceGuard::default();
        for limits in [
            ForceLimits {
                force_n: f64::NAN,
                ..Default::default()
            },
            ForceLimits {
                force_n: 0.6,
                ..Default::default()
            },
            ForceLimits {
                dac_counts: 97,
                ..Default::default()
            },
            ForceLimits {
                slew_n_s: 3.,
                ..Default::default()
            },
        ] {
            assert!(g.set_limits(limits).is_err());
        }
        g.set_limits(ForceLimits {
            force_n: 0.1,
            release_after: Some(Duration::from_millis(100)),
            ..Default::default()
        })
        .unwrap();
        g.arm();
        let t = Instant::now();
        for i in 0..300 {
            let now = t + Duration::from_millis(i);
            let force = if i % 75 == 0 { [0.; 3] } else { [0., 1., 0.] };
            g.request(
                Wrench {
                    force,
                    ..Default::default()
                },
                now,
            )
            .unwrap();
            g.tick(now, [0; 6]);
            assert!(g.fault.is_none());
        }
    }
}
