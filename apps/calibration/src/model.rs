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

/// Slow polyline follower. Arc-length progress prevents a curved route from being
/// replaced by a direct shortcut, including when two branches pass nearby.
pub struct Move {
    route: Vec<V>,
    distances: Vec<f64>,
    progress: f64,
    target_progress: f64,
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
        Self::from_route(vec![start, goal], settings).expect("finite application route")
    }
    pub fn from_route(mut route: Vec<V>, settings: Settings) -> Result<Self, &'static str> {
        if !settings.valid()
            || route.is_empty()
            || route.len() > 25_002
            || route
                .iter()
                .flatten()
                .any(|x| !x.is_finite() || x.abs() > 10.)
        {
            return Err("Invalid return route or settings.");
        }
        route.dedup();
        let mut distances = vec![0.];
        for pair in route.windows(2) {
            distances.push(distances.last().unwrap() + norm(sub(pair[1], pair[0])));
        }
        Ok(Self {
            target: route[0],
            route,
            distances,
            progress: 0.,
            target_progress: 0.,
            demand: [0.; 3],
            elapsed: 0.,
            progress_at: 0.,
            best: 0.,
            settled: 0.,
            settings,
        })
    }
    fn point(&self, along: f64) -> V {
        let i = self
            .distances
            .partition_point(|d| *d <= along)
            .saturating_sub(1);
        if i + 1 >= self.route.len() {
            return *self.route.last().unwrap();
        }
        let fraction = (along - self.distances[i]) / (self.distances[i + 1] - self.distances[i]);
        add(
            self.route[i],
            mul(sub(self.route[i + 1], self.route[i]), fraction),
        )
    }
    pub fn tick(&mut self, position: V, velocity: V, dt: f64) -> Result<V, &'static str> {
        if !dt.is_finite()
            || dt <= 0.
            || dt > 0.025
            || position.iter().chain(&velocity).any(|x| !x.is_finite())
        {
            return Err("Invalid or delayed move input; output stopped.");
        }
        self.elapsed += dt;
        let length = *self.distances.last().unwrap();
        let lo = (self.progress - 0.003).max(0.);
        let hi = (self.progress + 0.003).min(length);
        let mut nearest_distance = norm(sub(position, self.point(lo)));
        let mut along = lo;
        let first = self
            .distances
            .partition_point(|d| *d < lo)
            .saturating_sub(1);
        for i in first..self.route.len().saturating_sub(1) {
            if self.distances[i] > hi {
                break;
            }
            let segment = sub(self.route[i + 1], self.route[i]);
            let segment_length = self.distances[i + 1] - self.distances[i];
            let projected = (self.distances[i]
                + dot(sub(position, self.route[i]), segment) / segment_length)
                .clamp(lo.max(self.distances[i]), hi.min(self.distances[i + 1]));
            let distance = norm(sub(position, self.point(projected)));
            if distance < nearest_distance {
                nearest_distance = distance;
                along = projected;
            }
        }
        if nearest_distance > 0.003 {
            return Err("Reverse or sideways travel exceeded 3 model mm; output stopped.");
        }
        if self.best - along > 0.003 {
            return Err("Reverse travel exceeded 3 model mm; output stopped.");
        }
        self.progress = along;
        if along > self.best + 0.00025 {
            self.best = along;
            self.progress_at = self.elapsed;
        }
        if self.elapsed > 180. {
            return Err("Attempt timed out; output stopped.");
        }
        if self.elapsed - self.progress_at > 5. {
            return Err("Resistance / stalled motion; output stopped at the selected force cap.");
        }
        self.target_progress = (self.target_progress + self.settings.speed_mm_s * 0.001 * dt)
            .min(along + 0.002)
            .min(length);
        self.target = self.point(self.target_progress);
        let distance = norm(sub(*self.route.last().unwrap(), position));
        if self.target_progress >= length && distance < 0.00075 && norm(velocity) < 0.002 {
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
    fn bent_route_targets_stay_on_recorded_legs_and_finish() {
        let route = vec![[0.; 3], [0.01, 0., 0.], [0.01, 0.01, 0.]];
        let mut m = Move::from_route(route, Settings::default()).unwrap();
        let mut p = [0.; 3];
        let mut v = [0.; 3];
        let mut reached = false;
        for _ in 0..30000 {
            match m.tick(p, v, 0.001) {
                Ok(f) => {
                    assert!(norm(f) <= 0.1 + 1e-12);
                    assert!(m.target[1].abs() < 1e-12 || (m.target[0] - 0.01).abs() < 1e-12);
                    assert!(m.target_progress - m.progress <= 0.002 + 1e-12);
                    v = add(v, mul(f, 0.001 / 0.05));
                    p = add(p, mul(v, 0.001));
                }
                Err(reason) => {
                    assert!(reason.contains("Zero reached"), "{reason}: {p:?}");
                    reached = true;
                    break;
                }
            }
        }
        assert!(reached);
    }
    #[test]
    fn nearby_loop_endpoint_cannot_skip_route_or_settle_early() {
        let mut m = Move::from_route(
            vec![
                [0.; 3],
                [0.02, 0., 0.],
                [0.02, 0.02, 0.],
                [0., 0.02, 0.],
                [0., 0.0005, 0.],
            ],
            Settings::default(),
        )
        .unwrap();
        for _ in 0..1000 {
            m.tick([0., 0.0005, 0.], [0.; 3], 0.001).unwrap();
            assert!(m.target[1].abs() < 1e-12);
            assert!(m.target_progress <= 0.002 + 1e-12);
        }
        assert_eq!(m.settled, 0.);
        assert!(m.tick([0.01, 0.01, 0.], [0.; 3], 0.001).is_err());
    }
    #[test]
    fn invalid_route_and_powered_timing_fail_closed() {
        assert!(Move::from_route(vec![], Settings::default()).is_err());
        assert!(Move::from_route(vec![[f64::NAN; 3]], Settings::default()).is_err());
        let mut m = Move::new([0.; 3], [0.01, 0., 0.], Settings::default());
        assert!(m.tick([0.; 3], [0.; 3], 0.05).is_err());
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
