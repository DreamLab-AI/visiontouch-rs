//! Motor-off spatial observations. Empty space between samples is unverified.
use crate::model::V;
use serde::Serialize;
use std::collections::HashSet;

pub const CELL_M: f64 = 0.002;
pub const MAX_POINTS: usize = 20_000;

#[derive(Clone, Debug, Default, Serialize)]
pub struct Sweep {
    pub points_model_m: Vec<V>,
    pub zero_model_m: Option<V>,
    pub path_review: Option<PathReview>,
    #[serde(skip)]
    cells: HashSet<[i32; 3]>,
}
/// Review evidence only. This does not grant output authority or physical safety.
#[derive(Clone, Debug, Serialize)]
pub struct PathReview {
    pub start_model_m: V,
    pub zero_model_m: V,
    pub sample_count: usize,
    pub support_radius_m: f64,
    pub evidence_margin_m: f64,
    pub first_gap_fraction: Option<[f64; 2]>,
}
impl Sweep {
    /// Retain one measured point per 2 mm cell; never manufacture interior points.
    pub fn record(&mut self, p: V) -> bool {
        if p.iter().any(|x| !x.is_finite() || x.abs() > 10.)
            || self.points_model_m.len() >= MAX_POINTS
        {
            return false;
        }
        let cell = p.map(|x| (x / CELL_M).floor() as i32);
        if self.cells.insert(cell) {
            self.points_model_m.push(p);
            self.path_review = None;
            true
        } else {
            false
        }
    }
    pub fn extrema(&self) -> Option<[Option<f64>; 6]> {
        let first = self.points_model_m.first()?;
        let mut edges = [
            Some(first[0]),
            Some(first[0]),
            Some(first[1]),
            Some(first[1]),
            Some(first[2]),
            Some(first[2]),
        ];
        for p in &self.points_model_m {
            for i in 0..3 {
                edges[2 * i] = Some(edges[2 * i].unwrap().min(p[i]));
                edges[2 * i + 1] = Some(edges[2 * i + 1].unwrap().max(p[i]));
            }
        }
        Some(edges)
    }
    pub fn display_bounds(&self) -> Option<[[f64; 2]; 3]> {
        let e = self.extrema()?;
        let centre = self.zero_model_m.unwrap_or_else(|| {
            std::array::from_fn(|i| (e[2 * i].unwrap() + e[2 * i + 1].unwrap()) * 0.5)
        });
        Some(std::array::from_fn(|i| {
            [
                (e[2 * i].unwrap() - centre[i]) * 1000.,
                (e[2 * i + 1].unwrap() - centre[i]) * 1000.,
            ]
        }))
    }
    /// Only accepts an interior zero; this is a measured pose, not automatic homing.
    pub fn choose_zero(&mut self, p: V) -> Result<(), &'static str> {
        let e = self.extrema().ok_or("Record a sweep first.")?;
        if p.iter().any(|x| !x.is_finite())
            || !(0..3)
                .all(|i| p[i] > e[2 * i].unwrap() + 0.002 && p[i] < e[2 * i + 1].unwrap() - 0.002)
        {
            return Err("Choose a still pose at least 2 model mm inside every observed extremity.");
        }
        self.record(p);
        self.zero_model_m = Some(p);
        self.path_review = None;
        Ok(())
    }
    /// Review a still current pose against the manually observed zero. Never arm.
    pub fn review_path(&mut self, start: V) -> Result<(), String> {
        self.path_review = None;
        let zero = self.zero_model_m.ok_or("Record a working zero first.")?;
        let policy = haptics_workspace::Policy::default();
        let mut observations = self.points_model_m.clone();
        // Both endpoints were explicitly observed while still, even if no sweep
        // cell was retained there (for example after reaching the sample cap).
        observations.extend([start, zero]);
        let cloud = haptics_workspace::EvidenceCloud::new(observations, policy)
            .map_err(|e| e.to_string())?;
        let gap = match cloud
            .review_segment(start, zero)
            .map_err(|e| e.to_string())?
        {
            haptics_workspace::SegmentReview::Supported => None,
            haptics_workspace::SegmentReview::Gap { from, to } => Some([from, to]),
        };
        self.path_review = Some(PathReview {
            start_model_m: start,
            zero_model_m: zero,
            sample_count: cloud.points().len(),
            support_radius_m: policy.support_radius_m,
            evidence_margin_m: policy.margin_m,
            first_gap_fraction: gap,
        });
        Ok(())
    }
    pub fn dimensions_mm(&self) -> V {
        self.extrema()
            .map(|e| std::array::from_fn(|i| (e[2 * i + 1].unwrap() - e[2 * i].unwrap()) * 1000.))
            .unwrap_or([0.; 3])
    }
    #[cfg(test)]
    pub fn near_sample(&self, p: V) -> bool {
        self.points_model_m
            .iter()
            .any(|q| crate::model::norm(crate::model::sub(*q, p)) < CELL_M)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retains_non_box_geometry_and_rejects_invalid_samples() {
        let mut s = Sweep::default();
        for p in [
            [-0.02, 0., 0.],
            [0.02, 0., 0.],
            [0., -0.02, 0.],
            [0., 0.02, 0.],
            [0., 0., -0.02],
            [0., 0., 0.02],
        ] {
            assert!(s.record(p));
        }
        assert!(!s.record([0.0201, 0., 0.]));
        assert!(!s.record([f64::NAN, 0., 0.]));
        assert_eq!(s.dimensions_mm(), [40.; 3]);
        assert!(!s.near_sample([0.02; 3])); // enclosing-box corner is not observed
        assert!(s.choose_zero([0.; 3]).is_ok());
        assert!(s.choose_zero([0.019, 0., 0.]).is_err());
        assert_eq!(s.display_bounds(), Some([[-20., 20.]; 3]));
    }
    #[test]
    fn changing_evidence_or_zero_invalidates_path_review() {
        let mut s = Sweep::default();
        for axis in 0..3 {
            for sign in [-1., 1.] {
                let mut p = [0.; 3];
                p[axis] = sign * 0.02;
                s.record(p);
            }
        }
        s.choose_zero([0.; 3]).unwrap();
        s.review_path([-0.02, 0., 0.]).unwrap();
        assert!(s.path_review.as_ref().unwrap().first_gap_fraction.is_some());
        assert!(s.record([-0.01, 0., 0.]));
        assert!(s.path_review.is_none());
        s.review_path([0.; 3]).unwrap();
        assert!(s.path_review.as_ref().unwrap().first_gap_fraction.is_none());
        s.choose_zero([0.001, 0., 0.]).unwrap();
        assert!(s.path_review.is_none());
        assert!(s.review_path([f64::NAN; 3]).is_err());
        assert!(s.path_review.is_none());
    }
    #[test]
    fn storage_is_bounded_and_empty_scan_has_no_volume() {
        let mut s = Sweep::default();
        assert!(s.extrema().is_none());
        assert!(s.choose_zero([0.; 3]).is_err());
        for i in 0..MAX_POINTS + 1 {
            s.record([(i % 200) as f64 * 0.003, (i / 200) as f64 * 0.003, 0.]);
        }
        assert_eq!(s.points_model_m.len(), MAX_POINTS);
    }
}
