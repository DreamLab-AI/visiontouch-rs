use rapier3d::prelude::*;

pub const POINTER_RADIUS: f32 = 0.2;

pub const DT: f32 = 1.0 / 120.0;
pub const MAX_BLOCKS: usize = 80;
pub const COLORS: [[u8; 3]; 6] = [
    [95, 219, 192],
    [249, 185, 101],
    [147, 158, 255],
    [242, 129, 142],
    [108, 187, 246],
    [216, 158, 236],
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    Drop,
    Stack,
    Scatter,
}
impl Preset {
    pub fn label(self) -> &'static str {
        match self {
            Self::Drop => "Mass comparison",
            Self::Stack => "Stack of blocks",
            Self::Scatter => "Mixed playground",
        }
    }
}

pub struct Block {
    pub id: u32,
    pub body: RigidBodyHandle,
    pub collider: ColliderHandle,
    pub size: [f32; 3],
    pub mass: f32,
    pub color: [u8; 3],
}

pub struct Simulation {
    pub world: PhysicsWorld,
    pub blocks: Vec<Block>,
    pub gravity: f32,
    pub bounce: f32,
    pub friction: f32,
    pub elapsed: f32,
    next_id: u32,
    pointer_body: RigidBodyHandle,
    coupling_target: Vector,
    pub reaction_force: Vector,
    pub free_block: Option<u32>,
    pointer_target: Option<Vector>,
    button_held: bool,
    grab: Option<(u32, ImpulseJointHandle)>,
}

impl Simulation {
    pub fn new(preset: Preset) -> Self {
        let mut world = PhysicsWorld::new();
        let (pointer_body, _) = world.insert(
            RigidBodyBuilder::dynamic()
                .gravity_scale(0.)
                .lock_rotations()
                .ccd_enabled(true)
                .can_sleep(false)
                .translation(Vector::new(0., -0.3, 0.))
                .enabled(false),
            ColliderBuilder::ball(POINTER_RADIUS)
                .mass(0.2)
                .friction(0.9)
                .restitution(0.)
                .collision_groups(InteractionGroups::new(
                    Group::GROUP_3,
                    Group::GROUP_2,
                    InteractionTestMode::And,
                )),
        );
        let mut sim = Self {
            world,
            pointer_body,
            coupling_target: Vector::new(0., -0.3, 0.),
            reaction_force: Vector::ZERO,
            free_block: None,
            pointer_target: None,
            button_held: false,
            grab: None,
            blocks: Vec::new(),
            gravity: 9.81,
            bounce: 0.2,
            friction: 0.65,
            elapsed: 0.,
            next_id: 1,
        };
        sim.world.integration_parameters.dt = DT;
        sim.world.insert_collider(
            ColliderBuilder::cuboid(7., 0.2, 6.)
                .translation(Vector::new(0., -0.2, 0.))
                .collision_groups(InteractionGroups::new(
                    Group::GROUP_1,
                    Group::GROUP_2,
                    InteractionTestMode::And,
                ))
                .friction(0.65)
                .restitution(0.2),
            None,
        );
        // Low walls keep most interactions on the visible tabletop.
        for (p, h) in [
            ([7., 0.25, 0.], [0.1, 0.25, 6.1]),
            ([-7., 0.25, 0.], [0.1, 0.25, 6.1]),
            ([0., 0.25, 6.], [7., 0.25, 0.1]),
            ([0., 0.25, -6.], [7., 0.25, 0.1]),
        ] {
            sim.world.insert_collider(
                ColliderBuilder::cuboid(h[0], h[1], h[2])
                    .translation(Vector::new(p[0], p[1], p[2]))
                    .collision_groups(InteractionGroups::new(
                        Group::GROUP_1,
                        Group::GROUP_2,
                        InteractionTestMode::And,
                    ))
                    .friction(0.65),
                None,
            );
        }
        match preset {
            Preset::Drop => {
                for (i, mass) in [0.5, 2., 10.].into_iter().enumerate() {
                    sim.add_block([i as f32 * 3. - 3., 3.8, 0.], [1.25; 3], mass);
                }
            }
            Preset::Stack => {
                for row in 0..5 {
                    for col in 0..2 {
                        sim.add_block(
                            [col as f32 * 1.35 - 0.675, 0.62 + row as f32 * 1.27, 0.],
                            [1.2; 3],
                            [0.5, 2., 10.][row % 3],
                        );
                    }
                }
            }
            Preset::Scatter => {
                for i in 0..15 {
                    let size = 0.75 + (i % 3) as f32 * 0.25;
                    let id = sim
                        .add_block(
                            [
                                (i % 5) as f32 * 2.2 - 4.4,
                                2. + (i % 4) as f32 * 0.8,
                                (i / 5) as f32 * 2.2 - 2.2,
                            ],
                            [size; 3],
                            [0.5, 2., 10.][i % 3],
                        )
                        .unwrap();
                    let b = sim.block(id).unwrap().body;
                    if sim.free_block == Some(id) {
                        sim.world.bodies[b].set_angvel(Vector::new(i as f32 * 0.1, 0.4, 0.2), true);
                    }
                }
            }
        }
        sim
    }

    pub fn block(&self, id: u32) -> Option<&Block> {
        self.blocks.iter().find(|b| b.id == id)
    }

    pub fn add_block(&mut self, p: [f32; 3], size: [f32; 3], mass: f32) -> Option<u32> {
        if self.blocks.len() >= MAX_BLOCKS {
            return None;
        }
        let id = self.next_id;
        self.next_id += 1;
        let (body, collider) = self.world.insert(
            RigidBodyBuilder::dynamic()
                .translation(Vector::new(p[0], p[1], p[2]))
                .ccd_enabled(true)
                .linear_damping(0.)
                .angular_damping(0.)
                .gyroscopic_forces_enabled(true),
            ColliderBuilder::cuboid(size[0] * 0.5, size[1] * 0.5, size[2] * 0.5)
                .collision_groups(InteractionGroups::new(
                    Group::GROUP_2,
                    Group::ALL,
                    InteractionTestMode::And,
                ))
                .mass(mass)
                .friction(self.friction)
                .restitution(self.bounce),
        );
        self.blocks.push(Block {
            id,
            body,
            collider,
            size,
            mass,
            color: COLORS[(id as usize - 1) % COLORS.len()],
        });
        self.refresh_constraints();
        Some(id)
    }

    pub fn spawn(&mut self, mass: f32, size: f32) -> Option<u32> {
        let index = self.next_id as usize;
        let x = ((index * 7) % 11) as f32 * 0.8 - 4.;
        let z = ((index * 3) % 9) as f32 * 0.8 - 3.2;
        let top = self
            .blocks
            .iter()
            .map(|b| self.world.bodies[b.body].translation().y + b.size[1])
            .fold(3.0_f32, f32::max)
            .min(12.);
        self.add_block([x, top + size, z], [size; 3], mass)
    }

    pub fn set_mass(&mut self, id: u32, mass: f32) {
        if let Some(block) = self.blocks.iter_mut().find(|b| b.id == id) {
            block.mass = mass.clamp(0.1, 50.);
            self.world.colliders[block.collider].set_mass(block.mass);
            let body = &mut self.world.bodies[block.body];
            body.recompute_mass_properties_from_colliders(&self.world.colliders);
            body.wake_up(true);
        }
        self.refresh_constraints();
    }

    fn refresh_constraints(&mut self) {
        let free = self
            .blocks
            .iter()
            .min_by(|a, b| a.mass.total_cmp(&b.mass).then(a.id.cmp(&b.id)))
            .map(|b| b.id);
        if self.free_block != free {
            self.release_grab();
        }
        self.free_block = free;
        for b in &self.blocks {
            let is_free = Some(b.id) == free;
            let body = &mut self.world.bodies[b.body];
            body.set_enabled_translations(is_free, true, is_free, true);
            body.lock_rotations(!is_free, true);
            if !is_free {
                body.set_linvel(Vector::new(0., body.linvel().y, 0.), true);
                body.set_angvel(Vector::ZERO, true);
                body.set_rotation(Rotation::IDENTITY, true);
            }
        }
    }

    pub fn momentum(&self, id: u32) -> Option<(Vector, Vector)> {
        let b = self.block(id)?;
        let body = &self.world.bodies[b.body];
        let [x, y, z] = b.size;
        let inertia =
            Vector::new(y * y + z * z, x * x + z * z, x * x + y * y) * (body.mass() / 12.);
        let local_omega = body.rotation().inverse() * body.angvel();
        Some((
            body.linvel() * body.mass(),
            *body.rotation() * (inertia * local_omega),
        ))
    }

    pub fn spin_free_block(&mut self, impulse: f32) {
        if let Some(b) = self
            .free_block
            .and_then(|id| self.block(id))
            .map(|b| b.body)
        {
            self.world.bodies[b].apply_torque_impulse(Vector::new(0., impulse, 0.), true);
        }
    }

    pub fn update_material(&mut self) {
        for (_, c) in self.world.colliders.iter_mut() {
            c.set_friction(self.friction);
            c.set_restitution(self.bounce);
        }
        self.wake_all();
    }
    pub fn wake_all(&mut self) {
        for (_, b) in self.world.bodies.iter_mut() {
            b.wake_up(true);
        }
    }
    pub fn impulse(&mut self, selected: Option<u32>, impulse: f32) {
        for b in &self.blocks {
            if selected.is_none_or(|id| id == b.id) {
                self.world.bodies[b.body].apply_impulse(Vector::new(0., impulse, 0.), true);
            }
        }
    }
    pub fn grabbed_block(&self) -> Option<u32> {
        self.grab.map(|(id, _)| id)
    }

    pub fn pointer_position(&self) -> Vector {
        self.world.bodies[self.pointer_body].translation()
    }

    fn release_grab(&mut self) {
        if let Some((_, joint)) = self.grab.take() {
            self.world.remove_impulse_joint(joint);
        }
    }

    pub fn set_pointer(&mut self, target: Option<Vector>, held: bool) {
        if target.is_none() || !held {
            self.release_grab();
        }
        let body = &mut self.world.bodies[self.pointer_body];
        if let Some(p) = target {
            if self.pointer_target.is_none() {
                body.set_translation(p, true);
                body.set_linvel(Vector::ZERO, true);
                self.coupling_target = p;
            }
            body.set_enabled(true);
        } else {
            body.set_enabled(false);
        }
        if target.is_none() {
            self.reaction_force = Vector::ZERO;
        }
        self.pointer_target = target;
        self.button_held = held && target.is_some();
    }

    fn advance_pointer(&mut self) {
        let Some(target) = self.pointer_target else {
            return;
        };
        let p = self.pointer_position();
        let delta = (target - self.coupling_target).clamp_length_max(8. * DT);
        self.coupling_target += delta;
        let body = &mut self.world.bodies[self.pointer_body];
        // Implicit spring/damper stabilizes the light proxy while it carries
        // heavier bodies; explicit high damping would inject numerical energy.
        let denominator = 1. + 60. * DT / 0.2 + 600. * DT * DT / 0.2;
        let force = (((self.coupling_target - p) * 600. + (delta / DT - body.linvel()) * 60.)
            / denominator)
            .clamp_length_max(250.);
        body.reset_forces(false);
        body.add_force(force, true);
        self.reaction_force = -force;
        if !self.button_held || self.grab.is_some() {
            return;
        }
        let mut closest = None;
        let mut distance = POINTER_RADIUS + 0.08;
        for b in &self.blocks {
            let body = &self.world.bodies[b.body];
            let local = body.position().inverse_transform_point(p);
            let half = Vector::from_array(b.size) * 0.5;
            let anchor = local.clamp(-half, half);
            let world_anchor = body.position().transform_point(anchor);
            let d = world_anchor.distance(p);
            if d < distance {
                closest = Some((b.id, b.body, anchor, world_anchor - p));
                distance = d;
            }
        }
        if let Some((id, body, local_anchor, pointer_anchor)) = closest {
            let joint = self.world.insert_impulse_joint(
                self.pointer_body,
                body,
                GenericJointBuilder::new(if self.free_block == Some(id) {
                    JointAxesMask::LIN_AXES
                } else {
                    JointAxesMask::LIN_Y
                })
                .local_anchor1(pointer_anchor)
                .local_anchor2(local_anchor)
                .contacts_enabled(false),
            );
            self.grab = Some((id, joint));
        }
    }

    pub fn remove(&mut self, id: u32) {
        if self.grabbed_block() == Some(id) {
            self.release_grab();
        }
        if let Some(index) = self.blocks.iter().position(|b| b.id == id) {
            let b = self.blocks.remove(index);
            self.world.remove_body(b.body);
            self.refresh_constraints();
        }
    }
    pub fn step(&mut self) {
        self.advance_pointer();
        self.world.gravity = Vector::new(0., -self.gravity, 0.);
        self.world.step();
        self.elapsed += DT;
        let lost: Vec<_> = self
            .blocks
            .iter()
            .filter(|b| self.world.bodies[b.body].translation().y < -30.)
            .map(|b| b.id)
            .collect();
        for id in lost {
            self.remove(id);
        }
    }
    pub fn energy(&self) -> f32 {
        self.blocks
            .iter()
            .map(|b| self.world.bodies[b.body].kinetic_energy())
            .sum()
    }
    pub fn awake(&self) -> usize {
        self.blocks
            .iter()
            .filter(|b| !self.world.bodies[b.body].is_sleeping())
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn settled_scene() -> Simulation {
        let mut sim = Simulation::new(Preset::Drop);
        for _ in 0..800 {
            sim.step();
        }
        sim
    }

    #[test]
    fn pointer_passes_under_floor_then_lifts_each_mass_by_collision() {
        for (id, x) in [(1, -3.), (2, 0.), (3, 3.)] {
            let mut sim = settled_scene();
            sim.set_pointer(Some(Vector::new(x - 1.5, -0.3, 0.)), false);
            sim.set_pointer(Some(Vector::new(x, -0.3, 0.)), false);
            for _ in 0..60 {
                sim.step();
            }
            assert!((sim.pointer_position().y + 0.3).abs() < 0.001);
            let handle = sim.block(id).unwrap().body;
            assert!((sim.world.bodies[handle].translation().y - 0.625).abs() < 0.03);
            let mut peak = 0_f32;
            for i in 0..180 {
                sim.set_pointer(Some(Vector::new(x, -0.3 + i as f32 * 0.008, 0.)), false);
                sim.step();
                peak = peak.max(sim.world.bodies[handle].translation().y);
            }
            // A freely balanced cube can tip off the spherical tool after lifting.
            assert!(
                peak > 1.2,
                "mass {}: {:?}",
                sim.block(id).unwrap().mass,
                sim.world.bodies[handle].translation()
            );
            assert!(sim.grabbed_block().is_none());
        }
    }

    #[test]
    fn held_button_grabs_nearby_block_and_release_restores_free_fall() {
        let mut sim = settled_scene();
        sim.set_pointer(Some(Vector::new(0., -0.21, 0.)), true);
        sim.step();
        assert_eq!(sim.grabbed_block(), Some(2));
        let handle = sim.block(2).unwrap().body;
        sim.set_pointer(Some(Vector::new(0., 3., 0.)), true);
        for _ in 0..180 {
            sim.step();
        }
        // The spherical grip permits rotation: a cube grabbed underneath can
        // swing below the tool, but its grabbed point must follow the tool.
        assert!(sim.world.bodies[handle].translation().y > 2.1);
        let joint = sim.world.impulse_joints.get(sim.grab.unwrap().1).unwrap();
        let a = sim.world.bodies[sim.pointer_body]
            .position()
            .transform_point(joint.data.local_anchor1());
        let b = sim.world.bodies[handle]
            .position()
            .transform_point(joint.data.local_anchor2());
        assert!(a.distance(b) < 0.05, "joint error {}", a.distance(b));
        // Move the released pointer clear so contact cannot support the block.
        sim.set_pointer(Some(Vector::new(4., 3., 0.)), false);
        assert!(sim.grabbed_block().is_none());
        for _ in 0..360 {
            sim.step();
        }
        assert!(sim.world.bodies[handle].translation().y < 1.2);
    }

    #[test]
    fn grab_does_not_reach_distant_blocks_and_input_loss_or_deletion_releases() {
        let mut sim = settled_scene();
        sim.set_pointer(Some(Vector::new(0., 6., 0.)), true);
        sim.step();
        assert!(sim.grabbed_block().is_none());
        sim.set_pointer(None, false);
        sim.set_pointer(Some(Vector::new(0., -0.21, 0.)), true);
        sim.step();
        assert_eq!(sim.grabbed_block(), Some(2));
        sim.set_pointer(None, true);
        assert!(sim.grabbed_block().is_none());
        assert!(!sim.world.bodies[sim.pointer_body].is_enabled());
        sim.set_pointer(Some(Vector::new(0., -0.21, 0.)), true);
        sim.step();
        sim.remove(2);
        assert!(sim.grabbed_block().is_none());
        assert_eq!(sim.world.impulse_joints().count(), 0);
    }

    #[test]
    fn held_weight_and_contact_weight_are_reported_as_pen_reaction() {
        for grab in [false, true] {
            let mut sim = settled_scene();
            sim.set_pointer(Some(Vector::new(0., -0.21, 0.)), grab);
            sim.step();
            sim.set_pointer(Some(Vector::new(0., 2., 0.)), grab);
            for _ in 0..600 {
                sim.step();
            }
            assert!(
                (sim.reaction_force.y + 2. * sim.gravity).abs() < 0.5,
                "grab={grab} reaction={:?}",
                sim.reaction_force
            );
            assert!(sim.reaction_force.x.abs() < 0.1 && sim.reaction_force.z.abs() < 0.1);
        }
    }

    #[test]
    fn only_lightest_is_free_and_both_momenta_persist_without_external_forces() {
        let mut sim = Simulation::new(Preset::Drop);
        sim.gravity = 0.;
        assert_eq!(sim.free_block, Some(1));
        let free = sim.block(1).unwrap().body;
        sim.world.bodies[free].apply_impulse(Vector::new(0.2, 0.1, 0.1), true);
        sim.world.bodies[free].apply_torque_impulse(Vector::new(0.02, 0.04, 0.01), true);
        let before = sim.momentum(1).unwrap();
        let locked = sim.block(2).unwrap().body;
        let origin = sim.world.bodies[locked].translation();
        sim.world.bodies[locked].apply_impulse(Vector::new(1., 2., 3.), true);
        sim.world.bodies[locked].apply_torque_impulse(Vector::ONE, true);
        for _ in 0..60 {
            sim.step();
        }
        let after = sim.momentum(1).unwrap();
        assert!(before.0.distance(after.0) < 0.001);
        assert!(before.1.distance(after.1) < 0.001);
        let p = sim.world.bodies[locked].translation();
        assert_eq!(p.x, origin.x);
        assert_eq!(p.z, origin.z);
        assert!(p.y > origin.y);
        assert_eq!(sim.world.bodies[locked].angvel(), Vector::ZERO);
        sim.set_mass(3, 0.1);
        assert_eq!(sim.free_block, Some(3));
        sim.remove(3);
        assert_eq!(sim.free_block, Some(1));
    }

    #[test]
    fn grab_reaction_changes_with_acceleration_and_release_keeps_motion() {
        let mut sim = settled_scene();
        sim.set_pointer(Some(Vector::new(-2.6, -0.21, 0.)), true);
        sim.step();
        assert_eq!(sim.grabbed_block(), Some(1));
        sim.set_pointer(Some(Vector::new(-2.6, 2., 0.)), true);
        let mut peak = 0_f32;
        for _ in 0..12 {
            sim.step();
            peak = peak.max(sim.reaction_force.length());
        }
        assert!(peak > 0.5 * sim.gravity * 2., "inertial force {peak}");
        let h = sim.block(1).unwrap().body;
        let v = sim.world.bodies[h].linvel();
        let w = sim.world.bodies[h].angvel();
        assert!(v.length() > 0.1);
        assert!(w.length() > 0.01);
        sim.set_pointer(None, false);
        assert_eq!(sim.world.bodies[h].linvel(), v);
        assert_eq!(sim.world.bodies[h].angvel(), w);
    }

    #[test]
    fn gravity_acceleration_does_not_depend_on_mass() {
        let mut sim = Simulation::new(Preset::Drop);
        for _ in 0..60 {
            sim.step();
        }
        let heights: Vec<_> = sim
            .blocks
            .iter()
            .map(|b| sim.world.bodies[b.body].translation().y)
            .collect();
        assert!(heights[0] < 3.0 && heights[0] > 2.4, "{heights:?}");
        assert!((heights[0] - heights[2]).abs() < 0.001);
    }
    #[test]
    fn equal_impulses_produce_inverse_mass_velocity() {
        let mut sim = Simulation::new(Preset::Drop);
        sim.impulse(None, 5.);
        for b in &sim.blocks {
            assert!((sim.world.bodies[b.body].linvel().y - 5. / b.mass).abs() < 0.001);
        }
    }
    #[test]
    fn mass_edits_update_inertia_and_impulse_response() {
        let mut sim = Simulation::new(Preset::Drop);
        sim.set_mass(1, 5.);
        sim.impulse(Some(1), 5.);
        let b = sim.block(1).unwrap();
        assert!((sim.world.bodies[b.body].mass() - 5.).abs() < 0.001);
        assert!((sim.world.bodies[b.body].linvel().y - 1.).abs() < 0.001);
    }
    #[test]
    fn blocks_settle_on_floor_without_falling_through() {
        let mut sim = Simulation::new(Preset::Drop);
        for _ in 0..1200 {
            sim.step();
        }
        for b in &sim.blocks {
            let body = &sim.world.bodies[b.body];
            assert!((body.translation().y - 0.625).abs() < 0.03);
            assert!(body.linvel().length() < 0.05);
        }
    }
    #[test]
    fn deletion_removes_body_and_collider_and_spawn_is_bounded() {
        let mut sim = Simulation::new(Preset::Drop);
        let collider = sim.block(1).unwrap().collider;
        sim.remove(1);
        assert!(sim.world.colliders.get(collider).is_none());
        for _ in 0..100 {
            sim.spawn(1., 1.);
        }
        assert_eq!(sim.blocks.len(), MAX_BLOCKS);
    }
}
