mod physics;
mod pointer;
mod scene;

use egui_macroquad::egui::{self, Color32, RichText};
use macroquad::prelude::*;
use physics::{DT, MAX_BLOCKS, Preset, Simulation};
use pointer::MuteReason;

/// Time scale counts as real time when within half a percent of 1.00x.
fn real_time(speed: f32) -> bool {
    (speed - 1.).abs() < 0.005
}

const ACCENT: Color32 = Color32::from_rgb(108, 225, 197);
const MUTED: Color32 = Color32::from_rgb(141, 158, 178);

struct App {
    sim: Simulation,
    preset: Preset,
    selected: Option<u32>,
    running: bool,
    speed: f32,
    accumulator: f32,
    spawn_mass: f32,
    spawn_size: f32,
    impulse: f32,
    grid: bool,
    labels: bool,
    orbit: scene::Orbit,
    pointer: pointer::Pointer,
}
impl App {
    fn new() -> Self {
        Self {
            sim: Simulation::new(Preset::Drop),
            preset: Preset::Drop,
            selected: Some(2),
            running: false,
            speed: 1.,
            accumulator: 0.,
            spawn_mass: 2.,
            spawn_size: 1.25,
            impulse: 5.,
            grid: true,
            labels: true,
            orbit: Default::default(),
            pointer: Default::default(),
        }
    }
    fn reset(&mut self, preset: Preset) {
        let (g, b, f) = (self.sim.gravity, self.sim.bounce, self.sim.friction);
        self.sim = Simulation::new(preset);
        self.sim.gravity = g;
        self.sim.bounce = b;
        self.sim.friction = f;
        self.sim.update_material();
        self.preset = preset;
        self.selected = self.sim.blocks.first().map(|b| b.id);
        self.accumulator = 0.;
        self.running = false;
    }
    fn sync_pointer(&mut self) {
        let p = self.pointer.position;
        self.sim.set_pointer(
            self.pointer
                .active()
                .then_some(rapier3d::prelude::Vector::new(p.x, p.y, p.z)),
            self.pointer.button_held,
        );
    }
    fn advance(&mut self, frame_time: f32) {
        self.sync_pointer();
        if !self.running {
            self.accumulator = 0.;
            return;
        }
        self.accumulator += (frame_time.clamp(0., 0.1) * self.speed).min(0.1);
        while self.accumulator >= DT {
            self.sim.step();
            self.accumulator -= DT;
        }
        if self.selected.is_some_and(|id| self.sim.block(id).is_none()) {
            self.selected = None;
        }
    }
    /// Why the pen gets no force right now, or `None` when force is flowing.
    fn force_mute_reason(&self) -> Option<MuteReason> {
        if !self.pointer.feedback {
            Some(MuteReason::FeedbackOff)
        } else if !self.pointer.registered {
            Some(MuteReason::Unregistered)
        } else if !self.running {
            Some(MuteReason::NotRunning)
        } else if !real_time(self.speed) {
            Some(MuteReason::NotRealTime)
        } else if !self.pointer.live() {
            Some(MuteReason::SceneStalled)
        } else {
            self.pointer.worker_muted
        }
    }
    /// Send this frame's proxy tether to the worker, or an explicit mute.
    fn send_coupling(&self) {
        let proxy = (self.running && real_time(self.speed)).then(|| {
            let p = self.sim.pointer_position();
            let v = self.sim.pointer_velocity();
            (vec3(p.x, p.y, p.z), vec3(v.x, v.y, v.z))
        });
        self.pointer.send_coupling(proxy);
    }
    fn delete_selected(&mut self) {
        if let Some(id) = self.selected.take() {
            self.sim.remove(id);
        }
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Mass Lab: 3D Physics".into(),
        window_width: 1280,
        window_height: 820,
        high_dpi: true,
        sample_count: 4,
        window_resizable: true,
        platform: miniquad::conf::Platform {
            linux_wm_class: "mass-lab",
            swap_interval: Some(1),
            ..Default::default()
        },
        ..Default::default()
    }
}
fn theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = Color32::from_rgb(17, 24, 34);
    style.visuals.window_fill = Color32::from_rgb(24, 33, 46);
    style.visuals.extreme_bg_color = Color32::from_rgb(12, 19, 28);
    style.visuals.faint_bg_color = Color32::from_rgb(26, 37, 51);
    style.visuals.override_text_color = Some(Color32::from_rgb(222, 231, 240));
    style.visuals.selection.bg_fill = Color32::from_rgb(40, 84, 79);
    style.visuals.selection.stroke = egui::Stroke::new(1.0_f32, ACCENT);
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(34, 47, 62);
    style.visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(30, 42, 56);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(48, 67, 83);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(49, 91, 83);
    style.visuals.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.0_f32, Color32::from_rgb(41, 55, 71));
    style.spacing.item_spacing = egui::vec2(10., 9.);
    style.spacing.button_padding = egui::vec2(12., 7.);
    style.spacing.slider_width = 147.;
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(14.));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(14.));
    ctx.set_style(style);
}
fn eyebrow(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(10.).color(MUTED).strong());
}
fn section(ui: &mut egui::Ui, title: &str, contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(Color32::from_rgb(24, 33, 46))
        .corner_radius(8)
        .inner_margin(12)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            eyebrow(ui, title);
            ui.add_space(3.);
            contents(ui);
        });
}
fn metric(ui: &mut egui::Ui, label: &str, value: String) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.monospace(value);
        });
    });
}

fn interface(app: &mut App, ctx: &egui::Context, fps: i32) -> (Rect, bool) {
    egui::TopBottomPanel::top("header")
        .exact_height(68.)
        .frame(
            egui::Frame::new()
                .fill(Color32::from_rgb(17, 24, 34))
                .inner_margin(egui::Margin::symmetric(22, 12)),
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                let (icon, _) = ui.allocate_exact_size(egui::vec2(30., 36.), egui::Sense::hover());
                let center = icon.center();
                let points = [
                    [0., -13.],
                    [12., -6.],
                    [12., 8.],
                    [0., 15.],
                    [-12., 8.],
                    [-12., -6.],
                ]
                .map(|[x, y]| center + egui::vec2(x, y));
                let stroke = egui::Stroke::new(1.6_f32, ACCENT);
                for i in 0..6 {
                    ui.painter()
                        .line_segment([points[i], points[(i + 1) % 6]], stroke);
                }
                let middle = center + egui::vec2(0., 1.);
                for i in [1, 3, 5] {
                    ui.painter().line_segment([middle, points[i]], stroke);
                }
                ui.vertical(|ui| {
                    ui.label(RichText::new("MASS LAB").size(20.).strong());
                    ui.label(
                        RichText::new("A playground for gravity & motion")
                            .size(11.)
                            .color(MUTED),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button("Reset")
                        .on_hover_text("Reset the current scene · R")
                        .clicked()
                    {
                        app.reset(app.preset);
                    }
                    if ui
                        .add_enabled(!app.running, egui::Button::new("Step"))
                        .on_hover_text("Advance one 1/120 s step · N")
                        .clicked()
                    {
                        app.sync_pointer();
                        app.sim.step();
                    }
                    let button = egui::Button::new(
                        RichText::new(if app.running {
                            "Pause"
                        } else {
                            "Run simulation"
                        })
                        .strong()
                        .color(Color32::from_rgb(15, 35, 32)),
                    )
                    .fill(ACCENT)
                    .min_size(egui::vec2(136., 36.));
                    if ui.add(button).on_hover_text("Space").clicked() {
                        app.running = !app.running;
                    }
                    ui.label(
                        RichText::new(format!("{:06.2} s", app.sim.elapsed))
                            .monospace()
                            .color(MUTED),
                    );
                });
            });
        });
    egui::TopBottomPanel::bottom("status")
        .exact_height(36.)
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.add_space(12.);
                ui.label(
                    RichText::new(if app.running { "RUNNING" } else { "PAUSED" })
                        .size(10.)
                        .strong()
                        .color(if app.running {
                            ACCENT
                        } else {
                            Color32::from_rgb(249, 185, 101)
                        }),
                );
                ui.separator();
                ui.label(
                    RichText::new(format!(
                        "{} blocks   ·   {} awake",
                        app.sim.blocks.len(),
                        app.sim.awake()
                    ))
                    .size(11.)
                    .color(MUTED),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(10.);
                    ui.label(
                        RichText::new(format!("{fps} FPS   /   120 Hz physics"))
                            .size(11.)
                            .monospace()
                            .color(MUTED),
                    );
                    ui.label(
                        RichText::new(
                            "Right drag: orbit   ·   Middle drag: pan   ·   Scroll: zoom",
                        )
                        .size(11.)
                        .color(MUTED),
                    );
                });
            });
        });
    egui::SidePanel::right("inspector")
        .exact_width(310.)
        .resizable(false)
        .frame(
            egui::Frame::new()
                .fill(Color32::from_rgb(17, 24, 34))
                .inner_margin(14),
        )
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    section(ui, "HAPTIC POINTER", |ui| {
                        let live = app.pointer.live();
                        ui.label(RichText::new(&app.pointer.status).color(if live { ACCENT } else { MUTED }));
                        ui.horizontal(|ui| {
                            if app.pointer.connected || app.pointer.connecting {
                                if ui.button("Disconnect").clicked() { app.pointer.disconnect(); }
                            } else if ui.button("Connect device").clicked() { app.pointer.connect(); }
                            if ui.add_enabled(live, egui::Button::new("Zero at reset pose · C")).clicked() { app.pointer.zero_at_reset_pose(); }
                        });
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut app.pointer.visible, "Show pointer");
                            ui.checkbox(&mut app.pointer.show_trail, "Trail");
                        });
                        ui.label(match app.sim.grabbed_block() {
                            Some(id) => format!("Holding block #{id} · release button to drop"),
                            None if app.pointer.button_held => "Button held · touch a block to grab".into(),
                            None => "Slide underneath to lift · hold button to grab".into(),
                        });
                        if !app.running { ui.label(RichText::new("Paused · press Space to interact").color(MUTED)); }
                        let mut feedback=app.pointer.feedback;
                        if ui.add_enabled(live && app.pointer.registered,egui::Checkbox::new(&mut feedback,"Enable pen force feedback")).changed() { app.pointer.set_feedback(feedback); }
                        if !app.pointer.registered && live { ui.label(RichText::new("Hold the arm at its reset pose, then zero at reset pose. Feedback needs registration.").size(11.).color(MUTED)); }
                        if app.pointer.feedback {
                            let f=app.pointer.applied_force;
                            let t=app.pointer.telemetry;
                            ui.label(format!("Pen force: {:.3} N command · cap {:.1} N / {} counts",(f[0]*f[0]+f[1]*f[1]+f[2]*f[2]).sqrt(),visiontouch_device::DEFAULT_FORCE_N,visiontouch_device::DEFAULT_DAC_OFFSET));
                            match app.force_mute_reason() {
                                Some(reason) => { ui.label(RichText::new(reason.text()).size(11.).color(Color32::from_rgb(249,185,101))); }
                                None => { ui.label(RichText::new("model command, not a measured force").size(11.).color(MUTED)); }
                            }
                            if t.force_capped || t.dac_capped || t.slewing || t.expired {
                                ui.label(RichText::new(format!("{}{}{}{}",if t.force_capped {"force cap · "} else {""},if t.dac_capped {"DAC cap · "} else {""},if t.slewing {"slew · "} else {""},if t.expired {"expired"} else {""})).size(11.).color(MUTED));
                            }
                            ui.label(RichText::new(format!("{} muted servo ticks",app.pointer.muted_ticks)).size(11.).color(MUTED));
                        }
                        ui.label("Coupling stiffness (model)");
                        ui.add(egui::Slider::new(&mut app.pointer.stiffness_n_m, pointer::STIFFNESS_RANGE).suffix(" N/m").logarithmic(true));
                        ui.collapsing("All six encoders", |ui| {
                            ui.monospace(format!("Arm: {:?}\nPen/wrist: {:?}",&app.pointer.encoders[..3],&app.pointer.encoders[3..]));
                            ui.monospace(format!("Continuous: {:?}",app.pointer.continuous));
                        });
                        ui.collapsing("Motion mapping", |ui| {
                            if ui.checkbox(&mut app.pointer.swap_height_depth, "Swap height / depth").changed() { app.pointer.trail.clear(); }
                            ui.label("Sensitivity");
                            let mut gain = app.pointer.sensitivity * 1000.;
                            if ui.add(egui::Slider::new(&mut gain, 0.5..=20.).suffix("×")).changed() {
                                app.pointer.sensitivity = gain / 1000.;
                                app.pointer.trail.clear();
                            }
                            ui.horizontal(|ui| {
                                ui.label("Invert");
                                for (i, axis) in ["X", "Y", "Z"].into_iter().enumerate() {
                                    if ui.checkbox(&mut app.pointer.invert[i], axis).changed() { app.pointer.trail.clear(); }
                                }
                            });
                            ui.label(RichText::new("Hold the reset pose (arm level, square to the base) and press Zero at reset pose. The pen then maps from registered model metres; the Cartesian scale is not physically calibrated.").size(11.).color(MUTED));
                            ui.monospace(format!("{} samples", app.pointer.samples));
                        });
                        if app.pointer.clamped && live {
                            ui.label(RichText::new("At scene limit: lower sensitivity or zero at the reset pose.").size(11.).color(Color32::from_rgb(249,185,101)));
                        }
                    });
                    ui.add_space(3.);
                    section(ui, "WORLD", |ui| {
                        ui.label("Lightest block: free · others: vertical");
                        let mut preset = app.preset;
                        egui::ComboBox::from_id_salt("preset")
                            .width(ui.available_width() - 8.)
                            .selected_text(preset.label())
                            .show_ui(ui, |ui| {
                                for p in [Preset::Drop, Preset::Stack, Preset::Scatter] {
                                    ui.selectable_value(&mut preset, p, p.label());
                                }
                            });
                        if preset != app.preset {
                            app.reset(preset);
                        }
                        ui.add_space(3.);
                        ui.label("Gravity");
                        let changed = ui
                            .add(
                                egui::Slider::new(&mut app.sim.gravity, 0.0..=25.)
                                    .suffix(" m/s²")
                                    .fixed_decimals(2),
                            )
                            .changed();
                        let mut gravity_changed = changed;
                        ui.horizontal(|ui| {
                            for (label, value) in [("Zero", 0.), ("Moon", 1.62), ("Earth", 9.81)] {
                                if ui
                                    .selectable_label((app.sim.gravity - value).abs() < 0.01, label)
                                    .clicked()
                                {
                                    app.sim.gravity = value;
                                    gravity_changed = true;
                                }
                            }
                        });
                        if gravity_changed {
                            app.sim.wake_all();
                        }
                        ui.label("Time scale");
                        let slid = ui
                            .add(
                                egui::Slider::new(&mut app.speed, 0.25..=2.)
                                    .suffix("×")
                                    .fixed_decimals(2),
                            )
                            .changed();
                        if slid && (app.speed - 1.).abs() < 0.03 {
                            app.speed = 1.;
                        }
                        ui.collapsing("Surface properties", |ui| {
                            let a = ui
                                .add(
                                    egui::Slider::new(&mut app.sim.bounce, 0.0..=1.).text("Bounce"),
                                )
                                .changed();
                            let b = ui
                                .add(
                                    egui::Slider::new(&mut app.sim.friction, 0.0..=1.)
                                        .text("Friction"),
                                )
                                .changed();
                            if a || b {
                                app.sim.update_material();
                            }
                        });
                    });
                    ui.add_space(3.);
                    section(ui, "BLOCKS", |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("blocks")
                            .max_height(122.)
                            .show(ui, |ui| {
                                for block in &app.sim.blocks {
                                    ui.horizontal(|ui| {
                                        let c = Color32::from_rgb(
                                            block.color[0],
                                            block.color[1],
                                            block.color[2],
                                        );
                                        ui.label(RichText::new("■").color(c));
                                        if ui
                                            .selectable_label(
                                                app.selected == Some(block.id),
                                                format!("Block {:02}", block.id),
                                            )
                                            .clicked()
                                        {
                                            app.selected = Some(block.id);
                                        }
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                ui.label(
                                                    RichText::new(format!("{:.1} kg", block.mass))
                                                        .color(MUTED)
                                                        .monospace(),
                                                );
                                            },
                                        );
                                    });
                                }
                                if app.sim.blocks.is_empty() {
                                    ui.label(RichText::new("Add a block to begin.").color(MUTED));
                                }
                            });
                    });
                    ui.add_space(3.);
                    section(ui, "SELECTED BLOCK", |ui| {
                        if let Some(id) = app.selected.filter(|id| app.sim.block(*id).is_some()) {
                            let block = app.sim.block(id).unwrap();
                            let body = &app.sim.world.bodies[block.body];
                            let height = body.translation().y;
                            let velocity = body.linvel().length();
                            let (linear_momentum, angular_momentum) = app.sim.momentum(id).unwrap();
                            let free = app.sim.free_block == Some(id);
                            let mut mass = block.mass;
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!("Block {id:02}")).strong().size(18.),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("Remove").clicked() {
                                            app.delete_selected();
                                        }
                                    },
                                );
                            });
                            if ui
                                .add(
                                    egui::Slider::new(&mut mass, 0.1..=50.)
                                        .logarithmic(true)
                                        .suffix(" kg")
                                        .fixed_decimals(1),
                                )
                                .changed()
                            {
                                app.sim.set_mass(id, mass);
                            }
                            metric(ui, "Height", format!("{height:.2} m"));
                            metric(ui, "Speed", format!("{velocity:.2} m/s"));
                            ui.label(if free {"Free translation + rotation"} else {"Vertical motion only"});
                            metric(ui, "Linear momentum", format!("{:.3} kg·m/s",linear_momentum.length()));
                            metric(ui, "Angular momentum", format!("{:.3} kg·m²/s",angular_momentum.length()));
                        } else {
                            ui.label(
                                RichText::new("Click a block in the scene to inspect it.")
                                    .color(MUTED),
                            );
                        }
                    });
                    ui.add_space(3.);
                    section(ui, "IMPULSE EXPERIMENT", |ui| {
                        ui.label(
                            RichText::new("Same push. Different response.")
                                .color(MUTED)
                                .size(12.),
                        );
                        ui.add(
                            egui::Slider::new(&mut app.impulse, 0.5..=20.)
                                .suffix(" N·s")
                                .fixed_decimals(1),
                        );
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(
                                    app.selected.is_some(),
                                    egui::Button::new("Push selected"),
                                )
                                .clicked()
                            {
                                app.sim.impulse(app.selected, app.impulse);
                                app.running = true;
                            }
                            if ui.button("Push all").clicked() {
                                app.sim.impulse(None, app.impulse);
                                app.running = true;
                            }
                        });
                        if ui.button("Spin lightest block").clicked() {
                            app.sim.spin_free_block(0.1);
                            app.running=true;
                        }
                        ui.label(
                            RichText::new("An upward impulse changes velocity by Δv = J / m.")
                                .size(11.)
                                .color(MUTED),
                        );
                    });
                    ui.add_space(3.);
                    section(ui, "ADD A BLOCK", |ui| {
                        ui.add(
                            egui::Slider::new(&mut app.spawn_mass, 0.1..=50.)
                                .logarithmic(true)
                                .suffix(" kg")
                                .fixed_decimals(1),
                        );
                        ui.add(
                            egui::Slider::new(&mut app.spawn_size, 0.4..=2.)
                                .suffix(" m side")
                                .fixed_decimals(2),
                        );
                        if ui
                            .add_enabled(
                                app.sim.blocks.len() < MAX_BLOCKS,
                                egui::Button::new("+  Drop new block")
                                    .min_size(egui::vec2(ui.available_width(), 32.)),
                            )
                            .clicked()
                        {
                            app.selected = app.sim.spawn(app.spawn_mass, app.spawn_size);
                            app.running = true;
                        }
                        if app.sim.blocks.len() >= MAX_BLOCKS {
                            ui.label("80-block limit reached.");
                        }
                    });
                    ui.add_space(3.);
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut app.grid, "Grid");
                        ui.checkbox(&mut app.labels, "Labels");
                        if ui.small_button("Home view").clicked() {
                            app.orbit = Default::default();
                        }
                    });
                    ui.label(
                        RichText::new(
                            "No air resistance: mass does not change free-fall acceleration.",
                        )
                        .size(11.)
                        .color(MUTED),
                    );
                });
        });
    let r = ctx.available_rect();
    let rect = Rect::new(r.min.x, r.min.y, r.width().max(1.), r.height().max(1.));
    let painter = ctx
        .layer_painter(egui::LayerId::new(
            egui::Order::Background,
            egui::Id::new("scene-text"),
        ))
        .with_clip_rect(r);
    painter.text(
        r.min + egui::vec2(26., 25.),
        egui::Align2::LEFT_TOP,
        "EXPERIMENT / 01",
        egui::FontId::monospace(11.),
        ACCENT,
    );
    painter.text(
        r.min + egui::vec2(26., 47.),
        egui::Align2::LEFT_TOP,
        app.preset.label(),
        egui::FontId::proportional(25.),
        Color32::from_rgb(226, 235, 245),
    );
    painter.text(
        r.min + egui::vec2(27., 82.),
        egui::Align2::LEFT_TOP,
        "Select a block to explore its mass and motion.",
        egui::FontId::proportional(12.),
        MUTED,
    );
    painter.text(
        r.left_bottom() + egui::vec2(26., -23.),
        egui::Align2::LEFT_BOTTOM,
        format!(
            "1 grid square = 1 m     ·     Kinetic energy {:.2} J",
            app.sim.energy()
        ),
        egui::FontId::monospace(11.),
        MUTED,
    );
    if app.labels {
        let camera = app.orbit.camera(rect);
        for block in &app.sim.blocks {
            let p = app.sim.world.bodies[block.body].translation();
            if let Some(pos) =
                scene::project(&camera, rect, vec3(p.x, p.y + block.size[1] * 0.8, p.z))
            {
                let text = format!("{:.1} kg", block.mass);
                let label_rect =
                    egui::Rect::from_center_size(egui::pos2(pos.x, pos.y), egui::vec2(66., 24.));
                painter.rect_filled(
                    label_rect,
                    5.,
                    Color32::from_rgba_unmultiplied(16, 24, 34, 230),
                );
                painter.text(
                    label_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    text,
                    egui::FontId::monospace(12.),
                    Color32::from_rgb(block.color[0], block.color[1], block.color[2]),
                );
            }
        }
    }
    if app.pointer.visible && app.pointer.connected && app.pointer.samples > 0 {
        let camera = app.orbit.camera(rect);
        let p = app.sim.pointer_position();
        let point = vec3(p.x, p.y, p.z);
        if let Some(pos) = scene::project(&camera, rect, point) {
            painter.circle_stroke(
                egui::pos2(pos.x, pos.y),
                7.,
                egui::Stroke::new(2_f32, if app.pointer.live() { ACCENT } else { MUTED }),
            );
        }
        if let Some(pos) = scene::project(&camera, rect, point + Vec3::Y * 0.4) {
            painter.text(
                egui::pos2(pos.x, pos.y),
                egui::Align2::CENTER_BOTTOM,
                if app.pointer.live() {
                    "HAPTIC POINTER"
                } else {
                    "POINTER · STALE"
                },
                egui::FontId::monospace(11.),
                if app.pointer.live() { ACCENT } else { MUTED },
            );
        }
    }
    (rect, ctx.wants_keyboard_input())
}

#[macroquad::main(window_conf)]
async fn main() {
    if let Err(e) = visiontouch_device::shutdown::install() {
        eprintln!("Cannot install orderly shutdown: {e}");
        return;
    }
    prevent_quit();
    let smoke = std::env::args().any(|a| a == "--smoke-test");
    let screenshot = std::env::var("MASS_LAB_SCREENSHOT").ok();
    let mut app = App::new();
    if !smoke && !std::env::args().any(|a| a == "--no-device") {
        app.pointer.connect();
        app.running = true;
    }
    let mut previous_mouse = Vec2::from(mouse_position());
    let mut frame = 0;
    loop {
        if is_quit_requested() || visiontouch_device::shutdown::requested() {
            break;
        }
        app.pointer.poll();
        clear_background(Color::from_rgba(13, 20, 30, 255));
        let mut rect = Rect::new(
            0.,
            68.,
            (screen_width() - 310.).max(1.),
            (screen_height() - 104.).max(1.),
        );
        let mut keyboard = false;
        egui_macroquad::ui(|ctx| {
            if frame == 0 {
                theme(ctx);
            }
            (rect, keyboard) = interface(&mut app, ctx, get_fps());
        });
        if !keyboard {
            if is_key_pressed(KeyCode::C) {
                app.pointer.zero_at_reset_pose();
            }
            if is_key_pressed(KeyCode::Escape) {
                break;
            }
            if is_key_pressed(KeyCode::Space) {
                app.running = !app.running;
            }
            if is_key_pressed(KeyCode::R) {
                app.reset(app.preset);
            }
            if is_key_pressed(KeyCode::N) && !app.running {
                app.sync_pointer();
                app.sim.step();
            }
            if is_key_pressed(KeyCode::Home) {
                app.orbit = Default::default();
            }
            if is_key_pressed(KeyCode::Delete) {
                app.delete_selected();
            }
        }
        app.orbit.input(rect, previous_mouse);
        previous_mouse = Vec2::from(mouse_position());
        let camera = app.orbit.camera(rect);
        if rect.contains(previous_mouse) && is_mouse_button_pressed(MouseButton::Left) {
            app.selected = scene::pick(&app.sim, &camera, rect, previous_mouse);
        }
        app.advance(get_frame_time());
        app.send_coupling();
        scene::draw(&app.sim, app.selected, &camera, app.grid);
        scene::draw_pointer(&app.pointer, &app.sim, &camera);
        egui_macroquad::draw();
        frame += 1;
        if frame == 3
            && let Some(path) = &screenshot
        {
            get_screen_data().export_png(path);
        }
        if smoke {
            match frame {
                5 => {
                    app.sim.set_mass(2, 5.);
                    app.sim.impulse(None, 3.);
                    app.running = true;
                }
                20 => {
                    app.reset(Preset::Stack);
                    app.running = true;
                }
                40 => {
                    app.reset(Preset::Scatter);
                    app.running = true;
                }
                60 => {
                    app.selected = app.sim.spawn(3., 1.);
                }
                80 => {
                    app.delete_selected();
                }
                100 => {
                    println!(
                        "Mass Lab graphics smoke test passed: rendered, stepped three scenes, edited mass, spawned and removed a body."
                    );
                    break;
                }
                _ => {}
            }
        }
        next_frame().await;
    }
}

#[cfg(test)]
mod app_tests {
    use super::*;
    #[test]
    fn mute_reasons_cover_paused_speed_and_unregistered() {
        let mut app = App::new();
        assert_eq!(app.force_mute_reason(), Some(MuteReason::FeedbackOff));
        app.pointer.feedback = true;
        assert_eq!(app.force_mute_reason(), Some(MuteReason::Unregistered));
        app.pointer.registered = true;
        app.pointer.connected = true;
        app.pointer.last_sample = Some(std::time::Instant::now());
        assert_eq!(app.force_mute_reason(), Some(MuteReason::NotRunning));
        app.running = true;
        app.speed = 1.004;
        assert_eq!(
            app.force_mute_reason(),
            None,
            "within half a percent is real time"
        );
        app.speed = 1.02;
        assert_eq!(app.force_mute_reason(), Some(MuteReason::NotRealTime));
        app.speed = 1.;
        app.pointer.last_sample =
            Some(std::time::Instant::now() - std::time::Duration::from_secs(1));
        assert_eq!(app.force_mute_reason(), Some(MuteReason::SceneStalled));
        app.pointer.last_sample = Some(std::time::Instant::now());
        app.pointer.worker_muted = Some(MuteReason::SceneStalled);
        assert_eq!(app.force_mute_reason(), Some(MuteReason::SceneStalled));
        app.pointer.worker_muted = None;
        assert_eq!(app.force_mute_reason(), None);
        assert!(real_time(1.) && !real_time(1.01) && !real_time(0.99));
    }

    #[test]
    fn paused_simulation_does_not_accumulate_time() {
        let mut app = App::new();
        for _ in 0..60 {
            app.advance(1. / 60.);
        }
        assert_eq!(app.sim.elapsed, 0.);
        assert_eq!(app.accumulator, 0.);
        app.running = true;
        app.advance(DT);
        assert!((app.sim.elapsed - DT).abs() < 1e-6);
    }
    #[test]
    fn fixed_steps_are_consistent_across_frame_rates() {
        let mut a = App::new();
        let mut b = App::new();
        a.running = true;
        b.running = true;
        for _ in 0..30 {
            a.advance(1. / 30.);
        }
        for _ in 0..120 {
            b.advance(1. / 120.);
        }
        assert!((a.sim.elapsed - b.sim.elapsed).abs() < 1e-5);
        let pa = a.sim.world.bodies[a.sim.blocks[0].body].translation();
        let pb = b.sim.world.bodies[b.sim.blocks[0].body].translation();
        assert!((pa - pb).length() < 1e-4);
    }
    #[test]
    fn reset_rebuilds_scene_preserves_world_settings_and_pauses() {
        let mut app = App::new();
        app.running = true;
        app.sim.gravity = 1.62;
        app.sim.bounce = 0.7;
        app.sim.spawn(20., 1.);
        app.advance(0.05);
        app.reset(Preset::Stack);
        assert_eq!(app.sim.blocks.len(), 10);
        assert_eq!(app.sim.elapsed, 0.);
        assert!(!app.running);
        assert_eq!(app.sim.gravity, 1.62);
        assert_eq!(app.sim.bounce, 0.7);
    }
}
