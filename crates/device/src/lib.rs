//! Reusable device state and bounded force feedback. See docs/spec/12-force-feedback.md.
//!
//! **Commands are model values.** Every "newton" in this crate is a model command
//! derived from a reference kinematic model and fixed gains. Delivered force has not
//! been measured; only an attended session with a gauge or mass can establish it.
//! Direct wrist or pen torque is unsupported: [`Demand`] has no torque term.
//!
//! # Lifecycle
//!
//! 1. [`Device::connect`] with output disabled and call [`Device::sample`] on a
//!    dedicated worker continuously, even when the UI has no new demand.
//! 2. With the arm at its reset pose, [`Device::register_reset`]. Registration makes
//!    the reset pose the model origin. **Registration precedes any Cartesian demand**:
//!    a [`Demand::Force`] is refused while unregistered. The raw
//!    [`Demand::Channels`] liveness test is allowed unregistered.
//! 3. Optionally lower the limits with [`ForceGuard::set_limits`] (disarmed only),
//!    then [`Device::enable_feedback`] (zero-only handshake, then arm).
//! 4. Submit demands with [`Device::request`]. A request carries its original
//!    [`Instant`]; a request older than [`COMMAND_TIMEOUT`] yields zero. A servo loop
//!    that recomputes its demand from the sample it has just read may stamp it `now`.
//!    Never refresh the stamp of a request that was not recomputed.
//!
//! ```no_run
//! use visiontouch_device::{Demand, Device, ForceLimits};
//! use std::time::Instant;
//! # fn main() -> std::io::Result<()> {
//! let mut device = Device::connect("/dev/parport0")?;
//! device.sample()?; // input only; no output session yet
//! device.register_reset().map_err(std::io::Error::other)?;
//! device.guard.set_limits(ForceLimits {
//!     force_n: 0.25,
//!     ..Default::default()
//! }).map_err(std::io::Error::other)?;
//! device.enable_feedback()?; // observed zero-only arming handshake
//! // A real application repeats this on a dedicated worker with operator,
//! // pose, timing and travel interlocks; this example sends zero once.
//! device.request(Demand::Force([0.; 3]), Instant::now())
//!     .map_err(|e| std::io::Error::other(format!("{e:?}")))?;
//! device.sample()?;
//! device.close()?;
//! # Ok(()) }
//! ```
//!
//! # Envelope
//!
//! [`ForceLimits::default`] is the envelope armed live in P-0030: 0.5 N and
//! 96 counts. [`ForceLimits`] may be raised only up to the ceilings: 2 N and
//! 300 counts ([`visiontouch_hal::device::MAX_MOTOR_OFFSET`], 14.7% of the 2047-count
//! half-span), the latter independent of the force model. Content with no fixed
//! dwell stays at or below [`CONTINUOUS_FORCE_N`]. Counts are limited by scaling the
//! whole force vector uniformly, so a wall never gains tangential force. There is
//! no latched release trip: a limit caps, it does not disarm. Software cleanup
//! cannot cover SIGKILL or a kernel scheduling stall; no hardware watchdog is claimed.
#![warn(missing_docs)]
pub mod shutdown;
use std::time::{Duration, Instant};
use visiontouch_hal::device::{MAX_MOTOR_OFFSET, Sample};

/// Maximum request age and serviced-tick gap before the software guard returns zero.
/// Only evaluated when the worker runs; this is not an autonomous hardware watchdog.
pub const COMMAND_TIMEOUT: Duration = Duration::from_millis(25);
/// Ceiling on the model-force vector magnitude in newtons, not a measured rating.
pub const MAX_FORCE_N: f64 = 2.0;
/// Model-force cap of [`ForceLimits::default`]: the envelope armed live in P-0030.
pub const DEFAULT_FORCE_N: f64 = 0.5;
/// Cap for content with no fixed dwell (demos, Mass Lab), below the 1.4 N
/// continuous rating (P-0053). Larger forces belong only in short fixed-dwell steps.
pub const CONTINUOUS_FORCE_N: f64 = 1.0;
/// Ceiling on the absolute per-channel command offset from neutral, in DAC counts.
/// Equal to the transport backstop in the HAL.
pub const MAX_DAC_OFFSET: i16 = MAX_MOTOR_OFFSET;
/// Per-channel DAC cap of [`ForceLimits::default`]: the envelope armed in P-0030.
pub const DEFAULT_DAC_OFFSET: i16 = 96;
/// Cap for the raw [`Demand::Channels`] liveness test, in counts per channel.
pub const MAX_CHANNEL_TEST_OFFSET: i16 = 96;
/// Ceiling on the force-vector slew, in model N/s. About 0.24 N per 2.4 ms tick, so
/// the stiffest intended content at a brisk approach does not bind it.
pub const MAX_SLEW_N_S: f64 = 100.0;
const _: () = assert!(MAX_DAC_OFFSET <= MAX_MOTOR_OFFSET);
const _: () = assert!(DEFAULT_DAC_OFFSET <= MAX_DAC_OFFSET);
const _: () = assert!(MAX_CHANNEL_TEST_OFFSET <= MAX_DAC_OFFSET);

/// One unit-wide gain, applied exactly once by the guard to force, slew and DAC
/// ceilings. This scales the already conservative SDK envelope, not a historical
/// manufacturer rating. A gauge-measured slope may lower it by ADR. Keep in (0, 1].
pub const UNIT_DERATING: f64 = 1.0;
const _: () = assert!(UNIT_DERATING > 0. && UNIT_DERATING <= 1.);

/// Application output limits. Values may only lower the SDK ceilings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ForceLimits {
    /// Pre-derating force-vector magnitude cap in model newtons, in (0, [`MAX_FORCE_N`]].
    pub force_n: f64,
    /// Pre-derating force-vector slew cap in model N/s, in (0, [`MAX_SLEW_N_S`]].
    pub slew_n_s: f64,
    /// Pre-derating absolute per-channel DAC offset cap, in `1..=`[`MAX_DAC_OFFSET`].
    pub dac_counts: i16,
}
impl Default for ForceLimits {
    fn default() -> Self {
        Self {
            force_n: DEFAULT_FORCE_N,
            slew_n_s: MAX_SLEW_N_S,
            dac_counts: DEFAULT_DAC_OFFSET,
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
            && self.slew_n_s <= MAX_SLEW_N_S
            && self.dac_counts > 0
            && self.dac_counts <= MAX_DAC_OFFSET
    }
}

/// What the application asks the guard to emit. There is no torque term: wrist and
/// pen actuation is unsupported.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Demand {
    /// Cartesian force in model newtons in the device kinematic frame. Requires
    /// registration; the guard caps, slews and DAC-limits it.
    Force([f64; 3]),
    /// Raw per-channel DAC offsets for actuator liveness tests. Shares arm, expiry and
    /// both clamps, skips slew and the force model, and is capped at
    /// [`MAX_CHANNEL_TEST_OFFSET`] counts per channel.
    Channels([i16; 3]),
}

/// Encoder counts of the reset pose, captured by registration. Counts are relative to
/// the first sample of this connection, so a reconnect invalidates registration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Registration {
    /// Continuous (unwrapped) counts of all six channels at the reset pose. Only the
    /// first three drive the position model; all six are kept for the record.
    pub reset_counts: [i64; 6],
}

/// Per-tick guard telemetry, so an application can show "cap reached" and
/// "request expired". Describes commands, not delivered force.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Telemetry {
    /// The demand serviced on the latest tick, if any request is held.
    pub demand: Option<Demand>,
    /// Model force actually sent on the latest tick, after every limit.
    pub applied_force: [f64; 3],
    /// Exact offsets returned on the latest tick, before transport I/O.
    pub applied_motor_offsets: [i16; 3],
    /// The force magnitude cap reduced the desired force.
    pub force_capped: bool,
    /// The DAC cap scaled the force (or clamped a channel demand).
    pub dac_capped: bool,
    /// The slew limit held the applied force below the desired force.
    pub slewing: bool,
    /// A held request was stale, or the servo gap exceeded [`COMMAND_TIMEOUT`].
    pub expired: bool,
}

/// Why [`Device::request`] refused a demand.
#[derive(Debug)]
pub enum RequestError {
    /// Rejected; output was shut down cleanly and the connection is still usable.
    /// The reason is `"non-finite"`, `"unregistered"` or `"not armed"`.
    Rejected(&'static str),
    /// Shutdown I/O failed after a rejection; close and reconnect.
    ShutdownFailed(std::io::Error),
}
impl std::fmt::Display for RequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(reason) => write!(f, "demand rejected: {reason}; output shut down"),
            Self::ShutdownFailed(e) => write!(f, "output shutdown failed: {e}"),
        }
    }
}
impl std::error::Error for RequestError {}

/// Retains every measured axis, including the three wrist/pen counters, and the
/// reset-pose registration that makes the main-arm position meaningful.
#[derive(Default)]
pub struct EncoderState {
    previous: Option<[i16; 6]>,
    continuous_counts: [i64; 6],
    registration: Option<Registration>,
    /// Latest raw signed 16-bit counters for all six measured channels.
    pub counts: [i16; 6],
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
                self.continuous_counts[i] += i64::from(self.counts[i].wrapping_sub(old));
            }
        } else {
            self.continuous_counts = self.counts.map(i64::from);
        }
        self.previous = Some(self.counts);
        self.button_pressed = sample.stylus_button_pressed();
    }
    /// Unwrapped counters since the first sample of this connection.
    /// Successive samples must be less than half a counter revolution apart.
    pub fn continuous_counts(&self) -> [i64; 6] {
        self.continuous_counts
    }
    /// Record the current pose as the reset pose. `None` until a sample has arrived.
    /// Applications should call [`Device::register_reset`], which also stops output.
    pub fn register_reset(&mut self) -> Option<Registration> {
        self.previous?;
        let r = Registration {
            reset_counts: self.continuous_counts,
        };
        self.registration = Some(r);
        Some(r)
    }
    /// Forget the registration.
    pub fn clear_registration(&mut self) {
        self.registration = None;
    }
    /// The reset-pose registration, if captured on this connection.
    pub fn registration(&self) -> Option<Registration> {
        self.registration
    }
    /// Continuous counts minus the reset counts; `None` while unregistered.
    pub fn registered_counts(&self) -> Option<[i64; 6]> {
        let r = self.registration?;
        Some(std::array::from_fn(|i| {
            self.continuous_counts[i] - r.reset_counts[i]
        }))
    }
    /// Model position in metres relative to the reset pose; `None` while unregistered.
    pub fn position_m(&self) -> Option<[f64; 3]> {
        let c = self.registered_counts()?;
        Some(position_from_counts(std::array::from_fn(|i| c[i] as f64)))
    }
    /// Position from power-on-relative counts, for display only. It is *not* the model
    /// frame the guard uses: label it unregistered and never derive a demand from it.
    pub fn unregistered_position_m(&self) -> [f64; 3] {
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

/// Model position in metres from *registered* main-arm counts (continuous counts
/// minus the reset-pose counts). Accumulated counts keep a signed 16-bit rollover
/// from jumping the pose. Registered counts `[0, 0, 0]` are the model origin.
pub fn position_from_counts(counts: [f64; 3]) -> [f64; 3] {
    let [a, b, c] = angles(counts);
    let r = 0.1397 * (b.cos() + c.sin());
    [
        -r * a.sin(),
        0.1397 * (b.sin() - c.cos() + 1.),
        r * a.cos() - 0.1397,
    ]
}

/// Jacobian rows in the order `d(position) / d(joint angle)`, for registered main-arm
/// counts. Force is transformed by J^T. Takes the same registered counts as
/// [`position_from_counts`].
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

/// Unclamped channel demand in DAC counts for a model force at registered counts.
///
/// Display and analysis only: the values are *not* limited and must never be sent to
/// the transport. [`ForceGuard`] performs the limiting.
pub fn motor_demand_counts(counts: [f64; 3], force: [f64; 3]) -> [f64; 3] {
    let j = jacobian_from_counts(counts);
    [
        -825. * dot(j[0], force),
        -960. * dot(j[2], force),
        -960. * dot(j[1], force),
    ]
}

/// Software deadman, force cap, slew limit and uniform DAC scaling.
///
/// Tick order: expiry and gap check; channel demand; unregistered force is zero;
/// force cap; immediate zero for a zero desired force; slew; DAC scaling; rounding.
/// The HAL clamp in the transport is an independent backstop. Wrist output is
/// unsupported.
#[derive(Default)]
pub struct ForceGuard {
    armed: bool,
    request: Option<(Demand, Instant)>,
    last_tick: Option<Instant>,
    limits: ForceLimits,
    telemetry: Telemetry,
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
    /// The configured limits.
    pub fn limits(&self) -> ForceLimits {
        self.limits
    }
    /// Reset pending demand and telemetry, then arm this software guard.
    /// This performs no I/O; use [`Device::enable_feedback`] for a device session.
    pub fn arm(&mut self) {
        self.disarm();
        self.armed = true;
    }
    /// Clear demand and output state while retaining configured limits.
    /// This performs no transport shutdown; use [`Device::disable_feedback`] for hardware.
    pub fn disarm(&mut self) {
        let limits = self.limits;
        *self = Self {
            limits,
            ..Self::default()
        };
    }
    /// Queue a demand with its monotonic creation timestamp. A non-finite force
    /// disarms the guard and returns an error. Queuing does not arm output; freshness
    /// is checked when the guard is serviced. Registration is checked at [`Self::tick`].
    pub fn request(&mut self, demand: Demand, time: Instant) -> Result<(), &'static str> {
        if let Demand::Force(f) = demand
            && f.iter().any(|x| !x.is_finite())
        {
            self.disarm();
            return Err("non-finite");
        }
        self.request = Some((demand, time));
        Ok(())
    }
    /// The latest tick's telemetry. Describes commands, not delivered force.
    pub fn telemetry(&self) -> Telemetry {
        self.telemetry
    }
    fn zero(&mut self, expired: bool) -> [i16; 3] {
        self.telemetry = Telemetry {
            demand: self.request.map(|r| r.0),
            expired,
            ..Telemetry::default()
        };
        [0; 3]
    }
    /// Service output. `counts` are *registered* counts, `None` while unregistered
    /// (a [`Demand::Force`] then yields zero). Returns the DAC offsets to send.
    pub fn tick(&mut self, now: Instant, counts: Option<[i64; 6]>) -> [i16; 3] {
        let dt = self.last_tick.and_then(|t| now.checked_duration_since(t));
        self.last_tick = Some(now);
        let Some((demand, stamped)) = self.request.filter(|_| self.armed) else {
            self.telemetry = Telemetry::default();
            return [0; 3];
        };
        let fresh = now
            .checked_duration_since(stamped)
            .is_some_and(|age| age <= COMMAND_TIMEOUT);
        if !fresh || dt.is_some_and(|d| d > COMMAND_TIMEOUT) {
            return self.zero(true);
        }
        let dac_limit = (f64::from(self.limits.dac_counts) * UNIT_DERATING).floor();
        let mut t = Telemetry {
            demand: Some(demand),
            ..Telemetry::default()
        };
        let offsets = match demand {
            Demand::Channels(c) => {
                let cap = f64::from(MAX_CHANNEL_TEST_OFFSET).min(dac_limit) as i16;
                t.dac_capped = c.iter().any(|x| x.unsigned_abs() > cap.unsigned_abs());
                c.map(|x| x.clamp(-cap, cap))
            }
            Demand::Force(demand) => match counts {
                None => [0; 3],
                Some(counts) => {
                    let cap = self.limits.force_n * UNIT_DERATING;
                    let desired = limited(demand, cap);
                    t.force_capped = norm(demand) > cap + 1e-12;
                    if desired == [0.; 3] {
                        [0; 3]
                    } else {
                        let max_step = self.limits.slew_n_s
                            * UNIT_DERATING
                            * dt.unwrap_or_default().as_secs_f64();
                        let applied = self.telemetry.applied_force;
                        let delta = std::array::from_fn(|i| desired[i] - applied[i]);
                        let step = limited(delta, max_step);
                        let mut f: [f64; 3] = std::array::from_fn(|i| applied[i] + step[i]);
                        let c = std::array::from_fn(|i| counts[i] as f64);
                        let raw = motor_demand_counts(c, f);
                        let largest = raw.iter().fold(0_f64, |m, x| m.max(x.abs()));
                        if largest > dac_limit {
                            t.dac_capped = true;
                            f = f.map(|x| x * dac_limit / largest);
                        }
                        // A DAC-limited output is not slew-limited, so the flag settles.
                        t.slewing = !t.dac_capped && norm(delta) > max_step + 1e-12;
                        t.applied_force = f;
                        motor_demand_counts(c, f)
                            .map(|x| x.round().clamp(-dac_limit, dac_limit) as i16)
                    }
                }
            },
        };
        t.applied_motor_offsets = offsets;
        self.telemetry = t;
        offsets
    }
}

/// Device-level force lifecycle, independent of graphics or world physics.
/// Call `sample` continuously on a worker; it services the deadman even when the
/// application stops supplying requests. Drop performs zero/shutdown cleanup.
/// A reconnect is a new `Device`, so registration never survives it.
pub struct Device {
    reader: visiontouch_hal::device::DeviceReader,
    /// Latest measured input, continuous counter history and registration.
    pub state: EncoderState,
    /// Software output policy and command telemetry. Configure limits while disarmed.
    pub guard: ForceGuard,
    feedback: bool,
}
impl Device {
    /// Claim and initialize the ppdev connection without enabling feedback.
    /// Performs active transport I/O; errors propagate from the HAL.
    pub fn connect(path: impl AsRef<std::path::Path>) -> std::io::Result<Self> {
        Ok(Self {
            reader: visiontouch_hal::device::DeviceReader::connect(path)?,
            state: EncoderState::default(),
            guard: ForceGuard::default(),
            feedback: false,
        })
    }
    /// Whether the motor session is active and the guard armed.
    pub fn feedback_enabled(&self) -> bool {
        self.feedback
    }
    /// Capture the current pose as the reset pose. Needs at least one sample. Turns
    /// feedback off first; if that shutdown fails the error asks for a reconnect.
    pub fn register_reset(&mut self) -> Result<Registration, &'static str> {
        if self.state.previous.is_none() {
            return Err("no sample yet; sample before registering");
        }
        if self.disable_feedback().is_err() {
            return Err("output shutdown failed; close and reconnect");
        }
        self.state.register_reset().ok_or("no sample yet")
    }
    /// Forget the registration. Turns feedback off; a failed shutdown is reported by
    /// the next [`Self::disable_feedback`] or [`Self::close`].
    pub fn clear_registration(&mut self) {
        let _ = self.disable_feedback();
        self.state.clear_registration();
    }
    /// Begin the motor-session handshake and arm the guard with cleared demand.
    /// Idempotent while feedback is already enabled. Allowed while unregistered (the
    /// channel test needs it); a Cartesian demand is still refused. Call only after
    /// operator intent and other interlocks have been checked.
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
    /// Queue a demand for the next serviced sample. Rejects non-finite input, a
    /// Cartesian force while unregistered, and any demand while not armed; each
    /// rejection shuts output down. A shutdown I/O error takes precedence over the
    /// rejection because physical release is then unconfirmed.
    pub fn request(&mut self, demand: Demand, time: Instant) -> Result<(), RequestError> {
        let verdict = if !self.feedback {
            Err("not armed")
        } else if matches!(demand, Demand::Force(_)) && self.state.registration().is_none() {
            Err("unregistered")
        } else {
            self.guard.request(demand, time)
        };
        match verdict {
            Ok(()) => Ok(()),
            Err(reason) => Err(reject(reason, self.disable_feedback())),
        }
    }
    /// Read inputs, update kinematics and service the previously queued demand.
    /// A newly read position reaches application interlocks only after that service.
    /// Every error is a transport fault: output was shut down (or its failure is
    /// reported) and the connection should be closed and reopened.
    pub fn sample(&mut self) -> std::io::Result<Sample> {
        let result = (|| {
            let s = self.reader.sample()?;
            self.state.update(s);
            if self.feedback {
                let offsets = self
                    .guard
                    .tick(Instant::now(), self.state.registered_counts());
                self.reader.write_motor_offsets(offsets)?;
            }
            Ok(s)
        })();
        match result {
            Ok(s) => Ok(s),
            Err(e) => match self.disable_feedback() {
                Ok(()) => Err(e),
                Err(shutdown) => Err(std::io::Error::new(
                    shutdown.kind(),
                    format!("output shutdown failed: {shutdown}; after: {e}"),
                )),
            },
        }
    }
    /// Consume the connection, attempt shutdown and release the port.
    /// Propagates cleanup errors; Drop also attempts cleanup on an early error.
    pub fn close(mut self) -> std::io::Result<()> {
        self.disable_feedback()?;
        self.reader.close()
    }
}

// A failed neutral/shutdown transaction outranks the rejection that caused it.
fn reject(reason: &'static str, shutdown: std::io::Result<()>) -> RequestError {
    match shutdown {
        Ok(()) => RequestError::Rejected(reason),
        Err(e) => RequestError::ShutdownFailed(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const TICK: Duration = Duration::from_micros(2400);
    const ZERO6: Option<[i64; 6]> = Some([0; 6]);

    fn armed(limits: ForceLimits) -> ForceGuard {
        let mut g = ForceGuard::default();
        g.set_limits(limits).unwrap();
        g.arm();
        g
    }
    fn wide() -> ForceLimits {
        ForceLimits {
            force_n: MAX_FORCE_N,
            slew_n_s: MAX_SLEW_N_S,
            dac_counts: MAX_DAC_OFFSET,
        }
    }
    fn packet(counts: [i16; 6]) -> Sample {
        let mut raw = [0; 16];
        for (i, v) in counts.into_iter().enumerate() {
            let w = [2, 0, 1, 3, 4, 5][i];
            raw[w * 2..w * 2 + 2].copy_from_slice(&v.to_le_bytes());
        }
        raw[13] = 0x1f;
        Sample { raw }
    }
    fn counts3(c: [i64; 6]) -> [f64; 3] {
        std::array::from_fn(|i| c[i] as f64)
    }

    #[test]
    fn reference_position_and_force_fixtures() {
        let c = [6298, -1052, 1100, 911, -123, -222];
        let c3 = counts3(c);
        let p = position_from_counts(c3);
        for (a, b) in p
            .into_iter()
            .zip([-0.1514001923, -0.0287494202, -0.0698199844])
        {
            assert!((a - b).abs() < 0.00001, "{p:?}");
        }
        // Independent +1 N reference outputs, reduced to 0.5 N.
        for (f, expected) in [
            ([0.5, 0., 0.], [29, 60, 14]),
            ([0., 0.5, 0.], [0, -15, -65]),
            ([0., 0., 0.5], [63, -27, -6]),
        ] {
            let out = motor_demand_counts(c3, f).map(|x| x.round() as i16);
            for (a, b) in out.into_iter().zip(expected) {
                assert!((a - b).abs() <= 1, "{out:?}");
            }
        }
    }

    #[test]
    fn default_limits_are_the_p0030_envelope() {
        let l = ForceLimits::default();
        assert_eq!((l.force_n, l.dac_counts), (0.5, 96));
        assert_eq!(l.slew_n_s, MAX_SLEW_N_S);
        assert_eq!(ForceGuard::default().limits(), l);
        assert_eq!(MAX_DAC_OFFSET, 300);
    }

    #[test]
    fn limits_cannot_exceed_ceilings() {
        let mut g = ForceGuard::default();
        for bad in [
            ForceLimits {
                force_n: 2.01,
                ..Default::default()
            },
            ForceLimits {
                dac_counts: 301,
                ..Default::default()
            },
            ForceLimits {
                slew_n_s: 101.,
                ..Default::default()
            },
            ForceLimits {
                force_n: f64::NAN,
                ..Default::default()
            },
            ForceLimits {
                slew_n_s: f64::NAN,
                ..Default::default()
            },
            ForceLimits {
                force_n: 0.,
                ..Default::default()
            },
            ForceLimits {
                dac_counts: 0,
                ..Default::default()
            },
        ] {
            assert!(g.set_limits(bad).is_err(), "{bad:?}");
        }
        g.set_limits(wide()).unwrap();
        assert_eq!(g.limits(), wide());
    }

    #[test]
    fn set_limits_refused_while_armed() {
        let mut g = ForceGuard::default();
        g.arm();
        assert!(g.set_limits(ForceLimits::default()).is_err());
        g.disarm();
        assert!(g.set_limits(ForceLimits::default()).is_ok());
    }

    #[test]
    fn force_requires_arm_registration_and_expires_to_zero() {
        let t = Instant::now();
        let mut g = ForceGuard::default();
        let d = Demand::Force([0., -100., 0.]);
        g.request(d, t).unwrap();
        assert_eq!(g.tick(t, ZERO6), [0; 3], "disarmed");
        g.arm();
        g.request(d, t).unwrap();
        g.tick(t, ZERO6);
        for i in 1..=100 {
            let now = t + Duration::from_millis(i);
            g.request(d, now).unwrap();
            assert_eq!(g.tick(now, None), [0; 3], "unregistered");
        }
        let mut last = [0; 3];
        for i in 101..=300 {
            let now = t + Duration::from_millis(i);
            g.request(d, now).unwrap();
            last = g.tick(now, ZERO6);
        }
        let f = g.telemetry().applied_force;
        assert!(
            norm(f) <= DEFAULT_FORCE_N + 1e-10 && norm(f) > 0.49,
            "{f:?}"
        );
        assert!(g.telemetry().force_capped);
        assert_ne!(last, [0; 3]);
        assert_eq!(g.tick(t + Duration::from_millis(326), ZERO6), [0; 3]);
        assert!(g.telemetry().expired);
        assert_eq!(g.telemetry().applied_force, [0.; 3]);
        assert!(g.request(Demand::Force([f64::NAN, 0., 0.]), t).is_err());
        assert_eq!(g.tick(t + Duration::from_millis(327), ZERO6), [0; 3]);
    }

    #[test]
    fn channel_demand_bypasses_slew_but_not_expiry_or_96_cap() {
        let t = Instant::now();
        let mut g = armed(wide());
        g.request(Demand::Channels([80, -500, 0]), t).unwrap();
        g.tick(t, None);
        let now = t + TICK;
        g.request(Demand::Channels([80, -500, 0]), now).unwrap();
        assert_eq!(
            g.tick(now, None),
            [80, -96, 0],
            "no slew, no registration, 96 cap"
        );
        let tel = g.telemetry();
        assert!(tel.dac_capped && !tel.slewing);
        assert_eq!(tel.applied_force, [0.; 3]);
        let late = now + Duration::from_millis(26);
        assert_eq!(g.tick(late, None), [0; 3]);
        assert!(g.telemetry().expired);
        let mut tight = armed(ForceLimits {
            dac_counts: 50,
            ..Default::default()
        });
        tight.request(Demand::Channels([90, 0, 0]), t).unwrap();
        tight.tick(t, None);
        tight.request(Demand::Channels([90, 0, 0]), now).unwrap();
        assert_eq!(tight.tick(now, None), [50, 0, 0]);
    }

    #[test]
    fn zero_demand_is_immediate() {
        let t = Instant::now();
        let mut g = armed(wide());
        let mut now = t;
        g.tick(now, ZERO6);
        for _ in 0..20 {
            now += TICK;
            g.request(Demand::Force([0., 1., 0.]), now).unwrap();
            g.tick(now, ZERO6);
        }
        assert!(g.telemetry().applied_force[1] > 0.5);
        now += TICK;
        g.request(Demand::Force([0.; 3]), now).unwrap();
        assert_eq!(g.tick(now, ZERO6), [0; 3]);
        assert_eq!(g.telemetry().applied_force, [0.; 3]);
        now += TICK;
        g.request(Demand::Force([0., 1., 0.]), now).unwrap();
        g.tick(now, ZERO6);
        assert!(
            g.telemetry().applied_force[1] <= 100. * TICK.as_secs_f64() + 1e-9,
            "slew restarts"
        );
    }

    #[test]
    fn dac_scaling_preserves_direction() {
        // r = 1.4 L: sin c = 0.4 at b = 0, a = 0. 2 N along X needs about 322 counts and
        // 2 N along XZ about 228, so a 200-count limit binds in both cases.
        let counts = [
            0,
            -(0.4_f64.asin() / 0.00021136618).round() as i64,
            0,
            0,
            0,
            0,
        ];
        for dir in [[1., 0., 0.], [1., 0., 1.]] {
            let n = norm(dir);
            let demand = dir.map(|x| 2. * x / n);
            let t = Instant::now();
            let mut g = armed(ForceLimits {
                dac_counts: 200,
                ..wide()
            });
            let mut now = t;
            g.tick(now, Some(counts));
            let mut out = [0; 3];
            for _ in 0..30 {
                now += Duration::from_millis(10);
                g.request(Demand::Force(demand), now).unwrap();
                out = g.tick(now, Some(counts));
            }
            let tel = g.telemetry();
            assert!(tel.dac_capped, "{tel:?}");
            assert_eq!(out.iter().map(|x| x.abs()).max(), Some(200));
            let f = tel.applied_force;
            let cos = dot(f, demand) / (norm(f) * norm(demand));
            assert!(cos.min(1.).acos().to_degrees() < 0.5, "{f:?}");
            assert!(norm(f) < 2.0);
        }
    }

    #[test]
    fn slewing_flag_settles_when_the_dac_cap_binds() {
        // 1 N along -Y at the reset pose needs about 134 counts; the default 96 binds.
        let t = Instant::now();
        let mut g = armed(ForceLimits {
            force_n: 1.0,
            ..ForceLimits::default()
        });
        g.tick(t, ZERO6);
        let mut now = t;
        for _ in 0..50 {
            now += TICK;
            g.request(Demand::Force([0., -1., 0.]), now).unwrap();
            g.tick(now, ZERO6);
        }
        let tel = g.telemetry();
        assert!(tel.dac_capped && !tel.slewing && !tel.expired, "{tel:?}");
        assert_eq!(
            tel.applied_motor_offsets.iter().map(|x| x.abs()).max(),
            Some(96)
        );
    }

    #[test]
    fn slew_reaches_half_newton_within_three_ticks_at_2_4_ms() {
        let t = Instant::now();
        let mut g = armed(ForceLimits::default());
        g.tick(t, ZERO6);
        let mut now = t;
        for _ in 0..3 {
            now += TICK;
            g.request(Demand::Force([0., 0.5, 0.]), now).unwrap();
            g.tick(now, ZERO6);
        }
        assert!((g.telemetry().applied_force[1] - 0.5).abs() < 1e-9);
        assert!(!g.telemetry().slewing);
    }

    #[test]
    fn registered_reset_pose_is_origin_and_reconnect_clears() {
        let mut s = EncoderState::default();
        assert!(s.register_reset().is_none(), "needs a sample");
        s.update(packet([1200, -300, 450, 7, 8, 9]));
        assert!(s.registration().is_none() && s.position_m().is_none());
        assert_ne!(s.unregistered_position_m(), [0.; 3]);
        let r = s.register_reset().unwrap();
        assert_eq!(r.reset_counts, [1200, -300, 450, 7, 8, 9]);
        assert_eq!(s.registered_counts(), Some([0; 6]));
        let p = s.position_m().unwrap();
        assert!(p.iter().all(|x| x.abs() < 1e-12), "{p:?}");
        s.update(packet([1210, -300, 450, 7, 8, 9]));
        assert_eq!(s.registered_counts().unwrap()[0], 10);
        s.clear_registration();
        assert!(s.position_m().is_none());
        // A reconnect constructs a new state: registration cannot survive it.
        assert!(EncoderState::default().registration().is_none());
    }

    #[test]
    fn all_six_axes_wrap_and_stay_registered() {
        let mut s = EncoderState::default();
        s.update(packet([32767; 6]));
        s.register_reset().unwrap();
        s.update(packet([-32768; 6]));
        assert_eq!(s.continuous_counts(), [32768; 6]);
        assert_eq!(s.registered_counts(), Some([1; 6]));
        s.update(packet([-32767; 6]));
        assert_eq!(s.registered_counts(), Some([2; 6]));
        let j0 = jacobian_from_counts([32767.; 3]);
        let j1 = jacobian_from_counts([32768.; 3]);
        for (a, b) in j0.into_iter().flatten().zip(j1.into_iter().flatten()) {
            assert!((a - b).abs() < 0.0002);
        }
    }

    #[test]
    fn device_force_transform_stays_continuous_at_signed_rollover() {
        let t = Instant::now();
        let mut g = armed(wide());
        let demand = Demand::Force([0.3, 0.2, 0.1]);
        let mut before = [0; 3];
        for i in 0..=500 {
            let now = t + Duration::from_millis(i);
            g.request(demand, now).unwrap();
            before = g.tick(now, Some([32767; 6]));
        }
        let now = t + Duration::from_millis(501);
        g.request(demand, now).unwrap();
        let after = g.tick(now, Some([32768; 6]));
        for (a, b) in before.into_iter().zip(after) {
            assert!((a - b).abs() <= 1);
        }
        let wrapped = motor_demand_counts([-32768.; 3], [0.3, 0.2, 0.1]).map(|x| x.round() as i16);
        assert_ne!(
            after, wrapped,
            "fixture must distinguish wrapped from continuous"
        );
    }

    #[test]
    fn shutdown_failure_outranks_rejection() {
        let failed = Err(std::io::Error::from(std::io::ErrorKind::TimedOut));
        match reject("unregistered", failed) {
            RequestError::ShutdownFailed(e) => assert_eq!(e.kind(), std::io::ErrorKind::TimedOut),
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            reject("unregistered", Ok(())),
            RequestError::Rejected("unregistered")
        ));
    }
}
