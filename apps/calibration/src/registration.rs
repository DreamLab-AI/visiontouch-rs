//! Registration support: stillness, reset-pose capture checks, ruler scale and sign,
//! gravity drop and repeatability. Everything here is hardware-free.
//!
//! A mark is only accepted from a still pen (250 ms within 0.5 mm), so a stale or
//! moving reading can never become a reference.
use crate::protocol::{Axis, Mark};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use visiontouch_effects::math::{V, norm, sub};

/// Stillness window in seconds.
pub const STILL_WINDOW_S: f64 = 0.25;
/// Largest extent of the pen within the stillness window, in metres.
pub const STILL_EXTENT_M: f64 = 0.0005;
/// Return-to-reset and repeatability tolerance in metres.
pub const RETURN_TOL_M: f64 = 0.002;
/// Ruler scale outside this range is recorded and warned, never a stop.
pub const SCALE_RANGE: (f64, f64) = (0.8, 1.25);
/// Largest tolerated off-axis fraction of the ruler slide.
pub const OFF_AXIS_MAX: f64 = 0.15;
/// A downward move this long (metres) within [`DROP_WINDOW_S`] is a drop.
pub const DROP_DISTANCE_M: f64 = 0.010;
/// Time limit for a drop, from first motion.
pub const DROP_WINDOW_S: f64 = 0.5;

/// Sliding-window stillness detector over positions.
#[derive(Default, Debug)]
pub struct Stillness {
    samples: VecDeque<(f64, V)>,
}
impl Stillness {
    /// Add a position at time `t` (seconds, monotonic).
    pub fn push(&mut self, t: f64, p: V) {
        self.samples.push_back((t, p));
        while self.samples.len() > 2 && self.samples[1].0 <= t - STILL_WINDOW_S {
            self.samples.pop_front();
        }
    }
    /// True once at least 250 ms of samples span no more than 0.5 mm.
    pub fn is_still(&self) -> bool {
        let (Some(first), Some(last)) = (self.samples.front(), self.samples.back()) else {
            return false;
        };
        if last.0 - first.0 < STILL_WINDOW_S {
            return false;
        }
        let mut lo = last.1;
        let mut hi = last.1;
        for (_, p) in &self.samples {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
        norm(sub(hi, lo)) <= STILL_EXTENT_M
    }
    /// Forget history, for example after a reconnect.
    pub fn clear(&mut self) {
        self.samples.clear();
    }
}

/// One axis of the ruler check.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RulerResult {
    /// Axis slid along.
    pub axis: Axis,
    /// Operator ruler distance in millimetres, signed along the shown +axis direction.
    pub measured_mm: f64,
    /// Model displacement along the axis, millimetres.
    pub model_mm: f64,
    /// Model over ruler. Ideal 1.0.
    pub scale: f64,
    /// Fraction of the slide that was off the axis.
    pub off_axis: f64,
    /// Model and ruler agree on direction.
    pub sign_ok: bool,
    /// Off-axis fraction within [`OFF_AXIS_MAX`].
    pub off_axis_ok: bool,
    /// Scale outside [`SCALE_RANGE`]: warn only.
    pub scale_warning: bool,
}
impl RulerResult {
    /// The stop-the-line condition: wrong sign or excessive off-axis motion.
    pub fn ok(&self) -> bool {
        self.sign_ok && self.off_axis_ok
    }
}

/// Score one ruler slide from registered start and end positions.
pub fn ruler_result(axis: Axis, start: V, end: V, measured_mm: f64) -> Result<RulerResult, String> {
    if !measured_mm.is_finite() || measured_mm == 0. || measured_mm.abs() > 500. {
        return Err("ruler distance must be a non-zero length up to 500 mm".into());
    }
    let d = sub(end, start).map(|x| x * 1000.);
    let total = norm(d);
    if total < 1. {
        return Err("the pen moved less than 1 mm between the two marks".into());
    }
    let along = d[axis.index()];
    let scale = along / measured_mm;
    let off_axis = (total * total - along * along).max(0.).sqrt() / total;
    Ok(RulerResult {
        axis,
        measured_mm,
        model_mm: along,
        scale,
        off_axis,
        sign_ok: along.signum() == measured_mm.signum(),
        off_axis_ok: off_axis <= OFF_AXIS_MAX,
        scale_warning: scale.abs() < SCALE_RANGE.0 || scale.abs() > SCALE_RANGE.1,
    })
}

/// Largest pairwise distance between captures, in millimetres; `None` below two.
pub fn repeatability_mm(points: &[V]) -> Option<f64> {
    if points.len() < 2 {
        return None;
    }
    let mut worst = 0_f64;
    for (i, a) in points.iter().enumerate() {
        for b in &points[i + 1..] {
            worst = worst.max(norm(sub(*a, *b)) * 1000.);
        }
    }
    Some(worst)
}

/// Marks and results of the registration checks for the current connection.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Checks {
    /// Return-to-reset captures in registered metres.
    pub returns: Vec<V>,
    /// Latest distance of a return capture from the reset origin, millimetres.
    pub return_error_mm: Option<f64>,
    /// Largest spread between return captures, millimetres.
    pub repeat_mm: Option<f64>,
    /// Gravity drop moved the pen toward -Y.
    pub gravity: Option<bool>,
    /// Ruler results per axis.
    pub ruler: [Option<RulerResult>; 3],
    /// Pending ruler start mark.
    pub ruler_start: Option<V>,
    /// Pending ruler end mark.
    pub ruler_end: Option<V>,
    /// Effect centre mark.
    pub effect_centre: Option<V>,
}
impl Checks {
    /// Record a gravity-drop result. A failure is sticky until re-registration clears the
    /// checks: a later pass never overwrites it.
    pub fn record_gravity(&mut self, ok: bool) {
        if self.gravity != Some(false) {
            self.gravity = Some(ok);
        }
    }
    /// Store a mark taken from a still pen at registered position `p`.
    pub fn capture(&mut self, mark: Mark, p: V, still: bool) -> Result<(), String> {
        if !still {
            return Err("hold the pen still for a quarter of a second, then capture".into());
        }
        match mark {
            Mark::RulerStart => self.ruler_start = Some(p),
            Mark::RulerEnd => self.ruler_end = Some(p),
            Mark::EffectCentre => self.effect_centre = Some(p),
            Mark::ReturnToReset => {
                self.returns.push(p);
                self.return_error_mm = Some(norm(p) * 1000.);
                // Spread among the captures only; the origin is the reset pose itself.
                self.repeat_mm = repeatability_mm(&self.returns);
            }
        }
        Ok(())
    }
    /// Score the pending ruler marks for `axis`.
    pub fn submit_ruler(&mut self, axis: Axis, measured_mm: f64) -> Result<RulerResult, String> {
        let (Some(a), Some(b)) = (self.ruler_start, self.ruler_end) else {
            return Err("capture both the ruler start and end marks first".into());
        };
        let r = ruler_result(axis, a, b, measured_mm)?;
        self.ruler[axis.index()] = Some(r);
        self.ruler_start = None;
        self.ruler_end = None;
        Ok(r)
    }
    /// The pen returned to the reset pose within 2 mm, and repeated captures agree.
    pub fn return_ok(&self) -> Option<bool> {
        let e = self.return_error_mm?;
        Some(e <= RETURN_TOL_M * 1000. && self.repeat_mm.is_none_or(|r| r <= RETURN_TOL_M * 1000.))
    }
    /// All three ruler slides agree on sign with acceptable off-axis motion.
    pub fn ruler_ok(&self) -> bool {
        self.ruler.iter().all(|r| r.is_some_and(|r| r.ok()))
    }
    /// Everything the sign-pulse rung needs from registration.
    pub fn complete(&self) -> bool {
        self.return_ok() == Some(true) && self.gravity == Some(true) && self.ruler_ok()
    }
    /// The first missing or failed prerequisite, named for the operator.
    pub fn missing(&self) -> Option<String> {
        match self.return_ok() {
            None => return Some("return-to-reset check (move away, return, capture)".into()),
            Some(false) => return Some("return-to-reset within 2 mm (redo registration)".into()),
            Some(true) => {}
        }
        match self.gravity {
            None => return Some("gravity drop (release the pen onto your palm)".into()),
            Some(false) => return Some("gravity drop moved toward +Y (sign wrong)".into()),
            Some(true) => {}
        }
        for a in Axis::ALL {
            match self.ruler[a.index()] {
                None => return Some(format!("ruler slide along {}", a.name())),
                Some(r) if !r.ok() => {
                    return Some(format!("ruler {} sign or off-axis failed", a.name()));
                }
                Some(_) => {}
            }
        }
        None
    }
}

/// Detects a released pen falling toward -Y after a period of stillness.
#[derive(Default, Debug)]
pub struct GravityWatch {
    anchor: Option<(f64, V)>,
    left_at: Option<f64>,
}
impl GravityWatch {
    /// Feed a registered position. `Some(true)` for a downward drop, `Some(false)`
    /// for an equally quick move up, which means the Y sign is wrong.
    pub fn update(&mut self, t: f64, p: V, still: bool) -> Option<bool> {
        if still {
            self.anchor = Some((t, p));
            self.left_at = None;
            return None;
        }
        let (_, a) = self.anchor?;
        let d = sub(p, a);
        let dist = norm(d);
        if dist > 0.001 && self.left_at.is_none() {
            self.left_at = Some(t);
        }
        let left = self.left_at?;
        if t - left > DROP_WINDOW_S {
            self.anchor = None;
            self.left_at = None;
            return None;
        }
        if dist >= DROP_DISTANCE_M {
            self.anchor = None;
            self.left_at = None;
            if d[1].abs() >= 0.7 * dist {
                return Some(d[1] < 0.);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stillness_needs_250_ms_within_half_a_millimetre() {
        let mut s = Stillness::default();
        for i in 0..100 {
            s.push(f64::from(i) * 0.0024, [0.; 3]);
        }
        assert!(!s.is_still(), "only 240 ms");
        s.push(0.25, [0.; 3]);
        assert!(s.is_still());
        s.push(0.26, [0.0006, 0., 0.]);
        assert!(!s.is_still(), "0.6 mm of travel");
        for i in 0..200 {
            s.push(0.27 + f64::from(i) * 0.0024, [0.0006, 0., 0.]);
        }
        assert!(s.is_still(), "the window slides past the earlier pose");
    }

    #[test]
    fn ruler_scale_sign_and_off_axis() {
        let r = ruler_result(Axis::X, [0.; 3], [0.05, 0., 0.], 50.).unwrap();
        assert!(r.ok() && !r.scale_warning && (r.scale - 1.).abs() < 1e-9);
        let r = ruler_result(Axis::X, [0.; 3], [-0.05, 0., 0.], 50.).unwrap();
        assert!(!r.sign_ok && !r.ok());
        let r = ruler_result(Axis::Y, [0.; 3], [0.01, 0.045, 0.], 50.).unwrap();
        assert!(!r.off_axis_ok, "{}", r.off_axis);
        let r = ruler_result(Axis::Z, [0.; 3], [0., 0., 0.025], 50.).unwrap();
        assert!(
            r.ok() && r.scale_warning,
            "scale 0.5 warns but is not a stop"
        );
        assert!(ruler_result(Axis::Z, [0.; 3], [0., 0., 0.0001], 50.).is_err());
        assert!(ruler_result(Axis::Z, [0.; 3], [0., 0., 0.05], 0.).is_err());
    }

    #[test]
    fn captures_need_a_still_pen_and_return_is_within_2_mm() {
        let mut c = Checks::default();
        assert!(c.capture(Mark::ReturnToReset, [0.; 3], false).is_err());
        c.capture(Mark::ReturnToReset, [0.0015, 0., 0.], true)
            .unwrap();
        assert_eq!(c.return_ok(), Some(true));
        c.capture(Mark::ReturnToReset, [0., 0.0021, 0.], true)
            .unwrap();
        assert_eq!(c.return_ok(), Some(false), "outside 2 mm");
        assert!(c.missing().unwrap().contains("return"));
    }

    #[test]
    fn repeatability_is_the_worst_pairwise_spread() {
        assert!(repeatability_mm(&[[0.; 3]]).is_none());
        let r = repeatability_mm(&[[0.; 3], [0.001, 0., 0.], [0., 0.003, 0.]]).unwrap();
        assert!((r - 10f64.sqrt()).abs() < 1e-9, "{r}");
        let mut c = Checks::default();
        for p in [[0.0012, 0., 0.], [-0.0012, 0., 0.], [0., 0.0012, 0.]] {
            c.capture(Mark::ReturnToReset, p, true).unwrap();
        }
        assert_eq!(c.return_ok(), Some(false), "spread above 2 mm");
    }

    #[test]
    fn ruler_marks_are_consumed_and_complete_flag_needs_all() {
        let mut c = Checks::default();
        assert!(c.submit_ruler(Axis::X, 50.).is_err());
        for a in Axis::ALL {
            let mut d = [0.; 3];
            d[a.index()] = 0.05;
            c.capture(Mark::RulerStart, [0.; 3], true).unwrap();
            c.capture(Mark::RulerEnd, d, true).unwrap();
            c.submit_ruler(a, 50.).unwrap();
        }
        assert!(c.ruler_start.is_none() && c.ruler_ok());
        assert!(!c.complete());
        c.capture(Mark::ReturnToReset, [0.0005, 0., 0.], true)
            .unwrap();
        c.gravity = Some(true);
        assert!(c.complete() && c.missing().is_none());
    }

    #[test]
    fn a_failed_gravity_drop_is_sticky() {
        let mut c = Checks::default();
        c.record_gravity(false);
        c.record_gravity(true);
        assert_eq!(c.gravity, Some(false));
        let mut c = Checks::default();
        c.record_gravity(true);
        c.record_gravity(false);
        assert_eq!(c.gravity, Some(false), "a fail replaces a pass");
    }

    #[test]
    fn gravity_drop_sign() {
        let mut g = GravityWatch::default();
        g.update(0., [0.; 3], true);
        let mut out = None;
        for i in 1..=30 {
            let t = f64::from(i) * 0.01;
            out = out.or(g.update(t, [0., -0.5 * 9.8 * t * t, 0.], false));
        }
        assert_eq!(out, Some(true));
        let mut g = GravityWatch::default();
        g.update(0., [0.; 3], true);
        let mut out = None;
        for i in 1..=30 {
            let t = f64::from(i) * 0.01;
            out = out.or(g.update(t, [0., 0.5 * 9.8 * t * t, 0.], false));
        }
        assert_eq!(out, Some(false));
        let mut g = GravityWatch::default();
        g.update(0., [0.; 3], true);
        let mut out = None;
        for i in 1..=200 {
            let t = f64::from(i) * 0.01;
            out = out.or(g.update(t, [0., -0.002 * t, 0.], false));
        }
        assert_eq!(out, None, "slow lowering is not a drop");
    }
}
