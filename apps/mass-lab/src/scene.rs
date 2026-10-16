use crate::physics::Simulation;
use macroquad::prelude::*;

pub struct Orbit {
    pub yaw: f32,
    pub elevation: f32,
    pub distance: f32,
    pub target: Vec3,
}
impl Default for Orbit {
    fn default() -> Self {
        Self {
            yaw: 0.55,
            elevation: 0.55,
            distance: 18.,
            target: vec3(0., 1.2, 0.),
        }
    }
}
impl Orbit {
    pub fn camera(&self, rect: Rect) -> Camera3D {
        let p = vec3(
            self.yaw.sin() * self.elevation.cos(),
            self.elevation.sin(),
            self.yaw.cos() * self.elevation.cos(),
        ) * self.distance;
        Camera3D {
            position: self.target + p,
            target: self.target,
            up: Vec3::Y,
            aspect: Some(rect.w / rect.h.max(1.)),
            fovy: 42_f32.to_radians(),
            z_near: 0.05,
            z_far: 150.,
            viewport: Some((
                rect.x as i32,
                (screen_height() - rect.y - rect.h) as i32,
                rect.w.max(1.) as i32,
                rect.h.max(1.) as i32,
            )),
            ..Default::default()
        }
    }
    pub fn input(&mut self, rect: Rect, previous: Vec2) {
        let mouse = Vec2::from(mouse_position());
        if !rect.contains(mouse) {
            return;
        }
        let delta = mouse - previous;
        if is_mouse_button_down(MouseButton::Right) {
            self.yaw -= delta.x * 0.007;
            self.elevation = (self.elevation + delta.y * 0.007).clamp(0.12, 1.48);
        }
        if is_mouse_button_down(MouseButton::Middle) {
            let right = vec3(self.yaw.cos(), 0., -self.yaw.sin());
            let forward = vec3(self.yaw.sin(), 0., self.yaw.cos());
            self.target += (-right * delta.x + forward * delta.y) * self.distance * 0.0015;
            self.target.x = self.target.x.clamp(-10., 10.);
            self.target.z = self.target.z.clamp(-10., 10.);
        }
        self.distance = (self.distance * (-mouse_wheel().1 * 0.1).exp()).clamp(5., 42.);
    }
}

pub fn project(camera: &Camera3D, rect: Rect, point: Vec3) -> Option<Vec2> {
    let clip = camera.matrix() * point.extend(1.);
    if clip.w <= 0. {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    let p = vec2(
        rect.x + (ndc.x + 1.) * rect.w * 0.5,
        rect.y + (1. - ndc.y) * rect.h * 0.5,
    );
    rect.contains(p).then_some(p)
}

pub fn pick(sim: &Simulation, camera: &Camera3D, rect: Rect, point: Vec2) -> Option<u32> {
    let ndc = vec2(
        (point.x - rect.x) / rect.w * 2. - 1.,
        1. - (point.y - rect.y) / rect.h * 2.,
    );
    let inv = camera.matrix().inverse();
    let a = inv.project_point3(ndc.extend(-1.));
    let b = inv.project_point3(ndc.extend(1.));
    let direction = (b - a).normalize();
    let mut hit = None;
    let mut closest = f32::INFINITY;
    for block in &sim.blocks {
        let body = &sim.world.bodies[block.body];
        let p = body.translation();
        let r = body.rotation();
        let rotation = Quat::from_xyzw(r.x, r.y, r.z, r.w);
        let origin = rotation.inverse() * (a - vec3(p.x, p.y, p.z));
        let dir = rotation.inverse() * direction;
        let half = Vec3::from_array(block.size) * 0.5;
        if let Some(t) = ray_box(origin, dir, half)
            && t < closest
        {
            closest = t;
            hit = Some(block.id);
        }
    }
    hit
}
fn ray_box(origin: Vec3, dir: Vec3, half: Vec3) -> Option<f32> {
    let mut near = 0.0_f32;
    let mut far = f32::INFINITY;
    for i in 0..3 {
        if dir[i].abs() < 1e-6 {
            if origin[i].abs() > half[i] {
                return None;
            }
        } else {
            let a = (-half[i] - origin[i]) / dir[i];
            let b = (half[i] - origin[i]) / dir[i];
            near = near.max(a.min(b));
            far = far.min(a.max(b));
            if near > far {
                return None;
            }
        }
    }
    (far >= 0.).then_some(near)
}

fn tint(c: Color, k: f32) -> Color {
    Color::new(c.r * k, c.g * k, c.b * k, c.a)
}

fn draw_triangle_3d(a: Vec3, b: Vec3, c: Vec3, color: Color) {
    draw_mesh(&Mesh {
        vertices: [a, b, c]
            .map(|p| Vertex::new2(p, Vec2::ZERO, color))
            .to_vec(),
        indices: vec![0, 1, 2],
        texture: None,
    });
}
fn cube(position: Vec3, rotation: Quat, size: Vec3, color: Color, selected: bool) {
    let half = size * 0.5;
    let points = [
        vec3(-1., -1., -1.),
        vec3(1., -1., -1.),
        vec3(1., 1., -1.),
        vec3(-1., 1., -1.),
        vec3(-1., -1., 1.),
        vec3(1., -1., 1.),
        vec3(1., 1., 1.),
        vec3(-1., 1., 1.),
    ]
    .map(|p| position + rotation * (p * half));
    let faces = [
        ([0, 1, 2, 3], -Vec3::Z),
        ([5, 4, 7, 6], Vec3::Z),
        ([4, 0, 3, 7], -Vec3::X),
        ([1, 5, 6, 2], Vec3::X),
        ([3, 2, 6, 7], Vec3::Y),
        ([4, 5, 1, 0], -Vec3::Y),
    ];
    let light = vec3(-0.4, 0.85, 0.5).normalize();
    let mut mesh = Mesh {
        vertices: Vec::with_capacity(24),
        indices: Vec::with_capacity(36),
        texture: None,
    };
    for (face, normal) in faces {
        let shade = 0.46 + 0.54 * (rotation * normal).dot(light).max(0.);
        let base = mesh.vertices.len() as u16;
        for i in face {
            mesh.vertices
                .push(Vertex::new2(points[i], Vec2::ZERO, tint(color, shade)));
        }
        mesh.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    draw_mesh(&mesh);
    let edge = if selected {
        Color::new(0.91, 0.99, 0.97, 1.)
    } else {
        tint(color, 0.55)
    };
    for [a, b] in [
        [0, 1],
        [1, 2],
        [2, 3],
        [3, 0],
        [4, 5],
        [5, 6],
        [6, 7],
        [7, 4],
        [0, 4],
        [1, 5],
        [2, 6],
        [3, 7],
    ] {
        // Tiny expansion reduces z-fighting with the solid faces.
        draw_line_3d(
            position + (points[a] - position) * 1.002,
            position + (points[b] - position) * 1.002,
            edge,
        );
    }
}

pub fn draw(sim: &Simulation, selected: Option<u32>, camera: &Camera3D, grid: bool) {
    set_camera(camera);
    cube(
        vec3(0., -0.21, 0.),
        Quat::IDENTITY,
        vec3(14.2, 0.4, 12.2),
        Color::from_rgba(35, 47, 61, 255),
        false,
    );
    if grid {
        let minor = Color::from_rgba(52, 65, 79, 255);
        for i in -7..=7 {
            draw_line_3d(vec3(i as f32, 0.003, -6.), vec3(i as f32, 0.003, 6.), minor);
        }
        for i in -6..=6 {
            draw_line_3d(vec3(-7., 0.003, i as f32), vec3(7., 0.003, i as f32), minor);
        }
        draw_line_3d(
            vec3(-7., 0.006, 0.),
            vec3(7., 0.006, 0.),
            Color::from_rgba(112, 74, 79, 255),
        );
        draw_line_3d(
            vec3(0., 0.006, -6.),
            vec3(0., 0.006, 6.),
            Color::from_rgba(62, 105, 119, 255),
        );
    }
    for (p, s) in [
        (vec3(7., 0.25, 0.), vec3(0.2, 0.5, 12.2)),
        (vec3(-7., 0.25, 0.), vec3(0.2, 0.5, 12.2)),
        (vec3(0., 0.25, 6.), vec3(14., 0.5, 0.2)),
        (vec3(0., 0.25, -6.), vec3(14., 0.5, 0.2)),
    ] {
        cube(
            p,
            Quat::IDENTITY,
            s,
            Color::from_rgba(49, 65, 82, 255),
            false,
        );
    }
    for block in &sim.blocks {
        let body = &sim.world.bodies[block.body];
        let p = body.translation();
        let r = body.rotation();
        // Ground markers convey height without implying physically computed shadows.
        if p.x.abs() < 6.5 && p.z.abs() < 5.5 {
            let h = block.size[0] * 0.6;
            let c = Color::from_rgba(23, 32, 42, 255);
            draw_triangle_3d(
                vec3(p.x - h, 0.008, p.z - h),
                vec3(p.x + h, 0.008, p.z - h),
                vec3(p.x + h, 0.008, p.z + h),
                c,
            );
            draw_triangle_3d(
                vec3(p.x - h, 0.008, p.z - h),
                vec3(p.x + h, 0.008, p.z + h),
                vec3(p.x - h, 0.008, p.z + h),
                c,
            );
            if selected == Some(block.id) {
                draw_line_3d(
                    vec3(p.x, 0.01, p.z),
                    vec3(p.x, p.y, p.z),
                    Color::from_rgba(94, 150, 142, 255),
                );
            }
        }
        cube(
            vec3(p.x, p.y, p.z),
            Quat::from_xyzw(r.x, r.y, r.z, r.w),
            Vec3::from_array(block.size),
            Color::from_rgba(block.color[0], block.color[1], block.color[2], 255),
            selected == Some(block.id),
        );
    }
    set_default_camera();
}

pub fn draw_pointer(pointer: &crate::pointer::Pointer, sim: &Simulation, camera: &Camera3D) {
    if !pointer.visible || !pointer.connected || pointer.samples == 0 {
        return;
    }
    set_camera(camera);
    let position = sim.pointer_position();
    let p = vec3(position.x, position.y, position.z);
    let color = if pointer.live() {
        Color::from_rgba(108, 250, 212, 255)
    } else {
        Color::from_rgba(125, 138, 151, 255)
    };
    if pointer.show_trail {
        let trail: Vec<_> = pointer.trail.iter().copied().collect();
        for (i, pair) in trail.windows(2).enumerate() {
            let weight = 0.2 + 0.65 * i as f32 / trail.len().max(1) as f32;
            draw_line_3d(pair[0], pair[1], tint(color, weight));
        }
    }
    // Dashed projection and floor cross locate the pointer in depth.
    let count = (p.y / 0.15).ceil() as usize;
    for i in (0..count).step_by(2) {
        let a = (i as f32 * 0.15).min(p.y);
        let b = ((i + 1) as f32 * 0.15).min(p.y);
        draw_line_3d(
            vec3(p.x, a.max(0.02), p.z),
            vec3(p.x, b, p.z),
            tint(color, 0.5),
        );
    }
    draw_line_3d(
        vec3(p.x - 0.25, 0.02, p.z),
        vec3(p.x + 0.25, 0.02, p.z),
        color,
    );
    draw_line_3d(
        vec3(p.x, 0.02, p.z - 0.25),
        vec3(p.x, 0.02, p.z + 0.25),
        color,
    );
    draw_sphere(p, crate::physics::POINTER_RADIUS, None, color);
    draw_sphere_wires(p, 0.2, None, tint(color, 0.8));
    for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
        draw_line_3d(p + axis * 0.24, p + axis * 0.42, color);
        draw_line_3d(p - axis * 0.24, p - axis * 0.42, color);
    }
    set_default_camera();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn picking_handles_parallel_and_inside_rays() {
        assert_eq!(ray_box(vec3(0., 0., 3.), -Vec3::Z, Vec3::ONE), Some(2.));
        assert_eq!(ray_box(vec3(2., 0., 3.), -Vec3::Z, Vec3::ONE), None);
        assert_eq!(ray_box(Vec3::ZERO, Vec3::X, Vec3::ONE), Some(0.));
        assert_eq!(ray_box(vec3(0., 0., 3.), Vec3::Z, Vec3::ONE), None);
    }
}
