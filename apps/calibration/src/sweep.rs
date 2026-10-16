//! Motor-off spatial observations. Empty space between samples is unverified.
use crate::model::V;
use serde::Serialize;
use std::collections::HashSet;

pub const CELL_M: f64 = 0.002;
pub const MAX_POINTS: usize = 20_000;

#[derive(Clone, Debug, Default, Serialize)]
pub struct Sweep {
    pub points_model_m: Vec<V>,
    /// Exact observed axis limits [xmin, xmax, ymin, ymax, zmin, zmax].
    /// Unlike the display cloud, these include observations within an existing cell.
    /// They are not additional support points or a reachable-volume certificate.
    pub extrema_model_m: Option<[f64; 6]>,
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
    /// Update exact extrema even if no point is retained. The return value reports
    /// cloud insertion only, not whether the observation changed an extremum.
    pub fn record(&mut self, p: V) -> bool {
        if p.iter().any(|x| !x.is_finite() || x.abs() > 10.) {
            return false;
        }
        let old = self.extrema_model_m;
        let edges = self
            .extrema_model_m
            .get_or_insert([p[0], p[0], p[1], p[1], p[2], p[2]]);
        for i in 0..3 {
            edges[2 * i] = edges[2 * i].min(p[i]);
            edges[2 * i + 1] = edges[2 * i + 1].max(p[i]);
        }
        if old != self.extrema_model_m {
            self.path_review = None;
        }
        if self.points_model_m.len() >= MAX_POINTS {
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
        self.extrema_model_m.map(|edges| edges.map(Some))
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
        assert!(!s.record([0.02, 0., 0.]));
        assert!(!s.record([f64::NAN, 0., 0.]));
        assert_eq!(s.dimensions_mm(), [40.; 3]);
        assert!(!s.near_sample([0.02; 3])); // enclosing-box corner is not observed
        assert!(s.choose_zero([0.; 3]).is_ok());
        assert!(s.choose_zero([0.019, 0., 0.]).is_err());
        assert_eq!(s.display_bounds(), Some([[-20., 20.]; 3]));
    }
    #[test]
    fn display_thinning_preserves_exact_extrema_without_filling_support() {
        let mut s = Sweep::default();
        assert!(s.record([0.0201, -0.0201, 0.]));
        // Same cells, but a further extreme on both positive and negative axes.
        assert!(!s.record([0.0219, -0.0219, 0.0019]));
        let expected = [0.0201, 0.0219, -0.0219, -0.0201, 0., 0.0019];
        assert_eq!(s.extrema_model_m, Some(expected));
        assert_eq!(s.extrema(), Some(expected.map(Some)));
        assert_eq!(s.points_model_m, vec![[0.0201, -0.0201, 0.]]);
        let exported = serde_json::to_value(&s).unwrap();
        assert_eq!(exported["extrema_model_m"], serde_json::json!(expected));
        for bad in [[f64::NAN; 3], [f64::INFINITY; 3], [10.1; 3]] {
            assert!(!s.record(bad));
            assert_eq!(s.extrema_model_m, Some(expected));
        }
    }
    #[test]
    fn changed_extremum_invalidates_review_even_without_new_cloud_point() {
        let mut s = Sweep::default();
        s.record([0.; 3]);
        s.zero_model_m = Some([0.; 3]);
        s.review_path([0.; 3]).unwrap();
        assert!(s.path_review.is_some());
        assert!(!s.record([0.001; 3]));
        assert!(s.path_review.is_none());
        assert_eq!(s.points_model_m.len(), 1);
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
        assert!(!s.record([1.; 3]));
        assert_eq!(s.extrema_model_m, Some([0., 1., 0., 1., 0., 1.]));
        assert_eq!(s.points_model_m.len(), MAX_POINTS);
    }
}
