//! Simulated plant for `--demo`, `--smoke-test` and tests. No hardware is opened.
//!
//! [`SimPlant`] is a point mass in registered model metres driven by the delivered
//! force: the guard's channel offsets converted back through the same linear map the
//! SDK uses, scaled by a gain factor. Counts are quantised, so one-count flicker is
//! real. [`SimHw`] runs the *real* [`EncoderState`] and [`ForceGuard`] on top of it,
//! on a virtual clock, so everything above the plant is the production code path.
use crate::worker::Hardware;
use std::time::{Duration, Instant};
use visiontouch_device::{
    Demand, EncoderState, ForceGuard, ForceLimits, Registration, RequestError, motor_demand_counts,
    position_from_counts,
};
use visiontouch_effects::math::{V, add, mul, norm, sub};
use visiontouch_hal::device::Sample;

/// What the pen is doing in the simulated world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fixture {
    /// Floated on a light fingertip: weak spring to the hand point, viscous drag.
    Free,
    /// Pressed against a rigid scale, gauge or post at `anchor`.
    Braced {
        /// Contact point in registered metres.
        anchor: V,
    },
    /// Resting on a book stack: weight pulls down, the stack holds it up.
    Stack {
        /// Surface height in registered metres.
        floor_y: f64,
        /// Extra hung weight in newtons.
        hung_n: f64,
    },
    /// Released onto a palm 20 mm below `from`: gravity only.
    Dropped {
        /// Palm height in registered metres.
        floor_y: f64,
    },
}

/// Point-mass pen model.
#[derive(Clone, Debug)]
pub struct SimPlant {
    /// Delivered force over commanded force. The ladder should measure about this.
    pub gain: f64,
    /// Pen weight in newtons.
    pub weight_n: f64,
    /// Coulomb friction in newtons.
    pub friction_n: f64,
    /// Moving mass in kilograms.
    pub mass_kg: f64,
    /// Viscous drag of the holding fingers, N s/m.
    pub damping: f64,
    /// Spring toward the hand point in `Free`, N/m.
    pub hand_k: f64,
    /// Channel `i` of the drive acts on motor `drive_map[i]` (identity by default).
    pub drive_map: [usize; 3],
    /// Pen position, registered metres.
    pub pos: V,
    /// Pen velocity, metres per second.
    pub vel: V,
    /// Hand point for `Free`.
    pub hand: V,
    /// Current world.
    pub fixture: Fixture,
    /// Pen button held down.
    pub button: bool,
    /// Largest contact force seen on a `Braced` fixture since cleared, newtons.
    pub brace_peak_n: f64,
    q_ref: [f64; 3],
    origin: [i16; 3],
    c: [f64; 3],
}

impl Default for SimPlant {
    fn default() -> Self {
        Self {
            gain: 0.6,
            weight_n: 0.3,
            friction_n: 0.04,
            mass_kg: 0.2,
            damping: 12.,
            hand_k: 30.,
            drive_map: [0, 1, 2],
            pos: [0.; 3],
            vel: [0.; 3],
            hand: [0.; 3],
            fixture: Fixture::Free,
            button: false,
            brace_peak_n: 0.,
            q_ref: [800., -300., 450.],
            origin: [1200, -300, 450],
            c: [0.; 3],
        }
    }
}

fn solve3(m: [[f64; 3]; 3], b: V) -> Option<V> {
    let det = |a: [[f64; 3]; 3]| {
        a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1])
            - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
            + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0])
    };
    let d = det(m);
    if d.abs() < 1e-30 {
        return None;
    }
    let mut out = [0.; 3];
    for (k, o) in out.iter_mut().enumerate() {
        let mut t = m;
        for r in 0..3 {
            t[r][k] = b[r];
        }
        *o = det(t) / d;
    }
    Some(out)
}

impl SimPlant {
    /// Joint counts relative to the reset pose that put the tip at `p`.
    fn inverse(&self, p: V) -> [f64; 3] {
        let mut c = self.c;
        for _ in 0..8 {
            let f = position_from_counts(c);
            let err = sub(f, p);
            if norm(err) < 1e-10 {
                break;
            }
            let mut j = [[0.; 3]; 3];
            for k in 0..3 {
                let mut d = c;
                d[k] += 1.;
                let g = position_from_counts(d);
                for r in 0..3 {
                    j[r][k] = g[r] - f[r];
                }
            }
            match solve3(j, err) {
                Some(s) => {
                    for k in 0..3 {
                        c[k] -= s[k];
                    }
                }
                None => break,
            }
        }
        c
    }
    /// Force delivered for channel offsets at the current pose.
    pub fn delivered_force(&self, offsets: [i16; 3]) -> V {
        let mut eff = [0.; 3];
        for (i, o) in offsets.iter().enumerate() {
            eff[self.drive_map[i].min(2)] = f64::from(*o);
        }
        let cols: Vec<[f64; 3]> = (0..3)
            .map(|i| {
                let mut u = [0.; 3];
                u[i] = 1.;
                motor_demand_counts(self.c, u)
            })
            .collect();
        let m = std::array::from_fn(|r| std::array::from_fn(|k| cols[k][r]));
        solve3(m, eff).map_or([0.; 3], |f| mul(f, self.gain))
    }
    /// Place the pen at `p` at rest.
    pub fn teleport(&mut self, p: V) {
        self.pos = p;
        self.vel = [0.; 3];
        self.c = self.inverse(p);
    }
    /// Move the hand point and, if floated, let the pen follow.
    pub fn set_hand(&mut self, p: V) {
        self.hand = p;
    }
    /// Press the pen against a rigid object at the current position.
    pub fn brace(&mut self) {
        self.fixture = Fixture::Braced { anchor: self.pos };
        self.brace_peak_n = 0.;
    }
    /// Float the pen on a fingertip at the current position.
    pub fn float(&mut self) {
        self.fixture = Fixture::Free;
        self.hand = self.pos;
    }
    /// Advance by `dt` seconds under the given channel offsets; returns raw counts.
    pub fn step(&mut self, offsets: [i16; 3], dt: f64) -> [i16; 6] {
        let n = (dt / 0.001).ceil().max(1.) as usize;
        let h = dt / n as f64;
        for _ in 0..n {
            let f_del = self.delivered_force(offsets);
            let weight = [0., -self.weight_n, 0.];
            let (mut force, mass) = match self.fixture {
                Fixture::Free => (
                    add(
                        f_del,
                        add(
                            mul(sub(self.hand, self.pos), self.hand_k),
                            mul(self.vel, -self.damping),
                        ),
                    ),
                    self.mass_kg,
                ),
                Fixture::Braced { anchor } => {
                    let spring = mul(sub(anchor, self.pos), 20_000.);
                    self.brace_peak_n = self.brace_peak_n.max(norm(spring));
                    (add(f_del, add(spring, mul(self.vel, -200.))), 0.5)
                }
                Fixture::Stack { hung_n, .. } => (
                    add(f_del, add(weight, mul([0., -hung_n, 0.], 1.))),
                    self.mass_kg,
                ),
                Fixture::Dropped { .. } => (add(f_del, weight), self.mass_kg),
            };
            let speed = norm(self.vel);
            if self.friction_n > 0. && !matches!(self.fixture, Fixture::Braced { .. }) {
                let drag = self.friction_n * (speed / 0.001).tanh();
                if speed > 1e-9 {
                    force = sub(force, mul(self.vel, drag / speed));
                }
            }
            self.vel = add(self.vel, mul(force, h / mass));
            self.pos = add(self.pos, mul(self.vel, h));
            match self.fixture {
                Fixture::Stack { floor_y, .. } | Fixture::Dropped { floor_y }
                    if self.pos[1] <= floor_y && self.vel[1] <= 0. =>
                {
                    // Held by the surface, unless the net vertical force lifts the pen.
                    let lift = f_del[1]
                        - self.weight_n
                        - match self.fixture {
                            Fixture::Stack { hung_n, .. } => hung_n,
                            _ => 0.,
                        };
                    if lift <= self.friction_n {
                        self.pos[1] = floor_y;
                        self.vel = [0.; 3];
                    }
                }
                _ => {}
            }
        }
        self.c = self.inverse(self.pos);
        let q: [f64; 3] =
            std::array::from_fn(|i| (self.origin[i] as f64 + self.q_ref[i] + self.c[i]).round());
        [q[0] as i16, q[1] as i16, q[2] as i16, 0, 0, 0]
    }
    /// Current raw counts without stepping.
    pub fn counts(&self) -> [i16; 6] {
        let q: [f64; 3] =
            std::array::from_fn(|i| (self.origin[i] as f64 + self.q_ref[i] + self.c[i]).round());
        [q[0] as i16, q[1] as i16, q[2] as i16, 0, 0, 0]
    }
}

/// Pack counts into the 16-byte wire packet (word order from the SDK tests).
pub fn packet(counts: [i16; 6], button: bool) -> Sample {
    let mut raw = [0_u8; 16];
    for (i, v) in counts.into_iter().enumerate() {
        let w = [2, 0, 1, 3, 4, 5][i];
        raw[w * 2..w * 2 + 2].copy_from_slice(&v.to_le_bytes());
    }
    raw[13] = if button { 0x1d } else { 0x1f };
    Sample { raw }
}

/// The production encoder and guard over a [`SimPlant`] and a virtual clock.
pub struct SimHw {
    /// The simulated world.
    pub plant: SimPlant,
    state: EncoderState,
    guard: ForceGuard,
    feedback: bool,
    clock: Instant,
    /// Virtual time per sample.
    pub dt: Duration,
    offsets: [i16; 3],
}
impl SimHw {
    /// A fresh simulated device at 2.4 ms per sample.
    pub fn new(plant: SimPlant) -> Self {
        Self {
            plant,
            state: EncoderState::default(),
            guard: ForceGuard::default(),
            feedback: false,
            clock: Instant::now(),
            dt: Duration::from_micros(2400),
            offsets: [0; 3],
        }
    }
}
impl Hardware for SimHw {
    fn now(&self) -> Instant {
        self.clock
    }
    fn sample(&mut self) -> std::io::Result<Sample> {
        self.clock += self.dt;
        let offsets = if self.feedback { self.offsets } else { [0; 3] };
        let counts = self.plant.step(offsets, self.dt.as_secs_f64());
        let s = packet(counts, self.plant.button);
        self.state.update(s);
        self.offsets = if self.feedback {
            self.guard.tick(self.clock, self.state.registered_counts())
        } else {
            [0; 3]
        };
        Ok(s)
    }
    fn state(&self) -> &EncoderState {
        &self.state
    }
    fn guard(&self) -> &ForceGuard {
        &self.guard
    }
    fn register_reset(&mut self) -> Result<Registration, &'static str> {
        self.disable_feedback().map_err(|_| "shutdown failed")?;
        self.state.register_reset().ok_or("no sample yet")
    }
    fn clear_registration(&mut self) {
        let _ = self.disable_feedback();
        self.state.clear_registration();
    }
    fn set_limits(&mut self, l: ForceLimits) -> Result<(), &'static str> {
        self.guard.set_limits(l)
    }
    fn enable_feedback(&mut self) -> std::io::Result<()> {
        if !self.feedback {
            self.guard.arm();
            self.feedback = true;
        }
        Ok(())
    }
    fn disable_feedback(&mut self) -> std::io::Result<()> {
        self.guard.disarm();
        self.feedback = false;
        self.offsets = [0; 3];
        Ok(())
    }
    fn feedback_enabled(&self) -> bool {
        self.feedback
    }
    fn request(&mut self, d: Demand, t: Instant) -> Result<(), RequestError> {
        let verdict = if !self.feedback {
            Err("not armed")
        } else if matches!(d, Demand::Force(_)) && self.state.registration().is_none() {
            Err("unregistered")
        } else {
            self.guard.request(d, t)
        };
        verdict.map_err(|reason| {
            let _ = self.disable_feedback();
            RequestError::Rejected(reason)
        })
    }
}

/// Telemetry accessor used by tests.
#[cfg(test)]
pub fn telemetry(hw: &SimHw) -> visiontouch_device::Telemetry {
    hw.guard.telemetry()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settle(hw: &mut SimHw, n: usize) {
        for _ in 0..n {
            hw.sample().unwrap();
        }
    }

    #[test]
    fn inverse_kinematics_round_trips_the_sdk_model() {
        let mut p = SimPlant::default();
        for target in [[0.02, 0.03, -0.01], [-0.05, 0.01, 0.04], [0., 0., 0.]] {
            p.teleport(target);
            let back = position_from_counts(p.c);
            assert!(norm(sub(back, target)) < 1e-8, "{back:?}");
        }
    }

    #[test]
    fn counts_are_quantised_integers_and_hand_off_pen_floats_still() {
        let mut hw = SimHw::new(SimPlant::default());
        settle(&mut hw, 5);
        let c0 = hw.state().continuous_counts();
        settle(&mut hw, 100);
        assert_eq!(
            hw.state().continuous_counts(),
            c0,
            "a floated pen does not drift"
        );
        hw.register_reset().unwrap();
        assert_eq!(hw.state().registered_counts(), Some([0; 6]));
        assert!(
            hw.state()
                .position_m()
                .unwrap()
                .iter()
                .all(|x| x.abs() < 1e-12)
        );
    }

    #[test]
    fn braced_force_is_gain_times_command() {
        let mut hw = SimHw::new(SimPlant::default());
        settle(&mut hw, 5);
        hw.register_reset().unwrap();
        hw.plant.brace();
        hw.set_limits(ForceLimits {
            force_n: 1.0,
            ..ForceLimits::default()
        })
        .ok();
        hw.enable_feedback().unwrap();
        for _ in 0..400 {
            let now = hw.now();
            hw.request(Demand::Force([0., -0.5, 0.]), now).unwrap();
            hw.sample().unwrap();
        }
        let m = hw.plant.brace_peak_n;
        // 0.5 N commanded, 96-count cap does not bind at the reset pose; gain 0.6.
        assert!((m - 0.3).abs() < 0.02, "{m}");
        assert!(norm(sub(hw.plant.pos, [0.; 3])) < 0.001);
    }

    #[test]
    fn stack_holds_the_pen_until_the_lift_exceeds_the_weight() {
        let mut hw = SimHw::new(SimPlant::default());
        settle(&mut hw, 5);
        hw.register_reset().unwrap();
        hw.plant.fixture = Fixture::Stack {
            floor_y: 0.,
            hung_n: 0.,
        };
        hw.set_limits(ForceLimits {
            force_n: 1.0,
            ..ForceLimits::default()
        })
        .unwrap();
        hw.enable_feedback().unwrap();
        let mut rose_at = None;
        for i in 0..6000 {
            let f = 0.0002 * f64::from(i);
            let now = hw.now();
            hw.request(Demand::Force([0., f.min(1.0), 0.]), now)
                .unwrap();
            hw.sample().unwrap();
            if hw.plant.pos[1] > 0.002 && rose_at.is_none() {
                rose_at = Some(f);
            }
        }
        let f0 = rose_at.expect("lifts");
        // (0.3 weight + 0.04 friction) / 0.6 gain.
        assert!((0.5..=0.7).contains(&f0), "{f0}");
    }

    #[test]
    fn swapped_drive_map_moves_a_different_motor() {
        let mut a = SimPlant::default();
        let mut b = SimPlant {
            drive_map: [2, 1, 0],
            ..SimPlant::default()
        };
        let fa = a.delivered_force([60, 0, 0]);
        let fb = b.delivered_force([60, 0, 0]);
        assert!(norm(sub(fa, fb)) > 0.1);
        a.teleport([0.; 3]);
        b.teleport([0.; 3]);
    }

    #[test]
    fn rejected_requests_shut_output_down() {
        let mut hw = SimHw::new(SimPlant::default());
        settle(&mut hw, 2);
        let now = hw.now();
        assert!(matches!(
            hw.request(Demand::Force([0.; 3]), now),
            Err(RequestError::Rejected("not armed"))
        ));
        hw.enable_feedback().unwrap();
        assert!(matches!(
            hw.request(Demand::Force([0.1, 0., 0.]), now),
            Err(RequestError::Rejected("unregistered"))
        ));
        assert!(!hw.feedback_enabled());
        assert_eq!(telemetry(&hw), visiontouch_device::Telemetry::default());
    }
}
