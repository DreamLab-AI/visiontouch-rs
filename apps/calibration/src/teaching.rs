//! Motor-off freehand route recording. Time-ordered observations are reversed for
//! the return move; a straight shortcut through an unobserved interior is never used.
use crate::model::*;
use serde::Serialize;

pub const MAX_AGE: std::time::Duration = std::time::Duration::from_secs(300);
pub const PILOT_FORCE_N: f64 = 0.1;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Recording,
    Complete,
}
impl Stage {
    pub fn instruction(self) -> &'static str {
        match self {
            Self::Recording => {
                "Motors OFF: move freely to your starting pose; hold still and click Finish route."
            }
            Self::Complete => "Return route recorded. Held pilot tests available for five minutes.",
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Evidence {
    pub method: &'static str,
    pub start_model_m: V,
    pub zero_model_m: V,
    /// Motor-off observations [elapsed seconds, model x, model y, model z].
    pub observations: Vec<[f64; 4]>,
    /// Ordered powered return path, from the finishing pose back to zero.
    pub return_route_model_m: Vec<V>,
    pub elapsed_s: f64,
    pub support_radius_m: f64,
    pub evidence_margin_m: f64,
}
/// Live-only recorder. No deserialization or saved-evidence authorization.
pub struct Teaching {
    pub stage: Stage,
    zero: V,
    last: V,
    elapsed: f64,
    observations: Vec<[f64; 4]>,
    failed: bool,
}
impl Teaching {
    pub fn new(zero: V) -> Result<Self, &'static str> {
        if zero.iter().any(|x| !x.is_finite() || x.abs() > 10.) {
            return Err("Invalid route origin.");
        }
        Ok(Self {
            stage: Stage::Recording,
            zero,
            last: zero,
            elapsed: 0.,
            observations: vec![[0., zero[0], zero[1], zero[2]]],
            failed: false,
        })
    }
    /// Validate every live sample even when its point is not retained.
    pub fn sample(&mut self, p: V, dt: f64) -> Result<Stage, &'static str> {
        if self.failed {
            return Err("Route recording invalidated; start again.");
        }
        let result = self.step(p, dt);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn step(&mut self, p: V, dt: f64) -> Result<Stage, &'static str> {
        if self.stage == Stage::Complete {
            return Ok(self.stage);
        }
        if !dt.is_finite()
            || dt <= 0.
            || dt > 0.1
            || p.iter().any(|x| !x.is_finite() || x.abs() > 10.)
        {
            return Err("Route input invalid or delayed over 100 ms; record again.");
        }
        self.elapsed += dt;
        if self.elapsed > 180. {
            return Err("Route recording timed out; record again.");
        }
        let travel = norm(sub(p, self.last));
        if travel > 0.003 || travel / dt > 0.06 {
            return Err(
                "Route input jumped or exceeded 60 model mm/s; move more slowly and record again.",
            );
        }
        self.last = p;
        let r = self.observations.last().unwrap();
        if norm(sub(p, [r[1], r[2], r[3]])) >= 0.0005 {
            if self.observations.len() >= 25_000 {
                return Err("Route sample limit reached; record again.");
            }
            self.observations.push([self.elapsed, p[0], p[1], p[2]]);
        }
        Ok(self.stage)
    }
    /// Called after a fresh sample. A premature Finish leaves recording active.
    pub fn finish(&mut self, still: bool) -> Result<(), &'static str> {
        if self.failed {
            return Err("Route recording invalidated; start again.");
        }
        if !still {
            return Err("Hold still, then click Finish route again.");
        }
        if !(0.005..=0.5).contains(&norm(sub(self.last, self.zero))) {
            return Err("Finish at a pose 5–500 model mm from zero.");
        }
        self.observations
            .push([self.elapsed, self.last[0], self.last[1], self.last[2]]);
        self.stage = Stage::Complete;
        Ok(())
    }
    pub fn evidence(&self) -> Option<Evidence> {
        if self.failed || self.stage != Stage::Complete {
            return None;
        }
        let policy = haptics_workspace::Policy::default();
        let mut route: Vec<V> = self
            .observations
            .iter()
            .rev()
            .map(|r| [r[1], r[2], r[3]])
            .collect();
        route.dedup();
        Some(Evidence {
            method: "freehand_reverse_v1",
            start_model_m: self.last,
            zero_model_m: self.zero,
            observations: self.observations.clone(),
            return_route_model_m: route,
            elapsed_s: self.elapsed,
            support_radius_m: policy.support_radius_m,
            evidence_margin_m: policy.margin_m,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn records_bent_route_and_reverses_without_a_return_leg() {
        let mut t = Teaching::new([0.; 3]).unwrap();
        for i in 1..=100 {
            t.sample([i as f64 * 0.0002, 0., 0.], 0.01).unwrap();
        }
        for i in 1..=100 {
            t.sample([0.02, i as f64 * 0.0002, 0.], 0.01).unwrap();
        }
        assert!(t.evidence().is_none());
        assert!(t.finish(false).is_err());
        t.finish(true).unwrap();
        let e = t.evidence().unwrap();
        assert_eq!(e.return_route_model_m.first(), Some(&[0.02, 0.02, 0.]));
        assert_eq!(e.return_route_model_m.last(), Some(&[0.; 3]));
        let c = haptics_workspace::EvidenceCloud::new(
            e.return_route_model_m.clone(),
            Default::default(),
        )
        .unwrap();
        for pair in e.return_route_model_m.windows(2) {
            assert_eq!(
                c.review_segment(pair[0], pair[1]).unwrap(),
                haptics_workspace::SegmentReview::Supported
            );
        }
        assert_ne!(
            c.review_segment(e.start_model_m, e.zero_model_m).unwrap(),
            haptics_workspace::SegmentReview::Supported
        );
    }
    #[test]
    fn input_only_jitter_is_tolerated_but_gaps_jumps_and_invalid_data_latch() {
        let mut t = Teaching::new([0.; 3]).unwrap();
        assert!(t.sample([0.001, 0., 0.], 0.05).is_ok());
        for (p, dt) in [
            ([0.; 3], 0.101),
            ([0.004, 0., 0.], 0.1),
            ([f64::NAN; 3], 0.01),
            ([0.001, 0., 0.], 0.001),
        ] {
            let mut t = Teaching::new([0.; 3]).unwrap();
            assert!(t.sample(p, dt).is_err());
            assert!(t.sample([0.; 3], 0.01).is_err());
            assert!(t.finish(true).is_err());
            assert!(t.evidence().is_none());
        }
    }
    #[test]
    fn finish_requires_distance_and_stillness() {
        let mut t = Teaching::new([0.; 3]).unwrap();
        assert!(t.finish(true).is_err());
        for i in 1..=50 {
            t.sample([0., i as f64 * 0.0002, 0.], 0.01).unwrap();
        }
        assert!(t.finish(false).is_err());
        assert!(t.finish(true).is_ok());
        assert!(Teaching::new([f64::INFINITY; 3]).is_err());
    }
}
