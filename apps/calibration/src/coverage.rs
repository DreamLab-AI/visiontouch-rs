//! Bounded display-only coverage cloud of positions visited while registered.
//!
//! Cells are 2 mm cubes; the cloud is capped so a long session cannot grow without
//! bound. It is a record of where the pen has been, not a workspace measurement.
use std::collections::HashSet;
use visiontouch_effects::math::V;

/// Cell edge in metres.
pub const CELL_M: f64 = 0.002;
/// Maximum number of distinct cells retained.
pub const MAX_CELLS: usize = 20_000;

/// Occupied 2 mm cells in registered model metres.
#[derive(Clone, Default, Debug)]
pub struct Coverage {
    cells: HashSet<[i32; 3]>,
    order: Vec<[i32; 3]>,
    /// Number of observations dropped because the cloud was full.
    pub dropped: u64,
}

impl Coverage {
    /// Record a registered position. Returns true if a new cell was occupied.
    pub fn record(&mut self, p: V) -> bool {
        if p.iter().any(|x| !x.is_finite() || x.abs() > 1.) {
            return false;
        }
        let c = p.map(|x| (x / CELL_M).floor() as i32);
        if self.cells.contains(&c) {
            return false;
        }
        if self.cells.len() >= MAX_CELLS {
            self.dropped += 1;
            return false;
        }
        self.cells.insert(c);
        self.order.push(c);
        true
    }
    /// Number of occupied cells.
    pub fn len(&self) -> usize {
        self.order.len()
    }
    /// True while no cell is occupied.
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }
    /// Forget everything.
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    /// Cell centres in model metres, in first-visit order.
    pub fn points(&self) -> Vec<V> {
        self.order
            .iter()
            .map(|c| c.map(|i| (f64::from(i) + 0.5) * CELL_M))
            .collect()
    }
    /// Viewer schema-1 export (`apps/workspace-viewer`). Refuses an empty cloud.
    pub fn export(&self, duration_s: f64) -> Result<serde_json::Value, String> {
        if self.is_empty() {
            return Err("coverage is empty; enable coverage and move the pen first".into());
        }
        Ok(serde_json::json!({
            "schema": 1,
            "name": "Calibration coverage cloud",
            "points": self.points(),
            "frames": [],
            "sample_count": self.len(),
            "duration_s": duration_s,
            "reference_label": "Reset-pose registration; display only, no encoder replay",
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cells_dedupe_and_cap() {
        let mut c = Coverage::default();
        assert!(c.record([0.0001, 0., 0.]));
        assert!(!c.record([0.0009, 0.0001, 0.]), "same 2 mm cell");
        assert!(c.record([0.0031, 0., 0.]));
        assert_eq!(c.len(), 2);
        assert!(!c.record([f64::NAN, 0., 0.]));
        for i in 0..(MAX_CELLS as i32 + 50) {
            c.record([
                f64::from(i % 140) * 0.003,
                f64::from(i / 140) * 0.003 - 0.2,
                0.,
            ]);
        }
        assert!(c.len() <= MAX_CELLS);
    }
    #[test]
    fn export_matches_viewer_schema_one() {
        let mut c = Coverage::default();
        assert!(c.export(1.).is_err());
        c.record([0.01, -0.02, 0.03]);
        let v = c.export(2.5).unwrap();
        assert_eq!(v["schema"], 1);
        assert_eq!(v["frames"].as_array().unwrap().len(), 0);
        let pts = v["points"].as_array().unwrap();
        assert_eq!(pts.len(), 1);
        assert!(
            pts[0]
                .as_array()
                .unwrap()
                .iter()
                .all(|x| x.as_f64().unwrap().abs() <= 10.)
        );
    }

    #[test]
    fn export_has_the_shape_of_the_shared_viewer_fixture() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../workspace-viewer/examples/calibration-coverage.json"
        ))
        .unwrap();
        let mut c = Coverage::default();
        c.record([0.001, 0.003, -0.005]);
        let mine = c.export(1.5).unwrap();
        let keys = |v: &serde_json::Value| {
            let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
            k.sort();
            k
        };
        assert_eq!(keys(&mine), keys(&fixture));
        assert_eq!(mine["schema"], fixture["schema"]);
        assert_eq!(mine["name"], fixture["name"]);
    }
}
