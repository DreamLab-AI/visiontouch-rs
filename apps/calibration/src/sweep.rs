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
    #[serde(skip)]
    cells: HashSet<[i32; 3]>,
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
        self.zero_model_m = Some(p);
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
