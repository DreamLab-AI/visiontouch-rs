//! Guided volume capture and explicitly held haptic experiments.
//!
//! UI intent is sent to an independent servo worker. Rendering, bounds persistence
//! and synthetic demonstrations never issue individual motor commands.
mod model;
mod scene;
mod sweep;
mod worker;

use egui_macroquad::egui::{self, Color32, RichText};
use haptics_effects::{Effect, Kind};
use macroquad::prelude::*;
use model::*;
use std::{collections::VecDeque, sync::atomic::Ordering, time::Instant};
use worker::{Command, Snapshot, Test, Worker};

const ACCENT: Color32 = Color32::from_rgb(118, 231, 201);
const MUTED: Color32 = Color32::from_rgb(144, 164, 185);
const AMBER: Color32 = Color32::from_rgb(248, 197, 116);
const RED: Color32 = Color32::from_rgb(248, 130, 138);
const ORDER: [usize; 6] = [2, 0, 1, 3, 4, 5];

/// Only entered coordinates are reusable across sessions. Live measurements are
/// deliberately absent: a saved file cannot establish a fresh physical pose.
#[derive(serde::Serialize, serde::Deserialize)]
struct BoundsFile {
    schema: u32,
    working_bounds_mm: [[f64; 2]; 3],
    settings: Settings,
}

struct App {
    worker: Option<Worker>,
    snapshot: Snapshot,
    received: Instant,
    volume: Volume,
    settings: Settings,
    test: Test,
    selected: usize,
    manual_bounds: bool,
    demo: bool,
    smoke: bool,
    held: bool,
    blocked_hold: bool,
    notice: String,
    filename: String,
    sound: String,
    observation: String,
    temperatures: [String; 6],
    motor_labels: [String; 5],
    sensor_note: String,
    audio_recording: String,
    orbit: scene::Orbit,
    trail: VecDeque<V>,
    forces: VecDeque<[f64; 2]>,
    show_trail: bool,
    effect: Effect,
    planner: Option<Move>,
    demo_velocity: V,
    demo_depth: f64,
}
impl App {
    fn new(demo: bool, smoke: bool, no_device: bool) -> Self {
        let worker = (!demo).then(Worker::spawn);
        if !no_device && let Some(w) = &worker {
            w.send(Command::Connect);
        }
        let mut volume = Volume::default();
        let mut snapshot = Snapshot::default();
        if demo {
            volume.measured = [
                Some(-0.05),
                Some(0.05),
                Some(0.),
                Some(0.1),
                Some(-0.05),
                Some(0.05),
            ];
            snapshot.reference = true;
            snapshot.stable = true;
            snapshot.measured = volume.measured;
            snapshot.position = [0.032, 0.05, 0.];
            snapshot.status =
                "SIMULATION ONLY · no device opened. Left drag in the view moves the virtual pen."
                    .into();
        }
        Self {
            worker,
            snapshot,
            received: Instant::now(),
            volume,
            settings: Settings::default(),
            test: Test::Object(Kind::Cube),
            selected: 2,
            manual_bounds: false,
            demo,
            smoke,
            held: false,
            blocked_hold: false,
            notice: String::new(),
            filename: "working-bounds.json".into(),
            sound: "Not recorded".into(),
            observation: String::new(),
            temperatures: std::array::from_fn(|_| String::new()),
            motor_labels: std::array::from_fn(|i| format!("Motor {}", i + 1)),
            sensor_note: "Pixel 9 Pro · Object temperature · 5 cm · material: [record setting]"
                .into(),
            audio_recording: String::new(),
            orbit: Default::default(),
            trail: VecDeque::new(),
            forces: VecDeque::new(),
            show_trail: true,
            effect: Effect::new(Kind::Cube, 0.0225),
            planner: None,
            demo_velocity: [0.; 3],
            demo_depth: 0.,
        }
    }
    fn send(&self, command: Command) {
        if let Some(w) = &self.worker {
            w.send(command);
        }
    }
    fn poll(&mut self) {
        if let Some(w) = &self.worker {
            while let Ok(s) = w.snapshots.try_recv() {
                self.volume.measured = s.measured;
                if s.sweep_mode
                    && let Some(bounds) = s.sweep.display_bounds()
                {
                    self.volume.bounds = bounds;
                }
                if self.snapshot.measured != s.measured {
                    self.selected = ORDER
                        .into_iter()
                        .find(|&i| s.measured[i].is_none())
                        .unwrap_or(self.selected);
                }
                self.snapshot = s;
                self.received = Instant::now();
            }
        }
    }
    fn ready(&self) -> bool {
        (self.demo || (self.snapshot.connected && self.received.elapsed().as_millis() < 250))
            && !self.snapshot.sweep_mode
            && self.snapshot.reference
            && self.snapshot.stable
            && self.volume.validate().is_ok()
            && self.settings.valid()
    }
    fn reset_effect(&mut self) {
        let kind = match self.test {
            Test::Zero => Kind::Cube,
            Test::Object(k) => k,
        };
        self.effect = Effect::new(kind, self.volume.radius());
        let centre = self.volume.zero().unwrap_or([0.; 3]);
        self.snapshot.frame = self
            .effect
            .step(sub(self.snapshot.position, centre), [0.; 3], 0.001);
        self.trail.clear();
    }
    fn stop(&mut self) {
        self.blocked_hold = true;
        if let Some(w) = &self.worker {
            w.controls.held.store(false, Ordering::Relaxed);
            w.send(Command::Stop);
        }
        if self.demo {
            self.snapshot.active = false;
            self.snapshot.force = [0.; 3];
            self.snapshot.requested = [0.; 3];
            self.snapshot.status =
                "Simulation stopped. Release the hold control before starting again.".into();
        }
    }
    /// A physical attempt requires a new hold edge. A fault can never restart
    /// output merely because an operator kept the same key depressed.
    fn hold(&mut self, input: bool) {
        if !input {
            self.blocked_hold = false;
        }
        let held = input && !self.blocked_hold;
        if let Some(w) = &self.worker {
            w.controls.pulse.fetch_add(1, Ordering::Relaxed);
            w.controls.held.store(held, Ordering::Relaxed);
        }
        if held && !self.held && self.ready() {
            if self.demo {
                self.snapshot.active = true;
                self.snapshot.status =
                    format!("SIMULATED {} · release hold to stop.", self.test.name());
                self.planner = Some(Move::new(
                    self.snapshot.position,
                    self.volume.zero().unwrap(),
                    self.settings,
                ));
                self.reset_effect();
            } else {
                self.send(Command::Start(
                    self.volume.clone(),
                    self.settings,
                    self.test,
                ));
            }
        }
        if !held && self.held {
            self.send(Command::Stop);
            if self.demo {
                self.snapshot.active = false;
                self.snapshot.force = [0.; 3];
                self.snapshot.requested = [0.; 3];
                self.snapshot.status = "Simulation hold released. Output off.".into();
            }
        }
        self.held = held;
    }
    fn persistence_path(&self) -> Result<std::path::PathBuf, String> {
        let name = self.filename.trim();
        if name.is_empty()
            || name.contains('/')
            || name.contains('\\')
            || !name.ends_with(".json")
            || name == ".json"
        {
            return Err(
                "Use a simple filename ending in .json, within captures/calibration/.".into(),
            );
        }
        Ok(std::path::Path::new(worker::DIRECTORY).join(name))
    }
    fn save(&mut self) {
        self.notice = (|| -> Result<String, String> {
            validate_entered(self.volume.bounds, self.settings)?;
            let path = self.persistence_path()?;
            std::fs::create_dir_all(worker::DIRECTORY).map_err(|e| e.to_string())?;
            let file = BoundsFile {
                schema: 1,
                working_bounds_mm: self.volume.bounds,
                settings: self.settings,
            };
            std::fs::write(
                &path,
                serde_json::to_vec_pretty(&file).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            Ok(format!("Saved entered coordinates: {}", path.display()))
        })()
        .unwrap_or_else(|e| format!("Save failed: {e}"));
    }
    fn load(&mut self) {
        self.notice=(|| -> Result<String,String> {
            let path=self.persistence_path()?;
            let data=std::fs::read(&path).map_err(|e|e.to_string())?;
            let file:BoundsFile=serde_json::from_slice(&data).map_err(|e|e.to_string())?;
            if file.schema!=1 {return Err("Unsupported bounds schema.".into());}
            validate_entered(file.working_bounds_mm,file.settings)?;
            self.volume.bounds=file.working_bounds_mm;
            self.settings=file.settings;
            Ok("Loaded entered coordinates. Physical reference/captures were not loaded; output remains off.".into())
        })().unwrap_or_else(|e|format!("Load failed: {e}"));
    }
    fn save_observation(&mut self) {
        self.notice=(|| -> Result<String,String> {
            if self.snapshot.active || self.held {return Err("Release the hold control before saving an observation.".into());}
            let log=self.snapshot.log.as_ref().ok_or("Run and stop an attempt before adding its observation.")?;
            let path=format!("{log}.observation.json");
            let temperatures:Vec<Option<f64>>=self.temperatures.iter().map(|value|parse_temperature(value)).collect::<Result<_,_>>()?;
            let surfaces:Vec<_>=self.motor_labels.iter().enumerate().map(|(i,label)|serde_json::json!({"operator_target_label":label,"surface_temperature_c":temperatures[i+1]})).collect();
            let observation=serde_json::json!({
                "schema":1,
                "attempt_csv":log,
                "recorded_unix_ms":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis(),
                "operator_motor_sound":self.sound,
                "sound_label":self.sound.to_ascii_lowercase().replace(' ',"_"),
                "audio_recording_wav":if self.audio_recording.trim().is_empty() {None}else{Some(self.audio_recording.trim())},
                "operator_note":self.observation,
                "ambient_temperature_c":temperatures[0],
                "surface_temperatures":surfaces,
                "sensor_target_notes":self.sensor_note,
                "source":"Operator listening and manual surface-temperature readings; not an instrumented force, winding-temperature or acoustic measurement."
            });
            std::fs::write(&path,serde_json::to_vec_pretty(&observation).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
            Ok(format!("Saved sound / surface-temperature observation: {path}"))
        })().unwrap_or_else(|error|format!("Observation save failed: {error}"));
    }
    fn simulate(&mut self, rect: Rect, frame: u32) {
        if !self.demo {
            return;
        }
        let dt = f64::from(get_frame_time()).clamp(0.001, 0.025);
        let previous = self.snapshot.position;
        let centre = self.volume.zero().unwrap();
        let r = self.volume.radius();
        if self.smoke {
            let t = frame as f64 * 0.13;
            self.snapshot.position = add(
                centre,
                [
                    r * (0.8 + 0.5 * t.cos()),
                    r * 0.3 * (t * 0.8).sin(),
                    r * 0.25 * (t * 0.6).sin(),
                ],
            );
        } else {
            let mouse = Vec2::from(mouse_position());
            if rect.contains(mouse) && is_mouse_button_down(MouseButton::Left) {
                self.snapshot.position = add(
                    centre,
                    [
                        ((mouse.x - rect.center().x) / rect.w * 0.13) as f64,
                        ((rect.center().y - mouse.y) / rect.h * 0.13) as f64,
                        self.demo_depth,
                    ],
                );
            }
            if rect.contains(mouse) && is_key_down(KeyCode::LeftShift) {
                self.demo_depth =
                    (self.demo_depth + mouse_wheel().1 as f64 * 0.001).clamp(-0.045, 0.045);
                self.snapshot.position[2] = self.demo_depth;
            }
        }
        if self.smoke
            || !matches!(self.test, Test::Zero)
            || !self.snapshot.active
            || self.snapshot.position != previous
        {
            self.demo_velocity = mul(sub(self.snapshot.position, previous), 1. / dt);
        }
        if self.snapshot.active || self.smoke {
            let requested = match self.test {
                Test::Zero => match self
                    .planner
                    .as_mut()
                    .map(|p| p.tick(self.snapshot.position, self.demo_velocity, dt))
                {
                    Some(Ok(force)) => {
                        self.snapshot.target = self.planner.as_ref().unwrap().target;
                        force
                    }
                    Some(Err(reason)) => {
                        self.snapshot.status = reason.into();
                        self.snapshot.active = false;
                        [0.; 3]
                    }
                    None => [0.; 3],
                },
                Test::Object(_) => {
                    self.snapshot.frame = self.effect.step(
                        sub(self.snapshot.position, centre),
                        self.demo_velocity,
                        dt,
                    );
                    self.snapshot.target = add(centre, self.snapshot.frame.proxy);
                    self.snapshot.frame.force
                }
            };
            self.snapshot.requested = requested;
            let limited = mul(
                limit(requested, self.settings.force_n),
                haptics_device::UNIT_DERATING,
            );
            self.snapshot.force = add(
                self.snapshot.force,
                limit(
                    sub(limited, self.snapshot.force),
                    2. * haptics_device::UNIT_DERATING * dt,
                ),
            );
            if matches!(self.test, Test::Zero) && self.snapshot.active && !self.smoke {
                self.demo_velocity = add(
                    self.demo_velocity,
                    mul(
                        sub(self.snapshot.force, mul(self.demo_velocity, 1.)),
                        dt / 0.05,
                    ),
                );
                self.snapshot.position = add(self.snapshot.position, mul(self.demo_velocity, dt));
            }
        } else if matches!(self.test, Test::Object(_)) {
            self.snapshot.frame =
                self.effect
                    .step(sub(self.snapshot.position, centre), self.demo_velocity, dt);
        }
        if !self.snapshot.active && !self.smoke {
            self.snapshot.force = [0.; 3];
            self.snapshot.requested = [0.; 3];
        }
        self.snapshot.velocity = self.demo_velocity;
        self.snapshot.samples += 1;
        self.snapshot.rate = 1. / dt;
    }
}
fn parse_temperature(value: &str) -> Result<Option<f64>, &'static str> {
    if value.trim().is_empty() {
        return Ok(None);
    }
    let temperature = value
        .trim()
        .parse::<f64>()
        .map_err(|_| "Temperature entries must be numbers in °C or left blank.")?;
    if !temperature.is_finite() {
        return Err("Temperature entries must be finite.");
    }
    Ok(Some(temperature))
}
fn validate_entered(bounds: [[f64; 2]; 3], settings: Settings) -> Result<(), String> {
    if bounds
        .iter()
        .any(|[a, b]| !a.is_finite() || !b.is_finite() || *a >= 0. || *b <= 0.)
    {
        return Err("Each entered minimum must be negative and maximum positive.".into());
    }
    if !settings.valid() {
        return Err("Settings exceed the supported speed or model-force range.".into());
    }
    Ok(())
}
fn conf() -> Conf {
    Conf {
        window_title: "Haptics Calibration Studio".into(),
        window_width: 1480,
        window_height: 960,
        high_dpi: true,
        sample_count: 4,
        platform: miniquad::conf::Platform {
            linux_wm_class: "haptics-calibration",
            swap_interval: Some(1),
            ..Default::default()
        },
        ..Default::default()
    }
}
fn theme(ctx: &egui::Context) {
    let mut s = (*ctx.style()).clone();
    s.visuals = egui::Visuals::dark();
    s.visuals.panel_fill = Color32::from_rgb(15, 23, 33);
    s.visuals.window_fill = Color32::from_rgb(23, 33, 46);
    s.visuals.extreme_bg_color = Color32::from_rgb(10, 17, 25);
    s.visuals.faint_bg_color = Color32::from_rgb(27, 39, 53);
    s.visuals.override_text_color = Some(Color32::from_rgb(224, 234, 243));
    s.visuals.selection.bg_fill = Color32::from_rgb(43, 86, 78);
    s.visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(30, 45, 58);
    s.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(45, 65, 79);
    s.spacing.item_spacing = egui::vec2(9., 8.);
    s.spacing.button_padding = egui::vec2(10., 7.);
    s.spacing.slider_width = 125.;
    s.text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(13.));
    ctx.set_style(s);
}
fn eyebrow(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(10.).strong().color(MUTED));
}
fn card(ui: &mut egui::Ui, label: &str, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(Color32::from_rgb(23, 33, 46))
        .corner_radius(8)
        .inner_margin(12)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            eyebrow(ui, label);
            ui.add_space(3.);
            content(ui);
        });
    ui.add_space(3.);
}
fn metric(ui: &mut egui::Ui, label: &str, value: impl ToString) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.monospace(value.to_string());
        });
    });
}
fn vector(v: V, scale: f64) -> String {
    format!(
        "{:+8.2} {:+8.2} {:+8.2}",
        v[0] * scale,
        v[1] * scale,
        v[2] * scale
    )
}
fn force_plot(ui: &mut egui::Ui, history: &VecDeque<[f64; 2]>, cap: f64) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 76.), egui::Sense::hover());
    let p = ui.painter().with_clip_rect(rect);
    p.rect_filled(rect, 4., Color32::from_rgb(12, 21, 31));
    let max = cap.max(0.02) * 1.2;
    let at = |i: usize, f: f64| {
        egui::pos2(
            rect.left() + i as f32 / history.len().max(1) as f32 * rect.width(),
            rect.bottom() - 4. - (f.clamp(0., max) / max) as f32 * (rect.height() - 8.),
        )
    };
    let y = at(0, cap).y;
    p.line_segment(
        [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
        egui::Stroke::new(0.7_f32, Color32::from_rgb(87, 77, 66)),
    );
    for series in 0..2 {
        let color = if series == 0 { AMBER } else { ACCENT };
        let points: Vec<_> = history
            .iter()
            .enumerate()
            .map(|(i, v)| at(i, v[series]))
            .collect();
        if points.len() > 1 {
            p.add(egui::Shape::line(points, egui::Stroke::new(1.4_f32, color)));
        }
    }
}

fn interface(app: &mut App, ctx: &egui::Context) -> (Rect, bool, bool) {
    let locked = app.held || app.snapshot.active;
    let mut button_hold = false;
    egui::TopBottomPanel::top("header")
        .exact_height(72.)
        .frame(
            egui::Frame::new()
                .fill(Color32::from_rgb(15, 23, 33))
                .inner_margin(egui::Margin::symmetric(22, 12)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("CALIBRATION STUDIO").size(21.).strong());
                    ui.label(
                        RichText::new("PHANToM 1.0A  /  capture / map / feel / measure")
                            .size(11.)
                            .color(MUTED),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(RichText::new("STOP · ESC").strong().color(RED))
                                .min_size(egui::vec2(112., 36.)),
                        )
                        .clicked()
                    {
                        app.stop();
                    }
                    let (label, color) = if app.demo {
                        ("SIMULATION · NO DEVICE", AMBER)
                    } else if app.snapshot.active {
                        ("MOTOR OUTPUT ACTIVE · HOLD", RED)
                    } else {
                        ("MOTOR OUTPUT OFF", ACCENT)
                    };
                    ui.label(RichText::new(label).strong().size(12.).color(color));
                });
            });
        });
    egui::TopBottomPanel::bottom("status")
        .min_height(51.)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(if app.demo {
                        "SIM"
                    } else if app.snapshot.connected {
                        "LINK"
                    } else {
                        "OFFLINE"
                    })
                    .strong()
                    .color(if app.snapshot.connected || app.demo {
                        ACCENT
                    } else {
                        AMBER
                    }),
                );
                ui.label(RichText::new(&app.snapshot.status).size(12.));
            });
            if !app.notice.is_empty() {
                ui.label(RichText::new(&app.notice).size(11.).color(AMBER));
            }
        });
    egui::SidePanel::left("guide").exact_width(342.).resizable(false).frame(egui::Frame::new().fill(Color32::from_rgb(15,23,33)).inner_margin(14)).show(ctx,|ui|{
        egui::ScrollArea::vertical().id_salt("guide_scroll").show(ui,|ui|{
            card(ui,"01 / CONNECT & SET THE REFERENCE",|ui|{
                if app.demo {ui.label(RichText::new("Virtual volume ready").color(ACCENT));ui.label("Demo coordinates and captures are synthetic.");}
                else {
                    ui.horizontal(|ui|{if app.snapshot.connected {
                        if ui.add_enabled(!locked,egui::Button::new("Disconnect")).clicked(){app.send(Command::Disconnect);}
                    } else if ui.add_enabled(!locked,egui::Button::new("Connect /dev/parport0")).clicked(){app.send(Command::Connect);}
                    ui.label(RichText::new(if app.snapshot.stable {"Still"} else {"Hold still"}).color(if app.snapshot.stable {ACCENT} else {AMBER}));});
                    ui.label("With output OFF, rest the pen at the very bottom of travel, centred left/right and front/back. Keep it still for 0.3 s.");
                    if ui.add_enabled(app.snapshot.connected && app.snapshot.stable && !locked,egui::Button::new(if app.snapshot.reference {"Recapture bottom reference"} else {"Capture bottom reference"}).min_size(egui::vec2(ui.available_width(),32.))).clicked(){app.send(Command::Bottom);app.selected=0;}
                    if app.snapshot.reference {ui.label(RichText::new("[ok] Bottom reference captured · Y min recorded").size(11.).color(ACCENT));}
                }
            });
            if !app.demo {
                ui.add_enabled_ui(!locked && !app.snapshot.sweeping, |ui| {ui.checkbox(&mut app.manual_bounds,"Advanced: six-bound box calibration");});
                if app.manual_bounds {ui.label(RichText::new("A box may contain unreachable corners. Recapture bottom to start a fresh mapping; sweep maps never arm output.").size(11.).color(AMBER));}
            }
            if !app.demo && !app.manual_bounds {
                card(ui,"02 / SWEEP THE COMFORTABLE WORKSPACE",|ui|{
                    ui.label("Keep motors off. Sweep left/right arcs at several heights, then repeat nearer and farther away. Follow comfortable travel; do not push mechanical stops.");
                    ui.add_enabled_ui(!locked && app.snapshot.reference,|ui|{
                        ui.horizontal(|ui|{
                            if ui.add_enabled(!app.snapshot.sweeping,egui::Button::new("Start / continue sweep")).clicked(){app.send(Command::Sweep(true));}
                            if ui.add_enabled(app.snapshot.sweeping,egui::Button::new("Finish sweep")).clicked(){app.send(Command::Sweep(false));}
                        });
                    });
                    ui.label(RichText::new(if app.snapshot.sweeping {"RECORDING · MOTOR OUTPUT OFF"} else {"Motor-off observations"}).color(ACCENT));
                    ui.label(format!("{} sampled cells · 2 model mm spacing", app.snapshot.sweep.points_model_m.len()));
                    let d=app.snapshot.sweep.dimensions_mm();
                    ui.label(format!("Observed span: {:.1} × {:.1} × {:.1} model mm", d[0],d[1],d[2]));
                    ui.label(RichText::new("Dots are measured positions. The surrounding box is a display extent, not a reachable workspace. Gaps remain unknown.").color(AMBER));
                });
                card(ui,"03 / CHOOSE ZERO & SAVE THE SHAPE",|ui|{
                    ui.label("After sweeping, move manually to a comfortable central pose. Hold still, then record this as your working zero.");
                    if ui.add_enabled(!locked && !app.snapshot.sweeping && app.snapshot.stable && app.snapshot.sweep_mode,egui::Button::new("Record this pose as zero")).clicked(){app.send(Command::SweepZero);}
                    if app.snapshot.sweep.zero_model_m.is_some(){ui.label(RichText::new("[ok] Manually observed zero recorded").color(ACCENT));}
                    if ui.add_enabled(!locked && app.snapshot.connected && !app.snapshot.sweeping && app.snapshot.stable && app.snapshot.sweep.zero_model_m.is_some(), egui::Button::new("Review straight path from this pose")).clicked(){app.send(Command::ReviewSweepPath);}
                    if let Some(review)=&app.snapshot.sweep.path_review {
                        let supported=review.first_gap_fraction.is_none();
                        ui.label(RichText::new(if supported {"Path lies near observed samples"} else {"Path crosses an unsampled gap · shown red"}).color(if supported {ACCENT}else{AMBER}));
                        ui.label(RichText::new("5 mm sample support minus 2 mm evidence margin. Proximity is not physical clearance; this review does not enable force.").size(10.).color(MUTED));
                    }
                    if ui.add_enabled(!locked && !app.snapshot.sweeping && !app.snapshot.sweep.points_model_m.is_empty(),egui::Button::new("Export sweep evidence")).clicked(){app.send(Command::SaveCalibration(app.volume.clone()));}
                    ui.label("Coordinates use model millimetres about the chosen zero. Until then, the display centre is provisional.");
                    ui.label(RichText::new("Powered tests remain locked for sweep maps until the usable region and approach path are validated.").color(AMBER));
                });
            } else {
            card(ui,"02 / ENTER YOUR WORKING COORDINATES",|ui|{
                ui.label("Choose the coordinate labels at each limit, in mm. Zero must lie inside all three axes.");
                ui.add_enabled_ui(!locked,|ui|{
                    egui::Grid::new("bounds").num_columns(3).spacing([8.,7.]).show(ui,|ui|{
                        ui.label("Axis");ui.label(RichText::new("Minimum").color(MUTED));ui.label(RichText::new("Maximum").color(MUTED));ui.end_row();
                        for (i,label) in ["X · left/right","Y · bottom/top","Z · back/front"].into_iter().enumerate(){ui.label(label);for j in 0..2 {ui.add(egui::DragValue::new(&mut app.volume.bounds[i][j]).speed(1.).suffix(" mm").range(-10000.0..=10000.0));}ui.end_row();}
                    });
                    ui.collapsing("Save / load entered bounds",|ui|{
                        ui.label(RichText::new("captures/calibration/").size(11.).color(MUTED));ui.text_edit_singleline(&mut app.filename);
                        ui.horizontal(|ui|{if ui.button("Save bounds").clicked(){app.save();}
                        if ui.button("Load bounds").clicked(){app.load();}});
                        ui.label(RichText::new("Files store entered bounds + settings. Live physical captures must come from this connection.").size(11.).color(MUTED));
                    });
                });
                ui.label(RichText::new("±100 defaults are example coordinates, not device dimensions.").size(11.).color(AMBER));
            });
            card(ui,"03 / RECORD SIX COMFORTABLE LIMITS",|ui|{
                if ui.add_enabled(!locked && !app.demo && app.snapshot.reference && app.volume.validate().is_ok(), egui::Button::new("Export calibration evidence")).clicked() {
                    app.send(Command::SaveCalibration(app.volume.clone()));
                }
                let captured=app.volume.measured.iter().filter(|x|x.is_some()).count();
                ui.add(egui::ProgressBar::new(captured as f32/6.).text(format!("{captured} / 6 {} limits recorded",if app.demo {"synthetic"}else{"physical"})));
                ui.add_enabled_ui(!locked && !app.demo,|ui|{
                    egui::Grid::new("capture_list").num_columns(2).spacing([8.,3.]).show(ui,|ui|{
                        for i in ORDER {
                            let mark=if app.volume.measured[i].is_some(){"+"}else{"-"};
                            if ui.selectable_label(app.selected==i,format!("{mark} {}",NAMES[i])).clicked(){app.selected=i;}
                            ui.monospace(app.volume.measured[i].map(|v|format!("{v:+.4} m")).unwrap_or("—".into()));ui.end_row();
                        }
                    });
                });
                ui.add_space(4.);ui.label(RichText::new(if app.demo {"Synthetic calibration complete. Select a demonstration and hold Space to simulate its response."} else {INSTRUCTIONS[app.selected]}).color(if app.snapshot.reference {Color32::WHITE}else{MUTED}));
                if ui.add_enabled(app.snapshot.reference&&app.snapshot.stable&&!locked&&!app.demo,egui::Button::new(format!("Record {}",NAMES[app.selected])).min_size(egui::vec2(ui.available_width(),32.))).clicked(){app.send(Command::Capture(app.selected));}
                match app.volume.validate(){Ok(())=>{ui.label(RichText::new("[ok] Complete mapping · working zero is defined").color(ACCENT));},Err(reason)=>{ui.label(RichText::new(reason).size(11.).color(AMBER));}}
                ui.label(RichText::new("Do not force mechanical stops. Recorded values use model metres relative to bottom.").size(11.).color(MUTED));
            });
            }
        });
    });
    egui::SidePanel::right("experiments").exact_width(316.).resizable(false).frame(egui::Frame::new().fill(Color32::from_rgb(15,23,33)).inner_margin(14)).show(ctx,|ui|{
        egui::ScrollArea::vertical().id_salt("experiment_scroll").show(ui,|ui|{
            card(ui,"04 / CHOOSE A HELD EXPERIMENT",|ui|{
                let old=app.test;
                ui.add_enabled_ui(!locked,|ui|{
                    egui::ComboBox::from_id_salt("test").selected_text(app.test.name()).width(ui.available_width()-8.).show_ui(ui,|ui|{
                        ui.selectable_value(&mut app.test,Test::Zero,"Slow move to (0, 0, 0)");
                        for kind in Kind::ALL {ui.selectable_value(&mut app.test,Test::Object(kind),kind.name());}
                    });
                });
                if old!=app.test{app.reset_effect();}
                ui.label(match app.test{Test::Zero=>"Start still inside the mapped volume. Hold to move slowly to zero; apply gentle resistance to observe the bounded response and stall stop.",Test::Object(k)=>k.instructions()});
                ui.add_enabled_ui(!locked,|ui|{
                    if matches!(app.test,Test::Zero){ui.add(egui::Slider::new(&mut app.settings.speed_mm_s,0.5..=5.).text("mm/s").fixed_decimals(1));}
                    ui.add(egui::Slider::new(&mut app.settings.force_n,0.02..=haptics_device::MAX_FORCE_N).text("N cap").fixed_decimals(2));
                });
                let ready=app.ready();
                let label=if app.snapshot.active {"HOLDING · RELEASE TO STOP"}else if app.demo {"HOLD TO SIMULATE · SPACE"}else {"HOLD TO ENABLE · SPACE"};
                let response=ui.add_enabled(ready||app.held,egui::Button::new(RichText::new(label).strong().color(Color32::from_rgb(12,34,30))).fill(if app.snapshot.active {AMBER}else{ACCENT}).sense(egui::Sense::click_and_drag()).min_size(egui::vec2(ui.available_width(),41.)));
                button_hold=response.is_pointer_button_down_on();
                if !ready {ui.label(RichText::new(if app.snapshot.sweep_mode {"Input-only sweep: powered tests locked pending region and path validation."} else {"Requires live samples, stable pen and all six valid captures."}).size(11.).color(AMBER));}
                ui.label(RichText::new("Release / Escape / Stop / pen button: output off. Each hold starts a fresh attempt.").size(11.).color(MUTED));
                ui.horizontal(|ui|{
                    if ui.add_enabled(!locked,egui::Button::new("Reset scene")).clicked(){app.reset_effect();app.notice="Scene state resets at the next held attempt; simulation preview reset.".into();}
                    ui.checkbox(&mut app.show_trail,"Pen trail");
                });
            });
            card(ui,"MODEL FORCE · NOT MEASURED FORCE",|ui|{
                metric(ui,"Requested",format!("{:.3} N",norm(app.snapshot.requested)));
                metric(ui,"Guarded",format!("{:.3} N",norm(app.snapshot.force)));
                force_plot(ui,&app.forces,app.settings.force_n);
                ui.horizontal(|ui|{ui.label(RichText::new("— requested").size(10.).color(AMBER));ui.label(RichText::new("— guarded").size(10.).color(ACCENT));});
                ui.monospace(format!("Motor offsets: {:?}",app.snapshot.offsets));
                metric(ui,"Unit gain",format!("{:.0}%",100.*haptics_device::UNIT_DERATING));
                metric(ui,"Effective selected cap",format!("{:.3} model N",app.settings.force_n*haptics_device::UNIT_DERATING));
                ui.label(RichText::new(format!("SDK envelope: {:.2} N · {:.1} N/s · ±{} counts\n25 ms expiry · 250 ms UI heartbeat\nLatched release after 250 ms at command limit",haptics_device::MAX_FORCE_N*haptics_device::UNIT_DERATING,2.*haptics_device::UNIT_DERATING,(f64::from(haptics_device::MAX_DAC_OFFSET)*haptics_device::UNIT_DERATING).floor() as i16)).size(11.).color(MUTED));
                if let Test::Object(Kind::Balloon)=app.test {metric(ui,"Indentation",format!("{:.0}%",app.snapshot.frame.deformation*100.));ui.label(RichText::new(if app.snapshot.frame.popped {"POPPED · release and hold to reinflate"}else{"Intact · pops at 35% indentation"}).color(if app.snapshot.frame.popped {AMBER}else{ACCENT}));}
            });
            card(ui,"LIVE TELEMETRY",|ui|{
                let s=&app.snapshot;
                metric(ui,if app.demo{"Simulation rate"}else{"Device sample rate"},format!("{:.0} Hz",s.rate));
                metric(ui,"Samples",s.samples);
                ui.label(RichText::new("Working X / Y / Z · mm").size(10.).color(MUTED));
                ui.monospace(app.volume.working(s.position).map(|v|vector(v,1.)).unwrap_or("Capture all bounds to define working coordinates".into()));
                ui.label(RichText::new("Bottom-reference model X / Y / Z · mm").size(10.).color(MUTED));ui.monospace(vector(s.position,1000.));
                metric(ui,"Model speed",format!("{:.2} mm/s",norm(s.velocity)*1000.));
                egui::CollapsingHeader::new("All six encoder channels").default_open(true).show(ui,|ui|{
                    egui::Grid::new("encoders").num_columns(3).spacing([10.,3.]).show(ui,|ui|{
                        ui.label("Channel");ui.label("Raw");ui.label("Unwrapped Δ");ui.end_row();
                        for i in 0..6{ui.label(format!("{} {}",i,if i<3 {"arm"}else{"wrist"}));ui.monospace(s.counts[i].to_string());ui.monospace(s.accumulated[i].to_string());ui.end_row();}
                    });
                    if app.demo {ui.label(RichText::new("No encoder samples in simulation.").size(10.).color(AMBER));}
                });
                if let Some(log)=&s.log {ui.label(RichText::new("LAST ATTEMPT CSV").size(10.).color(ACCENT));ui.monospace(log);}
                ui.label(RichText::new("Every live attempt saves CSV + JSON after output is disabled.").size(11.).color(MUTED));
            });
            card(ui,"OPERATOR / SOUND & SURFACE TEMPERATURE",|ui|{
                ui.label("Listen during a held attempt; record your impression after releasing output.");
                ui.add_enabled_ui(!locked,|ui|{
                    egui::ComboBox::from_id_salt("sound_observation").selected_text(&app.sound).width(ui.available_width()-8.).show_ui(ui,|ui|{
                        for sound in ["Not recorded","Quiet","Normal","Laboured","Chatter","Other"] {ui.selectable_value(&mut app.sound,sound.to_string(),sound);}
                    });
                    ui.add(egui::TextEdit::multiline(&mut app.observation).hint_text("Optional notes: direction, location, resistance, sound…").desired_rows(2).desired_width(ui.available_width()));
                    ui.label(RichText::new("Audio recording (optional WAV)").size(11.).color(MUTED));
                    ui.add(egui::TextEdit::singleline(&mut app.audio_recording).hint_text("Path to saved WAV; no automatic recording").desired_width(ui.available_width()));
                    ui.collapsing("Optional optical temperature readings · °C",|ui|{
                        ui.label(RichText::new("Leave missing readings blank. Label each measured surface; motor roles are not assumed.").size(11.).color(MUTED));
                        ui.horizontal(|ui|{ui.label("Ambient °C");ui.add(egui::TextEdit::singleline(&mut app.temperatures[0]).hint_text("unset").desired_width(72.));});
                        egui::Grid::new("surface_temperatures").num_columns(2).spacing([6.,5.]).show(ui,|ui|{
                            ui.label("Measured target");ui.label("Surface °C");ui.end_row();
                            for i in 0..5 {ui.add(egui::TextEdit::singleline(&mut app.motor_labels[i]).desired_width(140.));ui.add(egui::TextEdit::singleline(&mut app.temperatures[i+1]).hint_text("unset").desired_width(70.));ui.end_row();}
                        });
                        ui.add(egui::TextEdit::multiline(&mut app.sensor_note).hint_text("Sensor / surface / optical settings / measurement time notes").desired_rows(2).desired_width(ui.available_width()));
                    });
                    if ui.add_enabled(app.snapshot.log.is_some(),egui::Button::new("Save observation with last attempt")).clicked(){app.save_observation();}
                });
                ui.label(RichText::new("Manual observations; surface temperature does not measure winding temperature. No temperature thresholds are assumed.").size(10.).color(MUTED));
            });

        });
    });
    let r = ctx.available_rect();
    let rect = Rect::new(r.min.x, r.min.y, r.width().max(1.), r.height().max(1.));
    let p = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("viewport_overlay"),
    ));
    let origin = egui::pos2(rect.x + 24., rect.y + 24.);
    p.text(
        origin,
        egui::Align2::LEFT_TOP,
        if app.snapshot.sweep_mode {
            "Observed workspace sweep"
        } else {
            app.test.name()
        },
        egui::FontId::proportional(25.),
        Color32::from_rgb(231, 242, 248),
    );
    p.text(
        origin + egui::vec2(0., 35.),
        egui::Align2::LEFT_TOP,
        if app.demo {
            "SIMULATED WORKING VOLUME"
        } else if app.snapshot.sweep_mode {
            "OBSERVED POSITIONS · INTERIOR AND PATHS UNVERIFIED"
        } else if app.volume.validate().is_ok() {
            "WORKING COORDINATE MAP · BOX APPROXIMATION"
        } else {
            "PREVIEW · PHYSICAL VOLUME NOT YET MAPPED"
        },
        egui::FontId::monospace(10.),
        MUTED,
    );
    let legend = if app.demo {
        "Left drag: virtual pen · Shift + wheel: depth\nRight drag: orbit · wheel: zoom · Home: reset view"
    } else {
        "Right drag: orbit · wheel: zoom · Home: reset view\nMint: measured pen · gold: target / contact proxy"
    };
    p.text(
        egui::pos2(rect.x + 24., rect.bottom() - 26.),
        egui::Align2::LEFT_BOTTOM,
        legend,
        egui::FontId::proportional(11.),
        MUTED,
    );
    let camera = app.orbit.camera(rect);
    for (axis, label, color) in [
        (Vec3::X, "+X · RIGHT", Color32::from_rgb(234, 113, 126)),
        (Vec3::Y, "+Y · UP", Color32::from_rgb(125, 220, 160)),
        (Vec3::Z, "+Z · TOWARD YOU", Color32::from_rgb(113, 166, 247)),
    ] {
        if let Some(pos) = scene::project(&camera, rect, axis * 4.25) {
            p.text(
                egui::pos2(pos.x, pos.y),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::monospace(10.),
                color,
            );
        }
    }
    if let Some(pos) = scene::project(&camera, rect, Vec3::ZERO) {
        p.text(
            egui::pos2(pos.x + 9., pos.y + 9.),
            egui::Align2::LEFT_TOP,
            "0, 0, 0",
            egui::FontId::monospace(11.),
            MUTED,
        );
    }
    (rect, button_hold, ctx.wants_keyboard_input())
}

#[macroquad::main(conf)]
async fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "Calibration Studio\n  --demo        Mouse-driven simulation; never opens hardware\n  --sweep-smoke Render an input-only sweep preview and exit; no hardware\n  --smoke-test  Exercise all effects and rendering, then exit; no hardware\n  --no-device   Start offline; connect only through the Connect button\nDefault: connect with motor output off. Holds explicitly enable attempts."
        );
        return;
    }
    if let Err(error) = haptics_device::shutdown::install() {
        eprintln!("Cannot install shutdown handler: {error}");
        return;
    }
    prevent_quit();
    let smoke = args.iter().any(|a| a == "--smoke-test");
    let sweep_smoke = args.iter().any(|a| a == "--sweep-smoke");
    let demo = smoke || sweep_smoke || args.iter().any(|a| a == "--demo");
    let mut app = App::new(demo, smoke, args.iter().any(|a| a == "--no-device"));
    if sweep_smoke {
        app.demo = false;
        app.snapshot.sweep_mode = true;
        let sweep = std::sync::Arc::make_mut(&mut app.snapshot.sweep);
        for i in 0..60 {
            for j in 0..20 {
                let a = i as f64 * std::f64::consts::TAU / 60.;
                let b = j as f64 * std::f64::consts::PI / 20.;
                sweep.record([
                    0.08 * a.cos() * b.sin(),
                    0.06 * (1. + b.cos()),
                    0.045 * a.sin() * b.sin(),
                ]);
            }
        }
        sweep.choose_zero([0., 0.06, 0.]).unwrap();
        sweep.review_path([0., 0., 0.]).unwrap();
        assert!(
            sweep
                .path_review
                .as_ref()
                .unwrap()
                .first_gap_fraction
                .is_some()
        );
        app.volume.measured = sweep.extrema().unwrap();
        app.volume.bounds = sweep.display_bounds().unwrap();
        app.snapshot.measured = app.volume.measured;
        app.snapshot.status = "OFFLINE SWEEP PREVIEW · synthetic shape · no device opened.".into();
        assert!(!app.ready());
    }
    let screenshot = std::env::var("CALIBRATION_SCREENSHOT").ok();
    let mut previous = Vec2::from(mouse_position());
    let mut frame = 0_u32;
    let mut scenes = [false; 7];
    loop {
        if is_quit_requested() || haptics_device::shutdown::requested() {
            break;
        }
        app.poll();
        if smoke {
            if frame == 0 {
                let mut incomplete = app.volume.clone();
                incomplete.measured[0] = None;
                assert!(incomplete.validate().is_err());
                assert_eq!(
                    app.volume.working(app.volume.zero().unwrap()),
                    Some([0.; 3])
                );
                assert!(app.worker.is_none());
            }
            if frame.is_multiple_of(12) && frame / 12 < 7 {
                let i = (frame / 12) as usize;
                app.test = Test::Object(Kind::ALL[i]);
                app.reset_effect();
                scenes[i] = true;
            }
            if frame == 88 {
                app.test = Test::Zero;
                app.planner = Some(Move::new(
                    app.snapshot.position,
                    app.volume.zero().unwrap(),
                    app.settings,
                ));
            }
            if frame == 96 {
                app.test = Test::Object(Kind::Springs);
                app.reset_effect();
                app.snapshot.status =
                    "SIMULATION ONLY · all seven effects exercised · no hardware opened.".into();
            }
        }
        clear_background(Color::from_rgba(10, 18, 27, 255));
        let mut rect = Rect::new(
            342.,
            72.,
            (screen_width() - 658.).max(1.),
            (screen_height() - 123.).max(1.),
        );
        let mut button = false;
        let mut keyboard = false;
        egui_macroquad::ui(|ctx| {
            if frame == 0 {
                theme(ctx);
            }
            (rect, button, keyboard) = interface(&mut app, ctx);
        });
        let input = button || (!keyboard && is_key_down(KeyCode::Space));
        if is_key_pressed(KeyCode::Escape) {
            app.stop();
        }
        if is_key_pressed(KeyCode::Home) && !keyboard {
            app.orbit = Default::default();
        }
        app.hold(input);
        app.orbit.input(rect, previous);
        previous = Vec2::from(mouse_position());
        app.simulate(rect, frame);
        if smoke && matches!(app.test, Test::Object(Kind::Springs)) {
            assert!(
                norm(sub(
                    app.snapshot.frame.bodies[0],
                    app.snapshot.frame.bodies[1]
                )) > 0.001,
                "three-body preview must advance a valid effect frame"
            );
        }
        if app.snapshot.reference {
            app.trail.push_back(app.snapshot.position);
            if app.trail.len() > 400 {
                app.trail.pop_front();
            }
        }
        app.forces
            .push_back([norm(app.snapshot.requested), norm(app.snapshot.force)]);
        if app.forces.len() > 240 {
            app.forces.pop_front();
        }
        scene::draw(
            &app.volume,
            &app.snapshot,
            app.test,
            &app.trail,
            &app.orbit.camera(rect),
            app.show_trail,
        );
        egui_macroquad::draw();
        if frame == 104
            && let Some(path) = &screenshot
        {
            get_screen_data().export_png(path);
        }
        if sweep_smoke && frame >= 112 {
            assert!(app.worker.is_none());
            assert!(!app.ready());
            println!(
                "Sweep graphics smoke passed: cloud, bounds, chosen zero, input-only UI and force lock; no device opened."
            );
            break;
        }
        if smoke && frame >= 112 {
            assert!(scenes.into_iter().all(|x| x));
            assert!(app.worker.is_none());
            println!(
                "Calibration Studio graphics smoke passed: seven effects, zero planner, mapping, missing-bound rejection, telemetry, view and disabled hardware controls; no device opened."
            );
            break;
        }
        frame += 1;
        next_frame().await;
    }
    app.stop();
    drop(app); // Worker::drop clears hold, sends Quit, disarms and joins.
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    #[test]
    fn manual_temperature_values_preserve_missing_readings_and_reject_nonfinite_data() {
        assert_eq!(parse_temperature(" "), Ok(None));
        assert_eq!(parse_temperature(" 22.5 "), Ok(Some(22.5)));
        assert_eq!(parse_temperature("0"), Ok(Some(0.)));
        for invalid in ["NaN", "inf", "twenty"] {
            assert!(parse_temperature(invalid).is_err());
        }
    }
    #[test]
    fn saved_bounds_never_include_live_measurements() {
        let file = BoundsFile {
            schema: 1,
            working_bounds_mm: [[-100., 100.]; 3],
            settings: Settings::default(),
        };
        let json = serde_json::to_string(&file).unwrap();
        assert!(!json.contains("measured"));
        assert!(!json.contains("reference"));
        let loaded: BoundsFile = serde_json::from_str(&json).unwrap();
        assert!(validate_entered(loaded.working_bounds_mm, loaded.settings).is_ok());
        assert!(validate_entered([[1., 100.]; 3], loaded.settings).is_err());
    }
}
