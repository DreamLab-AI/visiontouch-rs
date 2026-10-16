//! View-only rendering. All hardware and servo work belongs to `worker`.
use crate::{
    model::*,
    worker::{Snapshot, Test},
};
use haptics_effects::Kind;
use macroquad::prelude::*;
use std::collections::VecDeque;

pub struct Orbit {
    pub yaw: f32,
    pub elevation: f32,
    pub distance: f32,
}
impl Default for Orbit {
    fn default() -> Self {
        Self {
            yaw: 0.55,
            elevation: 0.4,
            distance: 13.5,
        }
    }
}
impl Orbit {
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
            self.distance = (self.distance * (-mouse_wheel().1 * 0.09).exp()).clamp(6., 30.);
        }
    }
    pub fn camera(&self, r: Rect) -> Camera3D {
        Camera3D {
            position: vec3(
                self.yaw.sin() * self.elevation.cos(),
                self.elevation.sin(),
                self.yaw.cos() * self.elevation.cos(),
            ) * self.distance,
            target: Vec3::ZERO,
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
pub fn project(camera: &Camera3D, r: Rect, p: Vec3) -> Option<Vec2> {
    let c = camera.matrix() * p.extend(1.);
    if c.w <= 0. {
        return None;
    }
    let n = c.truncate() / c.w;
    let p = vec2(r.x + (n.x + 1.) * r.w * 0.5, r.y + (1. - n.y) * r.h * 0.5);
    r.contains(p).then_some(p)
}
pub fn scale(volume: &Volume) -> f32 {
    let extent = volume
        .bounds
        .iter()
        .flatten()
        .copied()
        .filter(|x| x.is_finite())
        .fold(1_f64, |a, b| a.max(b.abs()));
    (3.6 / extent) as f32
}
pub fn point(volume: &Volume, p: V) -> Vec3 {
    let w = volume.working(p).unwrap_or_else(|| p.map(|x| x * 1000.));
    vec3(w[0] as f32, w[1] as f32, w[2] as f32) * scale(volume)
}
fn ring(center: Vec3, radius: Vec3, axis: usize, color: Color) {
    let axes = [(1, 2), (0, 2), (0, 1)];
    let (a, b) = axes[axis];
    for i in 0..64 {
        let mut p = center;
        let mut q = center;
        let t = i as f32 * std::f32::consts::TAU / 64.;
        let u = (i + 1) as f32 * std::f32::consts::TAU / 64.;
        p[a] += radius[a] * t.cos();
        p[b] += radius[b] * t.sin();
        q[a] += radius[a] * u.cos();
        q[b] += radius[b] * u.sin();
        draw_line_3d(p, q, color);
    }
}
fn ellipsoid(center: Vec3, radius: Vec3, color: Color, deform: Option<(Vec3, f32)>) {
    let mut mesh = Mesh {
        vertices: Vec::new(),
        indices: Vec::new(),
        texture: None,
    };
    for y in 0..=20 {
        for x in 0..=32 {
            let a = x as f32 * std::f32::consts::TAU / 32.;
            let b = y as f32 * std::f32::consts::PI / 20.;
            let n = vec3(a.cos() * b.sin(), b.cos(), a.sin() * b.sin());
            let depression = deform.map_or(0., |(d, amount)| n.dot(d).max(0.).powi(8) * amount);
            let p = center + n * radius * (1. - depression);
            let shade = 0.35 + 0.65 * n.dot(vec3(-0.4, 0.8, 0.5).normalize()).max(0.);
            mesh.vertices.push(Vertex::new2(
                p,
                Vec2::ZERO,
                Color::new(color.r * shade, color.g * shade, color.b * shade, color.a),
            ));
            if y < 20 && x < 32 {
                let i = (y * 33 + x) as u16;
                mesh.indices
                    .extend_from_slice(&[i, i + 1, i + 33, i + 1, i + 34, i + 33]);
            }
        }
    }
    draw_mesh(&mesh);
}
fn arrow(a: Vec3, b: Vec3, color: Color) {
    draw_line_3d(a, b, color);
    let direction = (b - a).normalize_or_zero();
    if direction.length_squared() > 0. {
        let side = direction.cross(Vec3::Y).normalize_or_zero();
        let side = if side.length_squared() < 0.1 {
            Vec3::X
        } else {
            side
        };
        draw_line_3d(b, b - direction * 0.15 + side * 0.08, color);
        draw_line_3d(b, b - direction * 0.15 - side * 0.08, color);
    }
}
pub fn draw(
    volume: &Volume,
    s: &Snapshot,
    test: Test,
    trail: &VecDeque<V>,
    camera: &Camera3D,
    show_trail: bool,
) {
    set_camera(camera);
    let k = scale(volume);
    let lo = vec3(
        volume.bounds[0][0] as f32,
        volume.bounds[1][0] as f32,
        volume.bounds[2][0] as f32,
    ) * k;
    let hi = vec3(
        volume.bounds[0][1] as f32,
        volume.bounds[1][1] as f32,
        volume.bounds[2][1] as f32,
    ) * k;
    if lo.is_finite() && hi.is_finite() && lo.cmplt(hi).all() {
        draw_cube_wires((lo + hi) * 0.5, hi - lo, Color::from_rgba(55, 77, 94, 255));
        for i in 0..=10 {
            let t = i as f32 / 10.;
            draw_line_3d(
                vec3(lo.x + (hi.x - lo.x) * t, lo.y, lo.z),
                vec3(lo.x + (hi.x - lo.x) * t, lo.y, hi.z),
                Color::from_rgba(30, 47, 60, 255),
            );
            draw_line_3d(
                vec3(lo.x, lo.y, lo.z + (hi.z - lo.z) * t),
                vec3(hi.x, lo.y, lo.z + (hi.z - lo.z) * t),
                Color::from_rgba(30, 47, 60, 255),
            );
        }
    }
    let colors = [
        Color::from_rgba(234, 113, 126, 255),
        Color::from_rgba(125, 220, 160, 255),
        Color::from_rgba(113, 166, 247, 255),
    ];
    for (i, color) in colors.iter().enumerate() {
        let mut end = Vec3::ZERO;
        end[i] = 4.1;
        arrow(Vec3::ZERO, end, *color);
    }
    draw_sphere_wires(Vec3::ZERO, 0.12, None, Color::from_rgba(222, 234, 242, 255));
    let centre_model = volume.zero().unwrap_or([0.; 3]);
    let r = volume.radius().clamp(0.0002, 0.1);
    let radius = vec3(
        (point(volume, add(centre_model, [r, 0., 0.]))).x.abs(),
        point(volume, add(centre_model, [0., r, 0.])).y.abs(),
        point(volume, add(centre_model, [0., 0., r])).z.abs(),
    );
    if !s.sweep_mode
        && let Test::Object(kind) = test
    {
        match kind {
            Kind::Cube => {
                draw_cube(
                    Vec3::ZERO,
                    radius * 2.,
                    None,
                    Color::from_rgba(46, 95, 116, 255),
                );
                draw_cube_wires(
                    Vec3::ZERO,
                    radius * 2.005,
                    Color::from_rgba(113, 213, 225, 255),
                );
            }
            Kind::Water => {
                ellipsoid(
                    Vec3::ZERO,
                    radius,
                    Color::from_rgba(40, 104, 170, 110),
                    None,
                );
                for i in 0..3 {
                    ring(Vec3::ZERO, radius, i, Color::from_rgba(97, 180, 239, 190));
                }
                for y in -2..=2 {
                    let y = y as f32 / 3.;
                    let rr = (1. - y * y).sqrt();
                    ring(
                        vec3(0., radius.y * y, 0.),
                        radius * vec3(rr, 1., rr),
                        1,
                        Color::from_rgba(58, 119, 171, 150),
                    );
                }
            }
            Kind::Balloon => {
                if !s.frame.popped {
                    let direction = point(volume, s.position).normalize_or_zero();
                    ellipsoid(
                        Vec3::ZERO,
                        radius,
                        Color::from_rgba(220, 94, 131, 255),
                        Some((direction, s.frame.deformation as f32)),
                    );
                    draw_line_3d(
                        vec3(0., -radius.y, 0.),
                        vec3(0., -radius.y - 0.9, 0.),
                        Color::from_rgba(176, 129, 153, 255),
                    );
                } else {
                    for i in 0..12 {
                        let a = i as f32 * std::f32::consts::TAU / 12.;
                        let d = vec3(a.cos(), a.sin(), (a * 2.).sin() * 0.4);
                        draw_line_3d(
                            d * radius * 1.05,
                            d * radius * 1.4,
                            Color::from_rgba(237, 124, 156, 255),
                        );
                    }
                }
            }
            Kind::Springs => {
                let bodies = s.frame.bodies.map(|p| point(volume, add(centre_model, p)));
                for i in 0..3 {
                    for j in i + 1..3 {
                        let d = bodies[j] - bodies[i];
                        let side = d.cross(Vec3::Z).normalize_or_zero();
                        let mut prev = bodies[i];
                        for n in 1..=40 {
                            let t = n as f32 / 40.;
                            let p = bodies[i]
                                + d * t
                                + side * (t * std::f32::consts::TAU * 8.).sin() * 0.06;
                            draw_line_3d(prev, p, Color::from_rgba(133, 151, 192, 255));
                            prev = p;
                        }
                    }
                    draw_line_3d(Vec3::ZERO, bodies[i], Color::from_rgba(63, 63, 92, 255));
                    ellipsoid(bodies[i], radius * 0.28, colors[i], None);
                }
                ellipsoid(
                    Vec3::ZERO,
                    Vec3::splat(0.1),
                    Color::from_rgba(245, 200, 112, 255),
                    None,
                );
            }
            Kind::Plane => {
                draw_cube(
                    vec3(0., -0.015, 0.),
                    vec3(radius.x * 3., 0.03, radius.z * 3.),
                    None,
                    Color::from_rgba(74, 97, 130, 255),
                );
                draw_cube_wires(
                    Vec3::ZERO,
                    vec3(radius.x * 3., 0.01, radius.z * 3.),
                    Color::from_rgba(132, 167, 219, 255),
                );
            }
            Kind::Viscous => {
                for x in -2..=2 {
                    for y in -2..=2 {
                        for z in -2..=2 {
                            let p = radius * vec3(x as f32, y as f32, z as f32) * 0.6;
                            draw_line_3d(
                                p,
                                p + vec3(0.08, 0.03, 0.1),
                                Color::from_rgba(94, 113, 170, 255),
                            );
                        }
                    }
                }
            }
            Kind::Detents => {
                for x in -2..=2 {
                    for y in -2..=2 {
                        for z in -2..=2 {
                            draw_sphere_wires(
                                radius * vec3(x as f32, y as f32, z as f32) * 0.6,
                                0.045,
                                None,
                                Color::from_rgba(169, 133, 224, 255),
                            );
                        }
                    }
                }
            }
        }
    }
    if let Some(review) = &s.sweep.path_review {
        let start = point(volume, review.start_model_m);
        let end = point(volume, review.zero_model_m);
        draw_line_3d(start, end, Color::from_rgba(229, 194, 126, 255));
        if let Some([from, to]) = review.first_gap_fraction {
            draw_line_3d(
                start.lerp(end, from as f32),
                start.lerp(end, to as f32),
                RED,
            );
        }
    }
    for sample in &s.sweep.points_model_m {
        let p = point(volume, *sample);
        let dy = vec3(0., 0.018, 0.);
        draw_line_3d(p - dy, p + dy, Color::from_rgba(118, 231, 201, 190));
    }
    if show_trail {
        let mut previous = None;
        for (i, p) in trail.iter().enumerate() {
            let p = point(volume, *p);
            if let Some(prev) = previous {
                draw_line_3d(
                    prev,
                    p,
                    Color::new(
                        0.24,
                        0.65,
                        0.58,
                        0.2 + 0.6 * i as f32 / trail.len().max(1) as f32,
                    ),
                );
            }
            previous = Some(p);
        }
    }
    if s.reference {
        let pen = point(volume, s.position);
        let target = point(volume, s.target);
        draw_line_3d(
            pen,
            vec3(pen.x, lo.y, pen.z),
            Color::from_rgba(59, 104, 105, 255),
        );
        ellipsoid(
            pen,
            Vec3::splat(0.085),
            Color::from_rgba(124, 255, 218, 255),
            None,
        );
        if s.active {
            draw_sphere_wires(target, 0.13, None, Color::from_rgba(246, 201, 116, 255));
            draw_line_3d(pen, target, Color::from_rgba(199, 165, 102, 255));
            let force = s.force.map(|f| f as f32);
            arrow(
                pen,
                pen + vec3(force[0], force[1], force[2]) * 8.,
                Color::from_rgba(244, 143, 107, 255),
            );
        }
    }
    set_default_camera();
}
