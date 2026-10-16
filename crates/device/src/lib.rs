//! Reusable device state and bounded force feedback. See docs/spec/12-force-feedback.md.
//! The reference kinematic model is not a physical calibration of this unit.
pub mod shutdown;
use haptics_hal::device::Sample;
use std::time::{Duration, Instant};

pub const COMMAND_TIMEOUT: Duration = Duration::from_millis(25);
pub const MAX_FORCE_N: f64 = 0.5;
pub const MAX_DAC_OFFSET: i16 = 96;

/// World-space force and torque requested at the tool, in N and N·m.
#[derive(Clone, Copy, Debug, Default)]
pub struct Wrench {
    pub force: [f64; 3],
    pub torque: [f64; 3],
}

/// Retains every measured axis, including the three wrist/pen counters.
#[derive(Default)]
pub struct EncoderState {
    previous: Option<[i16; 6]>,
    pub counts: [i16; 6],
    pub relative_counts: [i64; 6],
    pub button_pressed: bool,
}
impl EncoderState {
    pub fn update(&mut self, sample: Sample) {
        self.counts = sample.encoder_counts();
        if let Some(previous) = self.previous {
            for (i, old) in previous.into_iter().enumerate() {
                self.relative_counts[i] += i64::from(self.counts[i].wrapping_sub(old));
            }
        }
        self.previous = Some(self.counts);
        self.button_pressed = sample.stylus_button_pressed();
    }
    pub fn recenter(&mut self) {
        self.relative_counts = [0; 6];
    }
    pub fn position_m(&self) -> [f64; 3] {
        position_m(self.counts)
    }
}

fn angles(counts: [f64; 3]) -> [f64; 3] {
    [
        counts[0] * 0.00018075138,
        -counts[2] * 0.00021136618,
        -counts[1] * 0.00021136618,
    ]
}

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
    let [a, b, c] = angles([counts[0] as f64, counts[1] as f64, counts[2] as f64]);
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
    let j = jacobian(counts);
    [
        -825. * dot(j[0], force),
        -960. * dot(j[2], force),
        -960. * dot(j[1], force),
    ]
    .map(|x| {
        x.round()
            .clamp(-f64::from(MAX_DAC_OFFSET), f64::from(MAX_DAC_OFFSET)) as i16
    })
}

/// Software deadman, vector clamp and slew limit. Torque output remains unsupported
/// until the owner's possible wrist actuators and their protocol are verified.
#[derive(Default)]
pub struct ForceGuard {
    armed: bool,
    request: Option<(Wrench, Instant)>,
    last_tick: Option<Instant>,
    pub applied_force: [f64; 3],
}
impl ForceGuard {
    pub fn arm(&mut self) {
        self.disarm();
        self.armed = true;
    }
    pub fn disarm(&mut self) {
        *self = Self::default();
    }
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
    pub fn tick(&mut self, now: Instant, counts: [i16; 6]) -> [i16; 3] {
        let dt = self.last_tick.and_then(|t| now.checked_duration_since(t));
        self.last_tick = Some(now);
        let valid = self.armed
            && self.request.is_some_and(|(_, t)| {
                now.checked_duration_since(t)
                    .is_some_and(|age| age <= COMMAND_TIMEOUT)
            });
        if !valid || dt.is_some_and(|d| d > COMMAND_TIMEOUT) {
            self.applied_force = [0.; 3];
            return [0; 3];
        }
        let desired = limited(self.request.unwrap().0.force, MAX_FORCE_N);
        if desired == [0.; 3] {
            self.applied_force = [0.; 3];
            return [0; 3];
        }
        let delta = std::array::from_fn(|i| desired[i] - self.applied_force[i]);
        let step = limited(delta, 2. * dt.unwrap_or_default().as_secs_f64());
        for (f, d) in self.applied_force.iter_mut().zip(step) {
            *f += d;
        }
        motor_offsets(counts, self.applied_force)
    }
}

/// Device-level force lifecycle, independent of graphics or world physics.
/// Call `sample` continuously on a worker; it services the deadman even when the
/// application stops supplying requests. Drop performs zero/shutdown cleanup.
pub struct Device {
    reader: haptics_hal::device::DeviceReader,
    pub state: EncoderState,
    pub guard: ForceGuard,
    feedback: bool,
}
impl Device {
    pub fn connect(path: impl AsRef<std::path::Path>) -> std::io::Result<Self> {
        Ok(Self {
            reader: haptics_hal::device::DeviceReader::connect(path)?,
            state: EncoderState::default(),
            guard: ForceGuard::default(),
            feedback: false,
        })
    }
    pub fn enable_feedback(&mut self) -> std::io::Result<()> {
        if !self.feedback {
            self.reader.begin_force_session()?;
            self.guard.arm();
            self.feedback = true;
        }
        Ok(())
    }
    pub fn disable_feedback(&mut self) -> std::io::Result<()> {
        self.guard.disarm();
        self.feedback = false;
        self.reader.end_force_session()
    }
    pub fn request(&mut self, wrench: Wrench, time: Instant) -> std::io::Result<()> {
        if let Err(e) = self.guard.request(wrench, time) {
            let _ = self.disable_feedback();
            return Err(std::io::Error::other(e));
        }
        Ok(())
    }
    pub fn sample(&mut self) -> std::io::Result<Sample> {
        let result = (|| {
            let s = self.reader.sample()?;
            self.state.update(s);
            if self.feedback {
                let offsets = self.guard.tick(Instant::now(), self.state.counts);
                self.reader.write_motor_offsets(offsets)?;
            }
            Ok(s)
        })();
        if result.is_err() {
            let _ = self.disable_feedback();
        }
        result
    }
    pub fn close(mut self) -> std::io::Result<()> {
        self.disable_feedback()?;
        self.reader.close()
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
        s.recenter();
        s.update(packet([-32767; 6]));
        assert_eq!(s.relative_counts, [1; 6]);
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
}
