//! Motor-off approach teaching. All input samples are checked; retained points
//! are only a spatially reduced evidence record. Nothing here arms a device.
use crate::model::*;
use serde::Serialize;

pub const MAX_AGE: std::time::Duration = std::time::Duration::from_secs(300);
pub const PILOT_FORCE_N: f64 = 0.1;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    ToZero,
    Return,
    Complete,
}
impl Stage {
    pub fn instruction(self) -> &'static str {
        match self {
            Self::ToZero => "Motors OFF: follow the gold line to zero, then hold still.",
            Self::Return => {
                "Motors OFF: return along the gold line to the starting pose, then hold still."
            }
            Self::Complete => {
                "Manual route recorded. Held pilot tests are available for five minutes."
            }
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Evidence {
    pub start_model_m: V,
    pub zero_model_m: V,
    /// Each row is [elapsed seconds, model x, model y, model z].
    pub observations: Vec<[f64; 4]>,
    pub elapsed_s: f64,
    pub support_radius_m: f64,
    pub evidence_margin_m: f64,
}
/// Freshly constructed from a still live pose. Serialization is deliberately absent.
pub struct Teaching {
    pub start: V,
    pub zero: V,
    pub stage: Stage,
    last: V,
    elapsed: f64,
    dwell: f64,
    observations: Vec<[f64; 4]>,
    failed: bool,
}
impl Teaching {
    pub fn new(start: V, zero: V) -> Result<Self, &'static str> {
        let length = norm(sub(zero, start));
        if start
            .iter()
            .chain(&zero)
            .any(|x| !x.is_finite() || x.abs() > 10.)
            || !(0.005..=0.5).contains(&length)
        {
            return Err("Choose a still starting pose 5–500 model mm from zero.");
        }
        Ok(Self {
            start,
            zero,
            stage: Stage::ToZero,
            last: start,
            elapsed: 0.,
            dwell: 0.,
            observations: vec![[0., start[0], start[1], start[2]]],
            failed: false,
        })
    }
    /// `still` must come from a fresh, bounded history of live input positions.
    /// Errors permanently invalidate this trace; do not resume it after a gap.
    pub fn sample(&mut self, p: V, dt: f64, still: bool) -> Result<Stage, &'static str> {
        if self.failed {
            return Err("Teaching trace invalidated; start again.");
        }
        if self.stage == Stage::Complete {
            return Ok(Stage::Complete);
        }
        let result = self.step(p, dt, still);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn step(&mut self, p: V, dt: f64, still: bool) -> Result<Stage, &'static str> {
        if !dt.is_finite()
            || dt <= 0.
            || dt > 0.025
            || p.iter().any(|x| !x.is_finite() || x.abs() > 10.)
        {
            return Err("Teaching input invalid or delayed; start again.");
        }
        self.elapsed += dt;
        if self.elapsed > 180. {
            return Err("Teaching timed out; start again.");
        }
        if norm(sub(p, self.last)) / dt > 0.06 {
            return Err("Move more slowly while teaching (below 60 model mm/s).");
        }
        let line = sub(self.zero, self.start);
        let length2 = dot(line, line);
        let fraction = (dot(sub(p, self.start), line) / length2).clamp(0., 1.);
        if norm(sub(p, add(self.start, mul(line, fraction)))) > 0.002 {
            return Err(
                "Teaching left the 2 mm route corridor; choose a reachable straight path and retry.",
            );
        }
        self.last = p;
        let last = self.observations.last().unwrap();
        if norm(sub(p, [last[1], last[2], last[3]])) >= 0.0005 {
            if self.observations.len() >= 25_000 {
                return Err("Teaching evidence limit reached; start again.");
            }
            self.observations.push([self.elapsed, p[0], p[1], p[2]]);
        }
        let target = if self.stage == Stage::ToZero {
            self.zero
        } else {
            self.start
        };
        if norm(sub(p, target)) <= 0.00075 && still {
            self.dwell += dt;
        } else {
            self.dwell = 0.;
        }
        if self.dwell >= 0.3 {
            self.dwell = 0.;
            self.stage = if self.stage == Stage::ToZero {
                Stage::Return
            } else {
                Stage::Complete
            };
            self.observations.push([self.elapsed, p[0], p[1], p[2]]);
        }
        Ok(self.stage)
    }
    pub fn evidence(&self) -> Option<Evidence> {
        if self.failed || self.stage != Stage::Complete {
            return None;
        }
        let policy = haptics_workspace::Policy::default();
        Some(Evidence {
            start_model_m: self.start,
            zero_model_m: self.zero,
            observations: self.observations.clone(),
            elapsed_s: self.elapsed,
            support_radius_m: policy.support_radius_m,
            evidence_margin_m: policy.margin_m,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn dwell(t: &mut Teaching, p: V) {
        for _ in 0..40 {
            t.sample(p, 0.01, true).unwrap();
        }
    }
    #[test]
    fn requires_continuous_out_and_back_with_both_dwells() {
        let mut t = Teaching::new([0.; 3], [0., 0.02, 0.]).unwrap();
        for i in 1..=100 {
            t.sample([0., i as f64 * 0.0002, 0.], 0.01, false).unwrap();
        }
        assert!(t.evidence().is_none());
        dwell(&mut t, [0., 0.02, 0.]);
        assert_eq!(t.stage, Stage::Return);
        for i in (0..100).rev() {
            t.sample([0., i as f64 * 0.0002, 0.], 0.01, false).unwrap();
        }
        assert!(t.evidence().is_none());
        dwell(&mut t, [0.; 3]);
        assert_eq!(t.stage, Stage::Complete);
        let evidence = t.evidence().unwrap();
        assert!(evidence.observations.len() > 40);
        let points = evidence
            .observations
            .iter()
            .map(|r| [r[1], r[2], r[3]])
            .collect();
        let c = haptics_workspace::EvidenceCloud::new(points, Default::default()).unwrap();
        assert_eq!(
            c.review_segment(t.start, t.zero).unwrap(),
            haptics_workspace::SegmentReview::Supported
        );
    }
    #[test]
    fn delay_jump_and_sideways_motion_invalidate_instead_of_resuming() {
        for (p, dt) in [
            ([0.; 3], 0.03),
            ([0., 0.01, 0.], 0.01),
            ([f64::NAN, 0., 0.], 0.01),
        ] {
            let mut t = Teaching::new([0.; 3], [0., 0.02, 0.]).unwrap();
            assert!(t.sample(p, dt, false).is_err());
            assert!(t.sample([0.; 3], 0.01, true).is_err());
            assert!(t.evidence().is_none());
        }
        let mut t = Teaching::new([0.; 3], [0., 0.02, 0.]).unwrap();
        for i in 1..=10 {
            t.sample([i as f64 * 0.0002, 0., 0.], 0.01, false).unwrap();
        }
        assert!(t.sample([0.0022, 0., 0.], 0.01, false).is_err());
    }
    #[test]
    fn invalid_or_tiny_route_and_missing_stillness_cannot_confirm() {
        assert!(Teaching::new([0.; 3], [0.; 3]).is_err());
        assert!(Teaching::new([0.; 3], [f64::INFINITY, 0., 0.]).is_err());
        let mut t = Teaching::new([0.; 3], [0., 0.02, 0.]).unwrap();
        for i in 1..=100 {
            t.sample([0., i as f64 * 0.0002, 0.], 0.01, false).unwrap();
        }
        for _ in 0..100 {
            t.sample([0., 0.02, 0.], 0.01, false).unwrap();
        }
        assert_eq!(t.stage, Stage::ToZero);
        assert!(t.evidence().is_none());
    }
}
