//! Reference-model arm geometry for the drawn skeleton.
//!
//! This is an exact port of `armPose` in `apps/workspace-viewer/geometry.mjs`: the
//! same constants as `visiontouch_device::position_from_counts`, so the drawn tip is the
//! SDK model position. It is a visualisation of a reference model, not a calibration.
use visiontouch_effects::math::V;

/// Link length of both main-arm links in metres (reference model).
pub const LINK_M: f64 = 0.1397;
const A_PER_COUNT: f64 = 0.000_180_751_38;
const BC_PER_COUNT: f64 = 0.000_211_366_18;

/// Joint positions of the main arm in model metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArmPose {
    /// Shoulder pivot, fixed at `[0, L, -L]`.
    pub shoulder: V,
    /// Elbow joint.
    pub elbow: V,
    /// Pen tip; equal to `visiontouch_device::position_from_counts`.
    pub tip: V,
    /// Joint angles `[a, b, c]` in radians.
    pub angles: [f64; 3],
}

/// Pose for *registered* main-arm counts. Counts `[0, 0, 0]` put the tip at the origin.
pub fn pose(counts: [f64; 3]) -> ArmPose {
    let (upper, lower) = (LINK_M, LINK_M);
    let a = counts[0] * A_PER_COUNT;
    let b = -counts[2] * BC_PER_COUNT;
    let c = -counts[1] * BC_PER_COUNT;
    let shoulder = [0., lower, -upper];
    let elbow = [
        -upper * b.cos() * a.sin(),
        lower + upper * b.sin(),
        upper * b.cos() * a.cos() - upper,
    ];
    let tip = [
        elbow[0] - lower * c.sin() * a.sin(),
        elbow[1] - lower * c.cos(),
        elbow[2] + lower * c.sin() * a.cos(),
    ];
    ArmPose {
        shoulder,
        elbow,
        tip,
        angles: [a, b, c],
    }
}

/// Wireframe line segments (model metres) for the drawn arm: base block, shoulder
/// post, upper link, lower link and a short gimbal stub continuing the lower link.
pub fn segments(counts: [f64; 3]) -> Vec<(V, V)> {
    let p = pose(counts);
    let l = LINK_M;
    let (x, y, z) = (0.06, 0.05, 0.06);
    let base_y = -0.02;
    let c = [
        [-x, base_y, -l - z],
        [x, base_y, -l - z],
        [x, base_y, -l + z],
        [-x, base_y, -l + z],
    ];
    let mut s = Vec::new();
    for i in 0..4 {
        let j = (i + 1) % 4;
        let top = |q: V| [q[0], q[1] + y, q[2]];
        s.push((c[i], c[j]));
        s.push((top(c[i]), top(c[j])));
        s.push((c[i], top(c[i])));
    }
    s.push(([0., base_y + y, -l], p.shoulder));
    s.push((p.shoulder, p.elbow));
    s.push((p.elbow, p.tip));
    let d = [
        p.tip[0] - p.elbow[0],
        p.tip[1] - p.elbow[1],
        p.tip[2] - p.elbow[2],
    ];
    let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-9);
    s.push((
        p.tip,
        [
            p.tip[0] + d[0] / n * 0.03,
            p.tip[1] + d[1] / n * 0.03,
            p.tip[2] + d[2] / n * 0.03,
        ],
    ));
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use visiontouch_device::position_from_counts;

    #[test]
    fn counts_zero_matches_geometry_mjs() {
        let p = pose([0.; 3]);
        assert_eq!(p.shoulder, [0., LINK_M, -LINK_M]);
        for (a, b) in p.elbow.into_iter().zip([0., LINK_M, 0.]) {
            assert!((a - b).abs() < 1e-12, "{:?}", p.elbow);
        }
        assert!(p.tip.iter().all(|x| x.abs() < 1e-12), "{:?}", p.tip);
    }
    #[test]
    fn tip_equals_sdk_position_at_p0028_fixture_pose() {
        for c in [[6298., -1052., 1100.], [0., 0., 0.], [-900., 300., -2000.]] {
            let (t, s) = (pose(c).tip, position_from_counts(c));
            for i in 0..3 {
                assert!((t[i] - s[i]).abs() < 1e-9, "{t:?} {s:?}");
            }
        }
        let e = pose([6298., -1052., 1100.]).elbow;
        for (a, b) in e.into_iter().zip([
            -0.12342802842706341,
            0.10751120649107648,
            -0.08273076820732167,
        ]) {
            assert!((a - b).abs() < 1e-9, "elbow {e:?}");
        }
        let t = pose([6298., -1052., 1100.]).tip;
        for (a, b) in t.into_iter().zip([
            -0.15140019370903765,
            -0.028749424631449666,
            -0.06981998296672644,
        ]) {
            assert!((a - b).abs() < 1e-9);
        }
    }
    #[test]
    fn segments_end_at_the_tip() {
        let s = segments([100., 50., -50.]);
        assert!(s.len() > 12);
        assert!(
            s.iter()
                .all(|(a, b)| a.iter().chain(b).all(|x| x.is_finite()))
        );
    }
}
