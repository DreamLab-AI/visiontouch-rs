//! Geometry for reviewing an observed haptic workspace, without hardware I/O.
//!
//! An [`EvidenceCloud`] represents the union of neighbourhoods around measured
//! model positions. [`Policy::margin_m`] shrinks those neighbourhoods. This is
//! **sample support**, not mechanical clearance or proof of reachability. Sparse
//! boundary sweeps do not establish that their interior is accessible. This crate
//! never fills a convex hull, infers a hard stop, or authorizes motor output.
//!
//! [`EvidenceCloud::review_segment`] covers an entire segment analytically by
//! merging sphere-intersection intervals. Endpoint checks and fixed-step samples
//! can miss narrow gaps; this implementation preserves them.
//!
//! ```
//! use haptics_workspace::{EvidenceCloud, Policy, SegmentReview};
//! let cloud = EvidenceCloud::new(vec![[0., 0., 0.], [0.004, 0., 0.]],
//!     Policy::default()).unwrap();
//! assert!(cloud.contains([0.002, 0., 0.]));
//! assert_eq!(cloud.review_segment([0., 0., 0.], [0.004, 0., 0.]).unwrap(),
//!     SegmentReview::Supported);
//! // Points outside the observed neighbourhoods are not silently filled.
//! assert!(!cloud.contains([0., 0.01, 0.]));
//! ```
#![warn(missing_docs)]
use std::collections::HashMap;

/// Position in the caller's reference-model metres, with consistent origin/axes.
pub type Point = [f64; 3];
/// Bounded storage and review work. Applications may impose a smaller limit.
pub const MAX_POINTS: usize = 100_000;
const MAX_COORDINATE_M: f64 = 10.;
const NUMERICAL_INSET_M: f64 = 1e-10;

/// Independent evidence-review parameters; these are not physical safety limits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Policy {
    /// Maximum neighbourhood radius before applying the evidence margin.
    pub support_radius_m: f64,
    /// Inward distance removed from every neighbourhood, in model metres.
    pub margin_m: f64,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            support_radius_m: 0.005,
            margin_m: 0.002,
        }
    }
}
impl Policy {
    fn effective_radius(self) -> Result<f64, Error> {
        if !self.support_radius_m.is_finite()
            || !self.margin_m.is_finite()
            || !(0.0001..=0.05).contains(&self.support_radius_m)
            || self.margin_m < 0.
            || self.margin_m >= self.support_radius_m
            || self.support_radius_m - self.margin_m < 0.0001
        {
            return Err(Error::InvalidPolicy);
        }
        Ok(self.support_radius_m - self.margin_m - NUMERICAL_INSET_M)
    }
}

/// Invalid evidence is rejected, rather than silently dropping bad points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Support radius or margin is non-finite or outside the permitted range.
    InvalidPolicy,
    /// A coordinate is non-finite or outside ±10 model metres.
    InvalidPoint,
    /// Evidence exceeds the bounded storage limit [`MAX_POINTS`].
    TooManyPoints,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidPolicy => "support radius/margin must leave 0.1–50 mm of finite support",
            Self::InvalidPoint => "point must be finite and within ±10 model metres",
            Self::TooManyPoints => "workspace evidence exceeds 100,000 points",
        })
    }
}
impl std::error::Error for Error {}

/// Geometric support along the whole directed segment, parameterized from 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SegmentReview {
    /// The entire segment is covered by the shrunken sample neighbourhoods.
    /// This does not assert physical clearance or authorize motion.
    Supported,
    /// First uncovered interval. Interior fractions in this interval are outside
    /// all evidence neighbourhoods; its endpoints may themselves be covered.
    Gap {
        /// Beginning of the first uncovered interval, as a fraction in [0, 1].
        from: f64,
        /// End of the first uncovered interval, as a fraction in [0, 1].
        to: f64,
    },
}

/// Immutable model-position evidence and its bounded spatial index.
///
/// `contains` checks nearby bins; segment review is O(n log n) and allocates a
/// temporary interval vector, so run it outside the active force servo loop.
#[derive(Clone, Debug)]
pub struct EvidenceCloud {
    points: Vec<Point>,
    bins: HashMap<[i32; 3], Vec<usize>>,
    radius: f64,
    policy: Policy,
}
fn valid(p: Point) -> bool {
    p.into_iter()
        .all(|x| x.is_finite() && x.abs() <= MAX_COORDINATE_M)
}
fn sub(a: Point, b: Point) -> Point {
    std::array::from_fn(|i| a[i] - b[i])
}
fn dot(a: Point, b: Point) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
impl EvidenceCloud {
    /// Build an immutable index. An empty cloud is valid but supports no point.
    pub fn new(points: Vec<Point>, policy: Policy) -> Result<Self, Error> {
        let radius = policy.effective_radius()?;
        if points.len() > MAX_POINTS {
            return Err(Error::TooManyPoints);
        }
        if points.iter().any(|p| !valid(*p)) {
            return Err(Error::InvalidPoint);
        }
        let mut cloud = Self {
            points,
            bins: HashMap::new(),
            radius,
            policy,
        };
        for (i, p) in cloud.points.iter().enumerate() {
            cloud.bins.entry(cloud.cell(*p)).or_default().push(i);
        }
        Ok(cloud)
    }
    /// The requested policy; queries also apply a 0.1 nm numerical inward inset.
    pub fn policy(&self) -> Policy {
        self.policy
    }
    /// Original validated support observations, in model metres and insertion order.
    pub fn points(&self) -> &[Point] {
        &self.points
    }
    fn cell(&self, p: Point) -> [i32; 3] {
        p.map(|x| (x / self.radius).floor() as i32)
    }
    /// True only within a shrunken neighbourhood of an observed point.
    /// Invalid positions fail closed, even if the cloud is nonempty.
    pub fn contains(&self, p: Point) -> bool {
        if !valid(p) {
            return false;
        }
        let c = self.cell(p);
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    if let Some(indices) = self.bins.get(&[c[0] + x, c[1] + y, c[2] + z]) {
                        for i in indices {
                            let d = sub(self.points[*i], p);
                            if dot(d, d) <= self.radius * self.radius {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }
    /// Review complete continuous segment coverage, not just sampled waypoints.
    ///
    /// A zero-length segment is supported exactly when its point is supported.
    /// Invalid endpoints produce an error. A gap does not identify an obstacle;
    /// it means this evidence does not support the proposed path.
    pub fn review_segment(&self, start: Point, end: Point) -> Result<SegmentReview, Error> {
        if !valid(start) || !valid(end) {
            return Err(Error::InvalidPoint);
        }
        let direction = sub(end, start);
        let length = direction.into_iter().fold(0_f64, f64::hypot);
        if length == 0. {
            return Ok(if self.contains(start) {
                SegmentReview::Supported
            } else {
                SegmentReview::Gap { from: 0., to: 1. }
            });
        }
        let unit = direction.map(|x| x / length);
        let mut intervals = Vec::with_capacity(self.points.len());
        for p in &self.points {
            let delta = sub(*p, start);
            let centre = dot(delta, unit);
            let perpendicular: Point = std::array::from_fn(|i| delta[i] - centre * unit[i]);
            let residual = self.radius * self.radius - dot(perpendicular, perpendicular);
            if residual < 0. {
                continue;
            }
            let half = residual.sqrt();
            let lo = (centre - half).max(0.) / length;
            let hi = (centre + half).min(length) / length;
            if lo <= hi {
                intervals.push((lo, hi));
            }
        }
        intervals.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
        let mut cursor: f64 = 0.;
        for (lo, hi) in intervals {
            if lo > cursor {
                return Ok(SegmentReview::Gap {
                    from: cursor,
                    to: lo,
                });
            }
            cursor = cursor.max(hi);
            if cursor >= 1. {
                return Ok(SegmentReview::Supported);
            }
        }
        Ok(SegmentReview::Gap {
            from: cursor,
            to: 1.,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn supported_endpoints_do_not_hide_a_narrow_interior_gap() {
        let c = EvidenceCloud::new(vec![[0.; 3], [0.00601, 0., 0.]], Policy::default()).unwrap();
        assert!(c.contains([0.; 3]));
        assert!(c.contains([0.00601, 0., 0.]));
        let SegmentReview::Gap { from, to } = c.review_segment([0.; 3], [0.00601, 0., 0.]).unwrap()
        else {
            panic!("missed a 10 micrometre gap")
        };
        assert!(from < 0.5 && to > 0.5);
        assert!(to - from < 0.002);
    }
    #[test]
    fn bent_supported_route_does_not_imply_supported_shortcut() {
        let points = (0..=10)
            .flat_map(|i| [[i as f64 * 0.002, 0., 0.], [0.02, i as f64 * 0.002, 0.]])
            .collect();
        let c = EvidenceCloud::new(points, Policy::default()).unwrap();
        assert_eq!(
            c.review_segment([0.; 3], [0.02, 0., 0.]).unwrap(),
            SegmentReview::Supported
        );
        assert_eq!(
            c.review_segment([0.02, 0., 0.], [0.02, 0.02, 0.]).unwrap(),
            SegmentReview::Supported
        );
        assert!(matches!(
            c.review_segment([0.; 3], [0.02, 0.02, 0.]).unwrap(),
            SegmentReview::Gap { .. }
        ));
    }
    #[test]
    fn erosion_and_negative_bins_are_consistent_with_brute_force() {
        let points: Vec<_> = (0..15)
            .map(|i| [-0.015 + i as f64 * 0.002, 0.001, 0.])
            .collect();
        let c = EvidenceCloud::new(points.clone(), Policy::default()).unwrap();
        for x in -40..40 {
            for y in -10..10 {
                let p = [x as f64 * 0.0007, y as f64 * 0.0007, 0.];
                let expected = points.iter().any(|q| {
                    let d = sub(*q, p);
                    dot(d, d) <= c.radius * c.radius
                });
                assert_eq!(c.contains(p), expected);
            }
        }
        assert!(!c.contains([0., 0.004, 0.]));
    }
    #[test]
    fn invalid_and_empty_evidence_never_support_a_path() {
        assert!(EvidenceCloud::new(vec![[f64::NAN, 0., 0.]], Policy::default()).is_err());
        assert!(
            EvidenceCloud::new(
                vec![],
                Policy {
                    support_radius_m: 0.001,
                    margin_m: 0.002
                }
            )
            .is_err()
        );
        let c = EvidenceCloud::new(vec![], Policy::default()).unwrap();
        assert!(!c.contains([0.; 3]));
        assert!(!c.contains([f64::INFINITY; 3]));
        assert_eq!(
            c.review_segment([0.; 3], [0.; 3]).unwrap(),
            SegmentReview::Gap { from: 0., to: 1. }
        );
        assert!(c.review_segment([0.; 3], [11., 0., 0.]).is_err());
        assert!(matches!(
            EvidenceCloud::new(vec![[0.; 3]; MAX_POINTS + 1], Policy::default()),
            Err(Error::TooManyPoints)
        ));
    }
    #[test]
    fn tiny_segments_do_not_overflow_into_false_support() {
        let far = EvidenceCloud::new(vec![[1., 0., 0.]], Policy::default()).unwrap();
        let near = EvidenceCloud::new(vec![[0.; 3]], Policy::default()).unwrap();
        for length in [1e-150, 1e-200, 1e-300] {
            assert!(matches!(
                far.review_segment([0.; 3], [length, 0., 0.]).unwrap(),
                SegmentReview::Gap { .. }
            ));
            assert_eq!(
                near.review_segment([0.; 3], [length, 0., 0.]).unwrap(),
                SegmentReview::Supported
            );
        }
    }
    #[test]
    fn coverage_is_direction_independent_and_zero_length_is_a_point_query() {
        let c = EvidenceCloud::new(vec![[0.; 3], [0.004, 0., 0.]], Policy::default()).unwrap();
        for (a, b) in [
            ([0.; 3], [0.004, 0., 0.]),
            ([0.004, 0., 0.], [0.; 3]),
            ([0.; 3], [0.; 3]),
        ] {
            assert_eq!(c.review_segment(a, b).unwrap(), SegmentReview::Supported);
        }
    }
}
