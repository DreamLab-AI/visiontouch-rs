//! View-only rendering. All hardware and servo work belongs to `worker`.
//!
//! One scene unit is 50 mm, so the 140 mm links are 2.8 units long.
use crate::arm;
use crate::protocol::StepRequest;
use crate::worker::Snapshot;
use macroquad::prelude::*;
use visiontouch_effects::Kind;
use visiontouch_effects::math::{V, norm};

/// Scene units per metre.
pub const SCALE: f32 = 20.;
/// Most coverage points drawn per frame.
const MAX_DRAWN_CELLS: usize = 5000;
/// Arrow length per newton, in metres.
const ARROW_M_PER_N: f32 = 0.06;

/// Orbit camera state.
pub struct Orbit {
    pub yaw: f32,
    pub elevation: f32,
    pub distance: f32,
}
impl Default for Orbit {
    fn default() -> Self {
        Self {
            yaw: 0.7,
            elevation: 0.35,
            distance: 14.,
        }
    }
}
impl Orbit {
    /// Right drag orbits; the wheel zooms unless Shift is held.
    pub fn input(&mut self, rect: Rect, previous: Vec2) {
        let p = Vec2::from(mouse_position());
        if !rect.contains(p) {
            return;
        }
        if is_mouse_button_down(MouseButton::Right) {
            let d = p - previous;
            self.yaw -= d.x * 0.007;
            self.elevation = (self.elevation + d.y * 0.007).clamp(-0.2, 1.4);
        }
        if !is_key_down(KeyCode::LeftShift) {
            self.distance = (self.distance * (-mouse_wheel().1 * 0.09).exp()).clamp(5., 30.);
        }
    }
    /// The camera for a viewport.
    pub fn camera(&self, r: Rect) -> Camera3D {
        let target = vec3(0., 1.4, -1.4);
        Camera3D {
            position: target
                + vec3(
                    self.yaw.sin() * self.elevation.cos(),
                    self.elevation.sin(),
                    self.yaw.cos() * self.elevation.cos(),
                ) * self.distance,
            target,
            up: Vec3::Y,
            aspect: Some(r.w / r.h.max(1.)),
            fovy: 42_f32.to_radians(),
            z_near: 0.02,
            z_far: 100.,
            viewport: Some((
                r.x as i32,
                (screen_height() - r.y - r.h) as i32,
                r.w.max(1.) as i32,
                r.h.max(1.) as i32,
            )),
            ..Default::default()
        }
    }
}

/// Project a scene point to screen space, if it is in front of the camera and inside `r`.
pub fn project(camera: &Camera3D, r: Rect, p: Vec3) -> Option<Vec2> {
    let c = camera.matrix() * p.extend(1.);
    if c.w <= 0. {
        return None;
    }
    let n = c.truncate() / c.w;
    let p = vec2(r.x + (n.x + 1.) * r.w * 0.5, r.y + (1. - n.y) * r.h * 0.5);
    r.contains(p).then_some(p)
}

/// Model metres to scene units.
pub fn pt(v: V) -> Vec3 {
    vec3(v[0] as f32, v[1] as f32, v[2] as f32) * SCALE
}

fn ring(center: Vec3, radius: f32, axis: usize, color: Color) {
    let axes = [(1, 2), (0, 2), (0, 1)];
    let (a, b) = axes[axis];
    for i in 0..48 {
        let (mut p, mut q) = (center, center);
        let t = i as f32 * std::f32::consts::TAU / 48.;
        let u = (i + 1) as f32 * std::f32::consts::TAU / 48.;
        p[a] += radius * t.cos();
        p[b] += radius * t.sin();
        q[a] += radius * u.cos();
        q[b] += radius * u.sin();
        draw_line_3d(p, q, color);
    }
}
fn arrow(a: Vec3, b: Vec3, color: Color) {
    draw_line_3d(a, b, color);
    let d = (b - a).normalize_or_zero();
    if d.length_squared() > 0. {
        let side = d.cross(Vec3::Y).normalize_or_zero();
        let side = if side.length_squared() < 0.1 {
            Vec3::X
        } else {
            side
        };
        draw_line_3d(b, b - d * 0.18 + side * 0.09, color);
        draw_line_3d(b, b - d * 0.18 - side * 0.09, color);
    }
}
fn arm_lines(counts: [f64; 3], color: Color) {
    for (a, b) in arm::segments(counts) {
        draw_line_3d(pt(a), pt(b), color);
    }
}
fn wire_cube(c: Vec3, h: f32, color: Color) {
    let corner = |i: usize| {
        c + vec3(
            if i & 1 == 0 { -h } else { h },
            if i & 2 == 0 { -h } else { h },
            if i & 4 == 0 { -h } else { h },
        )
    };
    for i in 0..8 {
        for bit in [1, 2, 4] {
            if i & bit == 0 {
                draw_line_3d(corner(i), corner(i | bit), color);
            }
        }
    }
}

/// Draw the registered arm, reset ghost, coverage, active effect, plane, proxy and force arrow.
pub fn draw(s: &Snapshot, camera: &Camera3D) {
    set_camera(camera);
    draw_grid(
        24,
        1.,
        Color::new(0.2, 0.3, 0.38, 0.9),
        Color::new(0.12, 0.2, 0.27, 0.8),
    );
    for (axis, color) in [
        (Vec3::X, Color::new(0.92, 0.44, 0.49, 1.)),
        (Vec3::Y, Color::new(0.49, 0.86, 0.63, 1.)),
        (Vec3::Z, Color::new(0.44, 0.65, 0.97, 1.)),
    ] {
        draw_line_3d(Vec3::ZERO, axis * 3.4, color);
    }
    let reg = s.registration;
    let live: [f64; 3] = match reg {
        Some(r) => std::array::from_fn(|i| (s.continuous[i] - r.reset_counts[i]) as f64),
        None => std::array::from_fn(|i| s.continuous[i] as f64),
    };
    if reg.is_some() {
        arm_lines([0.; 3], Color::new(0.46, 0.91, 0.79, 0.28));
    }
    if s.connected {
        let color = if reg.is_some() {
            Color::new(0.46, 0.91, 0.79, 1.)
        } else {
            Color::new(0.97, 0.77, 0.45, 1.)
        };
        arm_lines(live, color);
        let tip = arm::pose(live).tip;
        draw_sphere(pt(tip), 0.14, None, color);
        let f = s.telemetry.applied_force;
        if norm(f) > 1e-6 && reg.is_some() {
            let capped = s.telemetry.force_capped || s.telemetry.dac_capped;
            let c = if capped {
                Color::new(0.97, 0.51, 0.54, 1.)
            } else {
                Color::new(1., 0.86, 0.4, 1.)
            };
            let tail = pt(tip);
            arrow(
                tail,
                tail + vec3(f[0] as f32, f[1] as f32, f[2] as f32) * ARROW_M_PER_N * SCALE,
                c,
            );
        }
    }
    if reg.is_some() && !s.coverage.is_empty() {
        let step = (s.coverage.len() / MAX_DRAWN_CELLS).max(1);
        for p in s.coverage.iter().step_by(step) {
            let q = pt(*p);
            draw_line_3d(
                q,
                q + vec3(0.03, 0.03, 0.03),
                Color::new(0.4, 0.62, 0.9, 0.45),
            );
        }
    }
    if let Some(StepRequest::Effect { kind }) = s.active {
        let centre = pt(s.centre.unwrap_or([0.; 3]));
        let r = 0.0225 * SCALE;
        let c = Color::new(1., 0.86, 0.4, 0.9);
        match kind {
            Kind::Cube => wire_cube(centre, r, c),
            Kind::Plane => {
                for i in -4..=4 {
                    let o = i as f32 * 0.5;
                    draw_line_3d(centre + vec3(o, 0., -2.), centre + vec3(o, 0., 2.), c);
                    draw_line_3d(centre + vec3(-2., 0., o), centre + vec3(2., 0., o), c);
                }
            }
            Kind::Springs => {
                if let Some(f) = s.effect_frame {
                    for b in f.bodies {
                        draw_sphere(centre + pt(b), 0.12, None, c);
                    }
                }
            }
            Kind::Water | Kind::Balloon | Kind::Viscous | Kind::Detents => {
                for axis in 0..3 {
                    ring(centre, r * 2., axis, c);
                }
            }
        }
        if let Some(p) = s.proxy {
            draw_sphere(pt(p), 0.1, None, Color::new(1., 0.7, 0.3, 1.));
        }
    }
    if s.active == Some(StepRequest::WallRamp)
        && let Some(t) = s.target
    {
        let y = pt(t).y;
        let c = Color::new(0.97, 0.51, 0.54, 0.8);
        for i in -4..=4 {
            let o = i as f32 * 0.6;
            draw_line_3d(vec3(o, y, -2.4), vec3(o, y, 2.4), c);
            draw_line_3d(vec3(-2.4, y, o), vec3(2.4, y, o), c);
        }
    }
    set_default_camera();
}
