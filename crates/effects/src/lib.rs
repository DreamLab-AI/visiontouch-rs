//! Canonical haptic effects in metres, seconds and model newtons.
//!
//! This crate is pure simulation: it never opens hardware or arms motors. Pass
//! its reaction through `visiontouch_device::ForceGuard` and an explicitly armed
//! device session. The caller owns volume, timing and operator interlocks.
//! Positions are relative to the calibrated centre. A contact proxy is the
//! nearest point on a simulated surface; it can differ from the measured pen.
//!
//! ```
//! use visiontouch_effects::{Effect, Kind, Tuning};
//! let mut cube = Effect::new(Kind::Cube, 0.02, Tuning::default());
//! let contact = cube.step([0.019, 0., 0.], [-0.001, 0., 0.], 0.002);
//! assert!(contact.force[0] > 0.); // opposes penetration into the +X face
//! ```
#![warn(missing_docs)]
use serde::{Deserialize, Serialize};

/// Small, allocation-free three-dimensional operations used by control code.
pub mod math {
    /// Cartesian vector; units are defined by the calling API.
    pub type V = [f64; 3];
    /// Componentwise sum; callers provide compatible units and finite inputs.
    pub fn add(a: V, b: V) -> V {
        std::array::from_fn(|i| a[i] + b[i])
    }
    /// Componentwise difference, with the same units as the operands.
    pub fn sub(a: V, b: V) -> V {
        std::array::from_fn(|i| a[i] - b[i])
    }
    /// Scale every component by a scalar.
    pub fn mul(a: V, s: f64) -> V {
        a.map(|x| x * s)
    }
    /// Euclidean dot product; resulting units are the product of operand units.
    pub fn dot(a: V, b: V) -> f64 {
        a.into_iter().zip(b).map(|(a, b)| a * b).sum()
    }
    /// Euclidean length. Very large finite inputs may overflow the squared sum.
    pub fn norm(a: V) -> f64 {
        dot(a, a).sqrt()
    }
    /// Limit vector length to a nonnegative finite cap, preserving direction.
    /// Inputs must be finite and small enough for the squared norm not to overflow.
    /// This arithmetic helper does not validate input or replace a device force guard.
    pub fn limit(a: V, cap: f64) -> V {
        let n = norm(a);
        mul(a, if n > cap { cap / n } else { 1. })
    }
}
use math::*;

/// Reference stiffness in N/m that the fixed effect constants were written for.
pub const REFERENCE_STIFFNESS_N_M: f64 = 150.;
/// Lowest accepted [`Tuning::stiffness_n_m`].
pub const MIN_STIFFNESS_N_M: f64 = 20.;
/// Highest accepted [`Tuning::stiffness_n_m`].
pub const MAX_STIFFNESS_N_M: f64 = 1500.;

/// Operator-tunable effect strength, in model units. These are commands, not
/// measured delivered stiffness.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tuning {
    /// Wall stiffness in model N/m, clamped to 20-1500 (default 150). The cube,
    /// plane and balloon use it directly; the other effects scale their fixed
    /// constants by `stiffness_n_m / 150`, so every force is linear in it.
    pub stiffness_n_m: f64,
    /// Contact damping in model N·s/m for the cube and plane (default 4), clamped to
    /// 0-100. The balloon uses half of it; other effects keep their own damping.
    pub damping_n_s_m: f64,
}
impl Default for Tuning {
    fn default() -> Self {
        Self {
            stiffness_n_m: REFERENCE_STIFFNESS_N_M,
            damping_n_s_m: 4.,
        }
    }
}
impl Tuning {
    fn sanitised(self) -> Self {
        let d = Self::default();
        Self {
            stiffness_n_m: if self.stiffness_n_m.is_finite() {
                self.stiffness_n_m
                    .clamp(MIN_STIFFNESS_N_M, MAX_STIFFNESS_N_M)
            } else {
                d.stiffness_n_m
            },
            damping_n_s_m: if self.damping_n_s_m.is_finite() {
                self.damping_n_s_m.clamp(0., 100.)
            } else {
                d.damping_n_s_m
            },
        }
    }
}

/// Original models used to isolate surface, damping, deformation and dynamic issues.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    /// Axis-aligned solid cube with a spring/damper contact reaction.
    Cube,
    /// Spherical fluid region with speed-dependent drag and upward buoyancy.
    Water,
    /// Deformable sphere that latches a popped state at 35% indentation.
    Balloon,
    /// Three simulated bodies joined by springs and attracted to the origin.
    Springs,
    /// Horizontal surface resisting penetration below model Y = 0.
    Plane,
    /// Uniform damping opposing motion in every direction.
    Viscous,
    /// Periodic force wells on all three axes, with damping.
    Detents,
}
impl Kind {
    /// All seven scenes in the presentation order used by the calibration app.
    pub const ALL: [Self; 7] = [
        Self::Cube,
        Self::Water,
        Self::Balloon,
        Self::Springs,
        Self::Plane,
        Self::Viscous,
        Self::Detents,
    ];
    /// Short human-readable scene label.
    pub fn name(self) -> &'static str {
        match self {
            Self::Cube => "Hard cube",
            Self::Water => "Water sphere",
            Self::Balloon => "Balloon",
            Self::Springs => "Three-body springs",
            Self::Plane => "Virtual plane",
            Self::Viscous => "Viscous field",
            Self::Detents => "Magnetic detents",
        }
    }
    /// Operator instructions describing the intended qualitative response.
    pub fn instructions(self) -> &'static str {
        match self {
            Self::Cube => {
                "Approach any face from outside. Push inward to feel a bounded spring wall. The surface marker stays on the face; the pen marker shows real penetration."
            }
            Self::Water => {
                "Move through the blue sphere. Faster motion produces stronger drag; a small upward buoyancy appears inside. There is no solid shell."
            }
            Self::Balloon => {
                "Press into the balloon slowly. It deforms as resistance builds, then pops at 35% indentation. Release and hold again to reinflate."
            }
            Self::Springs => {
                "Nudge the three orbiting bodies. Springs link each pair and gravity pulls toward the central anchor. Pen contact exchanges momentum with a body."
            }
            Self::Plane => {
                "Move downward through the central horizontal plane, then slide sideways. Compare vertical resistance with free sideways motion."
            }
            Self::Viscous => {
                "Move in each direction, then stop. Drag should oppose velocity and vanish when stationary; compare slow and fast strokes."
            }
            Self::Detents => {
                "Move slowly along each axis through the glowing lattice. Periodic force wells should feel like equally spaced soft clicks."
            }
        }
    }
}

/// Force is an unclamped model reaction. The hardware layer must limit it.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    /// Unclamped tool reaction in model newtons; not measured physical force.
    pub force: V,
    /// Contact proxy in model metres relative to the scene centre.
    /// Equals the input pen position when no surface proxy is applied.
    pub proxy: V,
    /// Three body centres in scene-relative model metres, used by the spring demo.
    pub bodies: [V; 3],
    /// Balloon indentation divided by radius, clamped to [0, 1].
    pub deformation: f64,
    /// Latched balloon rupture; recreate the effect to reinflate.
    pub popped: bool,
}
impl Default for Frame {
    fn default() -> Self {
        Self {
            force: [0.; 3],
            proxy: [0.; 3],
            bodies: [[0.; 3]; 3],
            deformation: 0.,
            popped: false,
        }
    }
}

/// Stateful, bounded-step effect simulator. Recreate to reset a balloon or orbit.
pub struct Effect {
    kind: Kind,
    radius: f64,
    tuning: Tuning,
    frame: Frame,
    velocities: [V; 3],
}
impl Effect {
    /// `radius` is a cube half-width or sphere radius in metres, clamped to
    /// 0.2–100 model mm. Non-finite radii select the 20 mm default.
    /// `tuning` is sanitised (see [`Tuning`]). Construction resets deformation,
    /// rupture and body velocities.
    pub fn new(kind: Kind, radius: f64, tuning: Tuning) -> Self {
        let radius = if radius.is_finite() {
            radius.clamp(0.0002, 0.1)
        } else {
            0.02
        };
        let mut frame = Frame::default();
        for i in 0..3 {
            let a = i as f64 * std::f64::consts::TAU / 3.;
            frame.bodies[i] = [radius * a.cos(), radius * a.sin(), 0.];
        }
        Self {
            kind,
            radius,
            tuning: tuning.sanitised(),
            frame,
            velocities: [[0.; 3]; 3],
        }
    }
    /// Advance using scene-relative position `p` in model metres, velocity `v`
    /// in model m/s, and elapsed seconds `dt` in (0, 0.025].
    /// Non-finite input or an invalid interval returns a default zero-force frame
    /// without mutating the simulation. The caller should treat invalid input
    /// as a fault. Returned reactions still require device-side force limits.
    pub fn step(&mut self, p: V, v: V, dt: f64) -> Frame {
        if p.iter().chain(&v).any(|x| !x.is_finite()) || !dt.is_finite() || dt <= 0. || dt > 0.025 {
            return Frame::default();
        }
        let r = self.radius;
        let Tuning {
            stiffness_n_m: k,
            damping_n_s_m: b,
        } = self.tuning;
        let s = k / REFERENCE_STIFFNESS_N_M;
        let f = &mut self.frame;
        f.proxy = p;
        f.force = [0.; 3];
        match self.kind {
            Kind::Cube => {
                if p.iter().all(|x| x.abs() < r) {
                    let i = (0..3)
                        .min_by(|&a, &b| (r - p[a].abs()).total_cmp(&(r - p[b].abs())))
                        .unwrap();
                    let sign = if p[i] >= 0. { 1. } else { -1. };
                    f.proxy[i] = sign * r;
                    f.force[i] = sign * (k * (r - p[i].abs()) + b * (-sign * v[i]).max(0.));
                }
            }
            Kind::Plane => {
                if p[1] < 0. {
                    f.proxy[1] = 0.;
                    f.force[1] = -k * p[1] + b * (-v[1]).max(0.);
                }
            }
            Kind::Water => {
                let immersion = ((r - norm(p)) / (r * 0.25)).clamp(0., 1.);
                f.force = mul(v, -immersion * (4. + 80. * norm(v)) * s);
                f.force[1] += 0.03 * immersion * s;
            }
            Kind::Balloon => {
                let depth = (r - norm(p)).max(0.);
                f.deformation = (depth / r).clamp(0., 1.);
                if f.deformation >= 0.35 {
                    f.popped = true;
                }
                if depth > 0. && !f.popped {
                    let direction = mul(p, 1. / norm(p).max(1e-9));
                    f.proxy = mul(direction, r);
                    f.force = mul(
                        direction,
                        s * (80. * depth + 6000. * depth * depth)
                            + b * 0.5 * (-dot(v, direction)).max(0.),
                    );
                }
            }
            Kind::Viscous => f.force = mul(v, -8. * s),
            Kind::Detents => {
                let spacing = r * 0.6;
                f.force = std::array::from_fn(|i| {
                    s * (-0.06 * (p[i] * std::f64::consts::TAU / spacing).sin() - 0.5 * v[i])
                });
            }
            Kind::Springs => {
                let steps = (dt / 0.001).ceil() as usize;
                let h = dt / steps as f64;
                for _ in 0..steps {
                    let mut forces = [[0.; 3]; 3];
                    for (i, force) in forces.iter_mut().enumerate() {
                        let q = f.bodies[i];
                        let softened = (dot(q, q) + r * r * 0.25).powf(1.5);
                        *force = add(
                            mul(q, -0.03 * r * r / softened),
                            mul(self.velocities[i], -0.04),
                        );
                    }
                    for i in 0..3 {
                        for j in i + 1..3 {
                            let d = sub(f.bodies[j], f.bodies[i]);
                            let length = norm(d).max(1e-9);
                            let direction = mul(d, 1. / length);
                            let spring = mul(
                                direction,
                                8. * (length - r * 3_f64.sqrt())
                                    + 0.08
                                        * dot(
                                            sub(self.velocities[j], self.velocities[i]),
                                            direction,
                                        ),
                            );
                            forces[i] = add(forces[i], spring);
                            forces[j] = sub(forces[j], spring);
                        }
                    }
                    let mut reaction = [0.; 3];
                    for (i, force) in forces.iter_mut().enumerate() {
                        let delta = sub(p, f.bodies[i]);
                        let distance = norm(delta);
                        if distance < r * 0.28 {
                            let normal = if distance > 1e-9 {
                                mul(delta, 1. / distance)
                            } else {
                                [0., 1., 0.]
                            };
                            let contact = mul(
                                normal,
                                (100. * (r * 0.28 - distance)
                                    - 1.5 * dot(sub(v, self.velocities[i]), normal))
                                .max(0.),
                            );
                            *force = sub(*force, contact);
                            reaction = add(reaction, contact);
                        }
                        self.velocities[i] =
                            limit(add(self.velocities[i], mul(*force, h / 0.02)), 0.2);
                        f.bodies[i] = add(f.bodies[i], mul(self.velocities[i], h));
                    }
                    f.force = add(f.force, mul(reaction, s / steps as f64));
                }
            }
        }
        *f
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cube_and_plane_push_out_of_surface() {
        let f = Effect::new(Kind::Cube, 0.02, Tuning::default()).step(
            [0.019, 0., 0.],
            [-0.01, 0., 0.],
            0.002,
        );
        assert!(f.force[0] > 0.);
        assert_eq!(f.proxy[0], 0.02);
        let f = Effect::new(Kind::Plane, 0.02, Tuning::default()).step(
            [0., -0.001, 0.],
            [0.; 3],
            0.002,
        );
        assert!(f.force[1] > 0.);
        assert_eq!(f.proxy[1], 0.);
    }
    #[test]
    fn fluids_oppose_motion_and_detents_restore_toward_well() {
        for kind in [Kind::Water, Kind::Viscous] {
            let f = Effect::new(kind, 0.02, Tuning::default()).step([0.; 3], [0.01, 0., 0.], 0.002);
            assert!(f.force[0] < 0.);
        }
        let f = Effect::new(Kind::Detents, 0.02, Tuning::default()).step(
            [0.001, 0., 0.],
            [0.; 3],
            0.002,
        );
        assert!(f.force[0] < 0.);
        assert_eq!(
            Effect::new(Kind::Viscous, 0.02, Tuning::default())
                .step([0.; 3], [0.; 3], 0.002)
                .force,
            [0.; 3]
        );
    }
    #[test]
    fn balloon_deforms_pops_and_reset_restores_it() {
        let mut e = Effect::new(Kind::Balloon, 0.02, Tuning::default());
        let f = e.step([0.018, 0., 0.], [0.; 3], 0.002);
        assert!(f.deformation > 0.);
        assert!(f.force[0] > 0.);
        let f = e.step([0.012, 0., 0.], [0.; 3], 0.002);
        assert!(f.popped);
        assert_eq!(f.force, [0.; 3]);
        assert!(e.step([0.018, 0., 0.], [0.; 3], 0.002).popped);
        assert!(
            !Effect::new(Kind::Balloon, 0.02, Tuning::default())
                .step([0.018, 0., 0.], [0.; 3], 0.002)
                .popped
        );
    }
    #[test]
    fn three_body_integration_is_finite_symmetric_and_consistent() {
        let mut a = Effect::new(Kind::Springs, 0.02, Tuning::default());
        let mut b = Effect::new(Kind::Springs, 0.02, Tuning::default());
        let mut frame = Frame::default();
        for _ in 0..1000 {
            frame = a.step([1.; 3], [0.; 3], 0.002);
            for _ in 0..2 {
                b.step([1.; 3], [0.; 3], 0.001);
            }
        }
        for i in 0..3 {
            assert!(norm(sub(frame.bodies[i], b.frame.bodies[i])) < 1e-8);
            assert!(norm(frame.bodies[i]) < 0.1);
        }
        assert!(norm(frame.bodies.into_iter().fold([0.; 3], add)) < 1e-10);
    }
    #[test]
    fn tuning_scales_force_linearly() {
        let at = |kind, k: f64, p, v| {
            Effect::new(
                kind,
                0.02,
                Tuning {
                    stiffness_n_m: k,
                    ..Tuning::default()
                },
            )
            .step(p, v, 0.002)
            .force
        };
        for (kind, p, v) in [
            (Kind::Cube, [0.019, 0., 0.], [0.; 3]),
            (Kind::Plane, [0., -0.001, 0.], [0.; 3]),
            (Kind::Balloon, [0.018, 0., 0.], [0.; 3]),
            (Kind::Detents, [0.001, 0., 0.], [0.; 3]),
            (Kind::Viscous, [0.; 3], [0.01, 0., 0.]),
            (Kind::Water, [0.; 3], [0.01, 0., 0.]),
        ] {
            let a = at(kind, 150., p, v);
            let b = at(kind, 300., p, v);
            let c = at(kind, 1_000_000., p, v);
            let d = at(kind, 20., p, v);
            for i in 0..3 {
                let ratio = if kind == Kind::Balloon {
                    None
                } else {
                    Some(2.)
                };
                if let Some(r) = ratio {
                    assert!((b[i] - r * a[i]).abs() < 1e-12, "{kind:?} {a:?} {b:?}");
                }
                assert!(
                    (c[i].abs() - 10. * a[i].abs()).abs() < 1e-12 || kind == Kind::Balloon,
                    "clamped to 1500"
                );
                assert!(d[i].abs() <= a[i].abs() + 1e-12);
            }
        }
        // Cube and plane: exactly the spring law with the chosen stiffness.
        let f = at(Kind::Cube, 400., [0.019, 0., 0.], [0.; 3]);
        assert!((f[0] - 400. * 0.001).abs() < 1e-12);
    }
    #[test]
    fn invalid_inputs_never_produce_force() {
        let mut e = Effect::new(Kind::Cube, 0.02, Tuning::default());
        assert_eq!(e.step([f64::NAN, 0., 0.], [0.; 3], 0.002).force, [0.; 3]);
        assert_eq!(e.step([0.; 3], [0.; 3], 0.1).force, [0.; 3]);
    }
}
