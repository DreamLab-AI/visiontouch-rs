use serde::{Deserialize, Serialize};

pub use haptics_effects::math::*;

pub const NAMES: [&str; 6] = [
    "Left · X min",
    "Right · X max",
    "Bottom · Y min",
    "Top · Y max",
    "Back · Z min",
    "Front · Z max",
];
pub const INSTRUCTIONS: [&str; 6] = [
    "Move the pen to its comfortable LEFT limit. Keep height and depth centred. Record left.",
    "Move the pen to its comfortable RIGHT limit. Keep height and depth centred. Record right.",
    "Rest the pen at the BOTTOM of travel, centred left/right and front/back. Capture bottom reference first.",
    "Lift the pen to its comfortable TOP limit. Keep left/right and depth centred. Record top.",
    "Move the pen AWAY from you to its comfortable BACK limit. Keep height and left/right centred. Record back.",
    "Move the pen TOWARD you to its comfortable FRONT limit. Keep height and left/right centred. Record front.",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Volume {
    pub bounds: [[f64; 2]; 3],
    pub measured: [Option<f64>; 6], // model metres relative to bottom reference
}
impl Default for Volume {
    fn default() -> Self {
        Self {
            bounds: [[-100., 100.]; 3],
            measured: [None; 6],
        }
    }
}
impl Volume {
    pub fn validate(&self) -> Result<(), &'static str> {
        for axis in 0..3 {
            let [lo, hi] = self.bounds[axis];
            if !lo.is_finite() || !hi.is_finite() || lo >= 0. || hi <= 0. {
                return Err("Each entered minimum must be below zero and maximum above zero.");
            }
            let (Some(a), Some(b)) = (self.measured[2 * axis], self.measured[2 * axis + 1]) else {
                return Err("Record all six physical bounds before enabling motor output.");
            };
            if !a.is_finite() || !b.is_finite() || b - a < 0.002 {
                return Err(
                    "Recorded maximum must exceed minimum by at least 2 model mm. Check direction and recapture.",
                );
            }
        }
        Ok(())
    }
    pub fn working(&self, model: V) -> Option<V> {
        self.validate().ok()?;
        Some(std::array::from_fn(|i| {
            let a = self.measured[2 * i].unwrap();
            let b = self.measured[2 * i + 1].unwrap();
            self.bounds[i][0] + (model[i] - a) / (b - a) * (self.bounds[i][1] - self.bounds[i][0])
        }))
    }
    pub fn zero(&self) -> Option<V> {
        self.validate().ok()?;
        Some(std::array::from_fn(|i| {
            let a = self.measured[2 * i].unwrap();
            let b = self.measured[2 * i + 1].unwrap();
            a + (-self.bounds[i][0]) / (self.bounds[i][1] - self.bounds[i][0]) * (b - a)
        }))
    }
    pub fn contains(&self, position: V, margin: f64) -> bool {
        self.validate().is_ok()
            && (0..3).all(|i| {
                position[i].is_finite()
                    && position[i] >= self.measured[2 * i].unwrap() - margin
                    && position[i] <= self.measured[2 * i + 1].unwrap() + margin
            })
    }
    pub fn radius(&self) -> f64 {
        let Some(zero) = self.zero() else {
            return 0.02;
        };
        (0..3)
            .map(|i| {
                (zero[i] - self.measured[2 * i].unwrap())
                    .min(self.measured[2 * i + 1].unwrap() - zero[i])
            })
            .fold(f64::INFINITY, f64::min)
            * 0.45
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Settings {
    pub speed_mm_s: f64,
    pub force_n: f64,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            speed_mm_s: 2.,
            force_n: 0.1,
        }
    }
}
impl Settings {
    pub fn valid(self) -> bool {
        self.speed_mm_s.is_finite()
            && (0.5..=5.).contains(&self.speed_mm_s)
            && self.force_n.is_finite()
            && (0.02..=haptics_device::MAX_FORCE_N).contains(&self.force_n)
    }
}

pub struct Move {
    start: V,
    goal: V,
    pub target: V,
    pub demand: V,
    elapsed: f64,
    progress_at: f64,
    best: f64,
    settled: f64,
    settings: Settings,
}
impl Move {
    pub fn new(start: V, goal: V, settings: Settings) -> Self {
        Self {
            start,
            goal,
            target: start,
            demand: [0.; 3],
            elapsed: 0.,
            progress_at: 0.,
            best: norm(sub(goal, start)),
            settled: 0.,
            settings,
        }
    }
    pub fn tick(&mut self, position: V, velocity: V, dt: f64) -> Result<V, &'static str> {
        self.elapsed += dt;
        let distance = norm(sub(self.goal, position));
        if distance < self.best - 0.00025 {
            self.best = distance;
            self.progress_at = self.elapsed;
        }
        if self.elapsed > 180. {
            return Err("Attempt timed out; output stopped.");
        }
        if self.elapsed - self.progress_at > 5. {
            return Err("Resistance / stalled motion; output stopped at the selected force cap.");
        }
        let line = sub(self.goal, self.start);
        let length = norm(line);
        let direction = if length > 1e-9 {
            mul(line, 1. / length)
        } else {
            [0.; 3]
        };
        let along = dot(sub(position, self.start), direction).clamp(0., length);
        let nearest = add(self.start, mul(direction, along));
        if norm(sub(position, nearest)) > 0.003 {
            return Err("Reverse or sideways travel exceeded 3 model mm; output stopped.");
        }
        let target_along = (dot(sub(self.target, self.start), direction)
            + self.settings.speed_mm_s * 0.001 * dt)
            .min(along + 0.002)
            .min(length);
        self.target = add(self.start, mul(direction, target_along));
        if distance < 0.00075 && norm(velocity) < 0.002 {
            self.settled += dt;
        } else {
            self.settled = 0.;
        }
        if self.settled >= 0.5 {
            return Err("Zero reached and settled; output stopped.");
        }
        self.demand = sub(mul(sub(self.target, position), 150.), mul(velocity, 4.));
        Ok(limit(self.demand, self.settings.force_n))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub fn volume() -> Volume {
        Volume {
            bounds: [[-100., 100.]; 3],
            measured: [
                Some(-0.05),
                Some(0.05),
                Some(0.),
                Some(0.1),
                Some(-0.05),
                Some(0.05),
            ],
        }
    }
    #[test]
    fn mapping_zero_and_unclamped_bounds() {
        let v = volume();
        assert_eq!(v.zero(), Some([0., 0.05, 0.]));
        assert_eq!(v.working([0., 0., 0.]), Some([0., -100., 0.]));
        assert!(norm(sub(v.working([0.1, 0.05, 0.]).unwrap(), [200., 0., 0.])) < 1e-10);
        assert!(!v.contains([0.1, 0., 0.], 0.));
        let mut invalid = v;
        invalid.bounds[1] = [1., 2.];
        assert!(invalid.validate().is_err());
        invalid.bounds[1] = [-1., 2.];
        invalid.measured[0] = None;
        assert!(invalid.zero().is_none());
    }
    #[test]
    fn sustained_resistance_stops_and_target_lead_is_bounded() {
        let mut m = Move::new([0.; 3], [0., 0.05, 0.], Settings::default());
        let mut stopped = false;
        for _ in 0..2600 {
            match m.tick([0.; 3], [0.; 3], 0.002) {
                Ok(f) => {
                    assert!(norm(f) <= 0.1 + 1e-12);
                    assert!(norm(m.target) <= 0.002 + 1e-12);
                }
                Err(reason) => {
                    assert!(reason.contains("Resistance"));
                    stopped = true;
                    break;
                }
            }
        }
        assert!(stopped);
    }
    #[test]
    fn free_move_reaches_zero_and_reverse_motion_stops() {
        let mut m = Move::new([0.; 3], [0., 0.004, 0.], Settings::default());
        let mut p = [0.; 3];
        let mut v = [0.; 3];
        let mut reached = false;
        for _ in 0..10000 {
            match m.tick(p, v, 0.001) {
                Ok(f) => {
                    v = add(v, mul(f, 0.001 / 0.05));
                    p = add(p, mul(v, 0.001));
                }
                Err(reason) => {
                    assert!(reason.contains("Zero reached"), "{reason}");
                    reached = true;
                    break;
                }
            }
        }
        assert!(reached);
        let mut m = Move::new([0.; 3], [0., 0.02, 0.], Settings::default());
        assert!(m.tick([0., -0.004, 0.], [0.; 3], 0.002).is_err());
    }
}
