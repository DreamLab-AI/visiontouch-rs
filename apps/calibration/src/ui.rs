//! Interface panels. Everything here reads a worker `Snapshot` and sends `Command`s; it
//! never touches the device.
use crate::App;
use crate::protocol::{Axis, Mark, Reading, Rung, Sign, StepKind, StepRequest, Unit, Verdict};
use crate::scene;
use crate::worker::{Command, EntryKind};
use egui_macroquad::egui::{self, Color32, RichText};
use macroquad::prelude::{Rect, Vec3};
use visiontouch_effects::Kind;
use visiontouch_effects::math::norm;

const ACCENT: Color32 = Color32::from_rgb(118, 231, 201);
const MUTED: Color32 = Color32::from_rgb(144, 164, 185);
const AMBER: Color32 = Color32::from_rgb(248, 197, 116);
const RED: Color32 = Color32::from_rgb(248, 130, 138);
const PANEL: Color32 = Color32::from_rgb(15, 23, 33);

/// Text-field and selector state of the interface.
pub struct Forms {
    pub reading_label: String,
    pub reading_value: String,
    pub reading_unit: usize,
    pub felt: bool,
    pub heard: bool,
    pub note: String,
    pub attempt: String,
    pub ruler_mm: [String; 3],
    pub axis: usize,
    pub sign: usize,
    pub rung: usize,
    pub effect: usize,
    pub mass: usize,
    pub seen_attempt: u64,
}
impl Default for Forms {
    fn default() -> Self {
        Self {
            reading_label: "dwell".into(),
            reading_value: String::new(),
            reading_unit: 0,
            felt: false,
            heard: false,
            note: String::new(),
            attempt: String::new(),
            ruler_mm: std::array::from_fn(|_| "50".into()),
            axis: 1,
            sign: 1,
            rung: 0,
            effect: 0,
            mass: 0,
            seen_attempt: 0,
        }
    }
}

/// What the frame needs back from the interface.
pub struct Frame {
    pub rect: Rect,
    pub hold_button: bool,
    pub keyboard: bool,
}

const UNITS: [(Unit, &str); 4] = [
    (Unit::N, "N"),
    (Unit::G, "g"),
    (Unit::C, "°C"),
    (Unit::Mm, "mm"),
];
const MASSES: [(Option<f64>, &str); 3] =
    [(None, "Bare"), (Some(50.), "50 g"), (Some(100.), "100 g")];

pub fn theme(ctx: &egui::Context) {
    let mut s = (*ctx.style()).clone();
    s.visuals = egui::Visuals::dark();
    s.visuals.panel_fill = PANEL;
    s.visuals.window_fill = Color32::from_rgb(23, 33, 46);
    s.visuals.extreme_bg_color = Color32::from_rgb(10, 17, 25);
    s.visuals.faint_bg_color = Color32::from_rgb(27, 39, 53);
    s.visuals.override_text_color = Some(Color32::from_rgb(224, 234, 243));
    s.visuals.selection.bg_fill = Color32::from_rgb(43, 86, 78);
    s.visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(30, 45, 58);
    s.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(45, 65, 79);
    s.spacing.item_spacing = egui::vec2(8., 7.);
    s.spacing.button_padding = egui::vec2(9., 6.);
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
        .inner_margin(11)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            eyebrow(ui, label);
            ui.add_space(2.);
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
fn tri(v: Option<bool>) -> RichText {
    match v {
        Some(true) => RichText::new("pass").color(ACCENT),
        Some(false) => RichText::new("fail").color(RED),
        None => RichText::new("not yet").color(MUTED),
    }
}
fn mm3(v: [f64; 3]) -> String {
    format!(
        "{:+8.2} {:+8.2} {:+8.2}",
        v[0] * 1e3,
        v[1] * 1e3,
        v[2] * 1e3
    )
}
fn parse(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|x| x.is_finite())
}

/// Start button with an inline reason when the step is locked.
fn start_button(app: &mut App, ui: &mut egui::Ui, req: StepRequest, text: &str) {
    let hint = app.snapshot.session.check(req).err();
    let busy = app.snapshot.active.is_some() || app.pending.is_some();
    let label = if hint.is_some() {
        RichText::new(text).color(MUTED)
    } else {
        RichText::new(text)
    };
    let r = ui.add_enabled(!busy && app.snapshot.connected, egui::Button::new(label));
    let r = match &hint {
        Some(h) => r.on_hover_text(h),
        None => r,
    };
    if r.clicked() {
        app.request(req);
    }
}
fn hint_line(app: &App, ui: &mut egui::Ui, req: StepRequest) {
    if let Err(h) = app.snapshot.session.check(req) {
        ui.label(RichText::new(h).size(11.).color(AMBER));
    }
}

fn left_panel(app: &mut App, ctx: &egui::Context) {
    egui::SidePanel::left("guide")
        .exact_width(352.)
        .resizable(false)
        .frame(egui::Frame::new().fill(PANEL).inner_margin(12))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().id_salt("guide").show(ui, |ui| {
                let busy = app.snapshot.active.is_some();
                card(ui, "01 / CONNECT", |ui| {
                    ui.horizontal(|ui| {
                        if app.snapshot.connected {
                            if ui.add_enabled(!busy, egui::Button::new("Disconnect")).clicked() {
                                app.send(Command::Disconnect);
                            }
                        } else if ui.button(if app.is_demo() { "Connect simulation" } else { "Connect with output off" }).clicked() {
                            app.send(Command::Connect);
                        }
                        let (t, c) = if app.snapshot.stable { ("still", ACCENT) } else { ("moving", AMBER) };
                        ui.label(RichText::new(t).color(c));
                    });
                    ui.label(RichText::new("Power on with the arm supported. Expect about 420 Hz. Stop if the rate is under 300 Hz or an interval exceeds 25 ms.").size(11.).color(MUTED));
                });
                card(ui, "02 / BACKDRIVE (MOTORS OFF)", |ui| {
                    ui.label("Pull the pen slowly with the gauge along X, Y and Z. Enter each reading below. Above 0.2 N, inspect the cables.");
                    start_button(app, ui, StepRequest::Backdrive, "Start backdrive record");
                    if app.snapshot.active == Some(StepRequest::Backdrive) && ui.button("Finish").clicked() {
                        app.send(Command::Stop);
                    }
                });
                card(ui, "03 / LIVENESS (80 COUNTS, 3 HZ, 2 S)", |ui| {
                    ui.label("Fingertips on the pen near the reset pose, hold the key. Note whether you feel or hear it.");
                    for c in 0..3 {
                        ui.horizontal(|ui| {
                            start_button(app, ui, StepRequest::Liveness { channel: c }, &format!("Drive channel {c}"));
                            ui.label(tri(app.snapshot.session.liveness[c]));
                        });
                    }
                });
                card(ui, "04 / REGISTER THE RESET POSE", |ui| {
                    ui.label("Arm level and square to the base front, far arm plumb. Hold still, then register.");
                    ui.label("Reset means this physical arm pose, not the middle of the working volume or the desk rest position.");
                    ui.horizontal(|ui| {
                        if ui.add_enabled(app.snapshot.connected && !busy, egui::Button::new("Register reset pose")).clicked() {
                            app.send(Command::RegisterReset);
                        }
                        ui.label(tri(Some(app.snapshot.registered).filter(|r| *r)));
                    });
                    let reg = app.snapshot.registered && !busy;
                    if let Some(p) = app.snapshot.position {
                        ui.label(RichText::new("RESET REFERENCE: white box in the 3D view").strong());
                        metric(ui, "distance to reset (model)", format!("{:.1} mm", norm(p) * 1e3));
                        metric(ui, "offset X / Y / Z mm", mm3(p));
                        ui.label("Move away, then reproduce the physical reset pose before capturing. Do not steer to the screen marker to pass the repeatability check.");
                    }
                    ui.horizontal(|ui| {
                        if ui.add_enabled(reg, egui::Button::new("Capture return to reset")).clicked() {
                            app.send(Command::CaptureStill(Mark::ReturnToReset));
                        }
                        let c = &app.snapshot.session.checks;
                        ui.label(match (c.return_error_mm, c.return_ok()) {
                            (Some(e), Some(ok)) => RichText::new(format!("{e:.2} mm")).color(if ok { ACCENT } else { RED }),
                            _ => RichText::new("move away, return").color(MUTED),
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.label("Gravity drop");
                        ui.label(tri(app.snapshot.session.checks.gravity));
                        if ui.add_enabled(reg && output_idle(&app.snapshot), egui::Button::new("Arm gravity drop test")).clicked() {
                            app.send(Command::ArmGravityDrop);
                        }
                        if app.snapshot.gravity_armed {
                            ui.label(RichText::new("armed").color(ACCENT));
                        }
                    });
                    ui.label(RichText::new("Press arm, hold the pen mid-height and still, then let it fall onto your palm 2 cm below. Only an armed drop counts; a fail stays until you re-register.").size(11.).color(MUTED));
                    ui.label("Ruler: mark start, slide along the square in the +axis direction, mark end, enter millimetres.");
                    for a in Axis::ALL {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(a.name()).strong());
                            if ui.add_enabled(reg, egui::Button::new("Start")).clicked() {
                                app.send(Command::CaptureStill(Mark::RulerStart));
                            }
                            if ui.add_enabled(reg, egui::Button::new("End")).clicked() {
                                app.send(Command::CaptureStill(Mark::RulerEnd));
                            }
                            ui.add(egui::TextEdit::singleline(&mut app.forms.ruler_mm[a.index()]).desired_width(46.));
                            ui.label("mm");
                            if ui.add_enabled(reg, egui::Button::new("Score")).clicked() {
                                match parse(&app.forms.ruler_mm[a.index()]) {
                                    Some(mm) => app.send(Command::SubmitRuler { axis: a, measured_mm: mm }),
                                    None => app.notice = "Enter the ruler distance as a number of millimetres.".into(),
                                }
                            }
                            if let Some(r) = app.snapshot.session.checks.ruler[a.index()] {
                                ui.label(RichText::new(format!("x{:.2}", r.scale)).color(if r.ok() { ACCENT } else { RED }));
                            }
                        });
                    }
                    ui.label(RichText::new("Note which physical direction is +X and +Z; the view shows them.").size(11.).color(MUTED));
                });
                card(ui, "05 / SIGN PULSES (±0.25 N)", |ui| {
                    ui.label("Float the pen on a fingertip and hold the key. The pulse pair is scored from the motion.");
                    for a in Axis::ALL {
                        ui.horizontal(|ui| {
                            start_button(app, ui, StepRequest::SignPulse { axis: a }, &format!("Pulse {}", a.name()));
                            ui.label(tri(app.snapshot.session.sign[a.index()]));
                        });
                    }
                    hint_line(app, ui, StepRequest::SignPulse { axis: Axis::X });
                });
                card(ui, "06 / FORCE LADDER", |ui| {
                    ui.label("Y: press the pen down on the scale (tare at zero, -Y only). X and Z: gauge against the post. Read the dwell value, enter it after release.");
                    ui.horizontal(|ui| {
                        combo(ui, "axis", &mut app.forms.axis, &["X", "Y", "Z"]);
                        combo(ui, "sign", &mut app.forms.sign, &["+", "-"]);
                        combo(ui, "rung", &mut app.forms.rung, &["0.1 N", "0.25 N", "0.5 N", "1.0 N", "2.0 N"]);
                    });
                    let req = StepRequest::Ladder {
                        axis: Axis::ALL[app.forms.axis],
                        sign: Sign::ALL[app.forms.sign],
                        rung: Rung::ALL[app.forms.rung],
                    };
                    start_button(app, ui, req, &format!("Start {}", req.label()));
                    hint_line(app, ui, req);
                    egui::Grid::new("rungs").spacing([10., 2.]).show(ui, |ui| {
                        ui.label("");
                        for a in Axis::ALL {
                            ui.label(RichText::new(a.name()).color(MUTED));
                        }
                        ui.end_row();
                        for r in Rung::ALL {
                            ui.label(RichText::new(r.name()).color(MUTED));
                            for a in Axis::ALL {
                                let cell: Vec<String> = Sign::ALL
                                    .iter()
                                    .filter_map(|s| {
                                        app.snapshot.session.rungs[r.index()][a.index()][s.index()]
                                            .map(|v| format!("{}{}", s.name(), match v { Verdict::Pass => "P", Verdict::Over => "O", Verdict::Under => "U", Verdict::None => "-" }))
                                    })
                                    .collect();
                                ui.label(RichText::new(if cell.is_empty() { "-".into() } else { cell.join(" ") }).monospace());
                            }
                            ui.end_row();
                        }
                    });
                    ui.label(RichText::new("P pass, O over (blocks the next rung on that axis), U under (recorded, not blocking).").size(11.).color(MUTED));
                });
                card(ui, "07 / EFFECTS", |ui| {
                    ui.label(format!("Cap {:.1} N; fingertip contact first, then a light grip. Report what you feel.", app.snapshot.effects_cap_n));
                    let names: Vec<&str> = Kind::ALL.iter().map(|k| k.name()).collect();
                    combo(ui, "effect", &mut app.forms.effect, &names);
                    let kind = Kind::ALL[app.forms.effect];
                    ui.label(RichText::new(kind.instructions()).size(11.).color(MUTED));
                    ui.horizontal(|ui| {
                        start_button(app, ui, StepRequest::Effect { kind }, "Start effect");
                        if ui.add_enabled(app.snapshot.registered && !busy, egui::Button::new("Mark centre here")).clicked() {
                            app.send(Command::CaptureStill(Mark::EffectCentre));
                        }
                    });
                    hint_line(app, ui, StepRequest::Effect { kind });
                });
                card(ui, "08 / LIFT TEST", |ui| {
                    ui.label("Rest the pen on the book stack at reset height, hands off. Run bare, then with 50 g, then 100 g.");
                    combo(ui, "mass", &mut app.forms.mass, &["Bare", "50 g", "100 g"]);
                    let req = StepRequest::Lift { mass_g: MASSES[app.forms.mass].0 };
                    start_button(app, ui, req, &format!("Start lift ({})", MASSES[app.forms.mass].1));
                    hint_line(app, ui, req);
                    let l = &app.snapshot.session.lift;
                    metric(ui, "F0 bare", l.f0().map_or("-".into(), |f| format!("{f:.3} N")));
                    metric(ui, "gain", l.gain().map_or("-".into(), |g| format!("{g:.2}")));
                });
                card(ui, "09 / WALL RAMP AND THERMAL", |ui| {
                    ui.label("Wall: touch the plane about 1 mm in, loose grip. Release at the first sustained buzz.");
                    start_button(app, ui, StepRequest::WallRamp, "Start wall ramp");
                    metric(ui, "K marginal", app.snapshot.session.k_marginal_n_mm.map_or("-".into(), |k| format!("{k:.2} N/mm")));
                    ui.separator();
                    ui.label("Thermal: pen braced down on the scale, 1.0 N for 10 minutes. Confirm every 30 s; enter the casing temperature each minute. Stops at +10 °C.");
                    ui.horizontal(|ui| {
                        start_button(app, ui, StepRequest::Thermal, "Start thermal");
                        let due = app.snapshot.confirm_in_s;
                        let open = due.is_some_and(|d| d <= 0.);
                        if ui.add_enabled(open, egui::Button::new("Confirm")).clicked() {
                            app.send(Command::Confirm);
                        }
                        if let Some(d) = due {
                            ui.label(RichText::new(if d > 0. { format!("due in {d:.0} s") } else { format!("OVERDUE {:.0} s left", 10. + d) }).color(if d > 0. { MUTED } else { AMBER }));
                        }
                    });
                    hint_line(app, ui, StepRequest::Thermal);
                });
                card(ui, "10 / COVERAGE (DISPLAY ONLY)", |ui| {
                    ui.horizontal(|ui| {
                        let mut on = app.snapshot.coverage_on;
                        if ui.checkbox(&mut on, "Record visited cells").changed() {
                            app.send(Command::Coverage(on));
                        }
                        if ui.add_enabled(!busy, egui::Button::new("Export")).clicked() {
                            app.send(Command::ExportCoverage);
                        }
                    });
                    metric(ui, "cells", app.snapshot.coverage.len());
                });
                if app.is_demo() {
                    card(ui, "SIMULATED WORLD", |ui| {
                        ui.label("Left drag in the view moves the virtual hand; Shift and wheel changes depth.");
                        ui.horizontal_wrapped(|ui| {
                            for (name, which) in [("Float", 0), ("Brace here", 1), ("Book stack", 2), ("Drop", 3), ("Reset pen", 4)] {
                                if ui.button(name).clicked() {
                                    app.sim_world(which);
                                }
                            }
                        });
                    });
                }
            });
        });
}

fn combo(ui: &mut egui::Ui, id: &str, index: &mut usize, items: &[&str]) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(items[(*index).min(items.len() - 1)])
        .show_ui(ui, |ui| {
            for (i, item) in items.iter().enumerate() {
                ui.selectable_value(index, i, *item);
            }
        });
}

fn right_panel(app: &mut App, ctx: &egui::Context) -> bool {
    let mut hold = false;
    egui::SidePanel::right("live")
        .exact_width(306.)
        .resizable(false)
        .frame(egui::Frame::new().fill(PANEL).inner_margin(12))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().id_salt("live").show(ui, |ui| {
                let s = app.snapshot.clone();
                card(ui, "HOLD", |ui| {
                    let b = ui.add_sized([ui.available_width(), 40.], egui::Button::new(RichText::new(if app.held { "HOLDING" } else { "HOLD TO RUN (Space)" }).strong()));
                    hold = b.is_pointer_button_down_on();
                    ui.label(RichText::new("Release, press Esc or the pen button to stop. Output goes to zero immediately.").size(11.).color(MUTED));
                    if let Some(req) = app.pending {
                        ui.label(RichText::new(format!("{} waits for the hold.", req.label())).color(AMBER));
                    }
                    if ui.button("STOP").clicked() {
                        app.stop();
                    }
                });
                card(ui, "STEP", |ui| {
                    match s.active {
                        Some(req) => {
                            ui.label(RichText::new(req.label()).strong().color(ACCENT));
                            metric(ui, "attempt", s.attempt_id.map_or("-".into(), |a| a.to_string()));
                            metric(ui, "elapsed", format!("{:.1} s", s.elapsed_s));
                            if let Some(r) = s.remaining_s {
                                metric(ui, "remaining", format!("{r:.1} s"));
                            }
                            if let Some(k) = s.wall_k_n_m {
                                metric(ui, "wall k", format!("{k:.0} N/m"));
                            }
                        }
                        None => {
                            ui.label(RichText::new("Idle. Output off.").color(MUTED));
                        }
                    }
                    if let Some(r) = &s.last_result {
                        ui.separator();
                        ui.label(RichText::new(format!("Last: #{} {}", r.attempt_id, r.label)).size(12.));
                        ui.label(RichText::new(format!("{} · {}", r.reason, r.verdict)).size(12.).color(if r.normal { ACCENT } else { AMBER }));
                    }
                });
                card(ui, "LIVE", |ui| {
                    match s.position {
                        Some(p) => {
                            metric(ui, "position mm", mm3(p));
                            metric(ui, "distance to reset mm", format!("{:.1}", norm(p) * 1e3));
                            ui.label(RichText::new("White box: saved reset reference. Distances are model values.").size(11.).color(MUTED));
                        }
                        None => {
                            ui.label(RichText::new("UNREGISTERED").strong().color(AMBER));
                            metric(ui, "display mm", mm3(s.unregistered_position));
                        }
                    }
                    metric(ui, "speed mm/s", format!("{:.1}", norm(s.velocity) * 1e3));
                    metric(ui, "counts 0-2", format!("{} {} {}", s.counts[0], s.counts[1], s.counts[2]));
                    metric(ui, "counts 3-5", format!("{} {} {}", s.counts[3], s.counts[4], s.counts[5]));
                    metric(ui, "rate", format!("{:.0} Hz", s.rate_hz));
                    metric(ui, "button", if s.button { "pressed" } else { "-" });
                    let t = s.telemetry;
                    ui.separator();
                    metric(ui, "applied N", format!("{:+.3} {:+.3} {:+.3}", t.applied_force[0], t.applied_force[1], t.applied_force[2]));
                    metric(ui, "offsets", format!("{} {} {}", t.applied_motor_offsets[0], t.applied_motor_offsets[1], t.applied_motor_offsets[2]));
                    ui.horizontal_wrapped(|ui| {
                        for (on, text) in [(s.feedback, "OUTPUT ARMED"), (t.force_capped, "FORCE CAP"), (t.dac_capped, "DAC CAP"), (t.slewing, "SLEW"), (t.expired, "EXPIRED")] {
                            if on {
                                ui.label(RichText::new(text).size(11.).strong().color(if text == "OUTPUT ARMED" { RED } else { AMBER }));
                            }
                        }
                    });
                    metric(ui, "limits", format!("{:.1} N, {} counts", s.limits.force_n, s.limits.dac_counts));
                    metric(ui, "stale ticks", s.stale_ticks);
                    let model = if s.registered { "commands are model values" } else { "no force until registered" };
                    ui.label(RichText::new(model).size(11.).color(MUTED));
                });
                card(ui, "UNLOCKED THIS CONNECTION", |ui| {
                    for k in &s.unlocked {
                        ui.label(RichText::new(format!("✓ {}", k.name())).color(ACCENT));
                    }
                    let locked: Vec<String> = [StepKind::SignPulse, StepKind::Ladder(Rung::N0_5), StepKind::Ladder(Rung::N1_0), StepKind::Ladder(Rung::N2_0), StepKind::Lift, StepKind::Effect, StepKind::WallRamp, StepKind::Thermal]
                        .into_iter()
                        .filter(|k| !s.unlocked.contains(k))
                        .map(StepKind::name)
                        .collect();
                    if !locked.is_empty() {
                        ui.label(RichText::new(format!("Locked: {}", locked.join(", "))).size(11.).color(MUTED));
                    }
                    metric(ui, "lift cap", format!("{:.1} N", s.lift_cap_n));
                    metric(ui, "effects cap", format!("{:.1} N", s.effects_cap_n));
                });
                entry_card(app, ui, &s);
                card(ui, "LOG", |ui| {
                    for line in s.log.iter().rev().take(10) {
                        ui.label(RichText::new(line).size(11.));
                    }
                    if let Some(d) = &s.session_dir {
                        ui.label(RichText::new(format!("Evidence: {d}")).size(10.).color(MUTED));
                    }
                });
            });
        });
    hold
}

fn entry_card(app: &mut App, ui: &mut egui::Ui, s: &crate::worker::Snapshot) {
    let output_off = !s.feedback;
    let thermal_running = s.active == Some(StepRequest::Thermal);
    card(ui, "OPERATOR ENTRIES", |ui| {
        if let Some((id, kind)) = s.awaiting_entry {
            let text = match kind {
                EntryKind::Reading => "Waiting for your reading.",
                EntryKind::Observation => "Waiting for felt and heard.",
            };
            ui.label(RichText::new(format!("Attempt {id}: {text}")).color(AMBER));
        }
        // Target the newest finished attempt whenever a new one appears, and clear the
        // previous attempt's observation so it can never be credited to another.
        if let Some(r) = &s.last_result
            && r.attempt_id != app.forms.seen_attempt
        {
            app.forms.seen_attempt = r.attempt_id;
            app.forms.attempt = r.attempt_id.to_string();
            app.forms.felt = false;
            app.forms.heard = false;
            app.forms.note.clear();
        }
        if let Some(r) = s
            .last_result
            .as_ref()
            .filter(|r| r.attempt_id.to_string() == app.forms.attempt.trim())
        {
            ui.label(
                RichText::new(format!("Entering for #{}: {}", r.attempt_id, r.label))
                    .size(11.)
                    .color(MUTED),
            );
        }
        let id_text = if thermal_running {
            s.attempt_id.map_or(String::new(), |a| a.to_string())
        } else {
            app.forms.attempt.clone()
        };
        ui.horizontal(|ui| {
            ui.label("Attempt");
            if thermal_running {
                ui.label(RichText::new(&id_text).monospace());
            } else {
                ui.add(egui::TextEdit::singleline(&mut app.forms.attempt).desired_width(48.));
            }
        });
        let id = id_text.trim().parse::<u64>().ok();
        let enabled_reading = output_off || thermal_running;
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut app.forms.reading_label).desired_width(70.));
            ui.add(egui::TextEdit::singleline(&mut app.forms.reading_value).desired_width(58.));
            let units: Vec<&str> = UNITS.iter().map(|u| u.1).collect();
            combo(ui, "unit", &mut app.forms.reading_unit, &units);
        });
        ui.label(
            RichText::new("N gauge, g scale, °C casing, mm ruler.")
                .size(11.)
                .color(MUTED),
        );
        if ui
            .add_enabled(
                enabled_reading && id.is_some(),
                egui::Button::new("Submit reading"),
            )
            .clicked()
        {
            match (id, parse(&app.forms.reading_value)) {
                (Some(attempt_id), Some(value)) => {
                    let unit = UNITS[app.forms.reading_unit].0;
                    if thermal_running && unit != Unit::C {
                        app.notice =
                            "During the thermal hold only temperatures are accepted.".into();
                    } else {
                        app.send(Command::SubmitReading {
                            attempt_id,
                            readings: vec![Reading {
                                label: app.forms.reading_label.clone(),
                                value,
                                unit,
                            }],
                        });
                        app.forms.reading_value.clear();
                    }
                }
                _ => app.notice = "Enter an attempt number and a numeric value.".into(),
            }
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.checkbox(&mut app.forms.felt, "felt it");
            ui.checkbox(&mut app.forms.heard, "heard it");
        });
        ui.add(
            egui::TextEdit::singleline(&mut app.forms.note)
                .hint_text("note")
                .desired_width(ui.available_width()),
        );
        if ui
            .add_enabled(output_off, egui::Button::new("Submit observation"))
            .clicked()
        {
            match id {
                Some(attempt_id) => app.send(Command::SubmitObservation {
                    attempt_id,
                    felt: Some(app.forms.felt),
                    heard: Some(app.forms.heard),
                    note: app.forms.note.clone(),
                }),
                None => app.notice = "Enter an attempt number first.".into(),
            }
            app.forms.felt = false;
            app.forms.heard = false;
            app.forms.note.clear();
        }
        if !output_off {
            ui.label(
                RichText::new("Entries for finished attempts open only while output is off.")
                    .size(11.)
                    .color(MUTED),
            );
        }
    });
}

/// Lay out every panel and the viewport overlay.
pub fn interface(app: &mut App, ctx: &egui::Context) -> Frame {
    egui::TopBottomPanel::top("header")
        .exact_height(64.)
        .frame(
            egui::Frame::new()
                .fill(PANEL)
                .inner_margin(egui::Margin::symmetric(20, 10)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("CALIBRATION STUDIO").size(20.).strong());
                    ui.label(
                        RichText::new("PHANToM 1.0A · register, pulse, load, measure")
                            .size(11.)
                            .color(MUTED),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let s = &app.snapshot;
                    let (text, color) = if s.feedback {
                        ("MOTOR OUTPUT ARMED", RED)
                    } else {
                        ("MOTOR OUTPUT OFF", ACCENT)
                    };
                    ui.label(RichText::new(text).strong().size(12.).color(color));
                    if s.simulated {
                        ui.label(RichText::new("SIMULATION").strong().size(12.).color(AMBER));
                    }
                    if s.connected && !s.registered {
                        ui.label(
                            RichText::new("UNREGISTERED")
                                .strong()
                                .size(12.)
                                .color(AMBER),
                        );
                    }
                });
            });
        });
    egui::TopBottomPanel::bottom("status")
        .min_height(44.)
        .show(ctx, |ui| {
            ui.label(RichText::new(&app.snapshot.status).size(12.));
            if !app.notice.is_empty() {
                ui.label(RichText::new(&app.notice).size(11.).color(AMBER));
            }
        });
    left_panel(app, ctx);
    let hold_button = right_panel(app, ctx);
    let r = ctx.available_rect();
    let rect = Rect::new(r.min.x, r.min.y, r.width().max(1.), r.height().max(1.));
    let p = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("overlay"),
    ));
    let title = app
        .snapshot
        .active
        .map_or("Idle".to_string(), |r| r.label());
    p.text(
        egui::pos2(rect.x + 22., rect.y + 18.),
        egui::Align2::LEFT_TOP,
        title,
        egui::FontId::proportional(22.),
        Color32::from_rgb(231, 242, 248),
    );
    let (sub, color) = if !app.snapshot.connected {
        ("NOT CONNECTED", AMBER)
    } else if !app.snapshot.registered {
        (
            "UNREGISTERED · power-on counts, display only, no force possible",
            AMBER,
        )
    } else {
        (
            "REGISTERED · bright arm live, faint arm is the reset pose",
            MUTED,
        )
    };
    p.text(
        egui::pos2(rect.x + 22., rect.y + 48.),
        egui::Align2::LEFT_TOP,
        sub,
        egui::FontId::monospace(11.),
        color,
    );
    p.text(
        egui::pos2(rect.x + 22., rect.bottom() - 22.),
        egui::Align2::LEFT_BOTTOM,
        "Right drag: orbit · wheel: zoom · Home: reset view · Space: hold · Esc: stop",
        egui::FontId::proportional(11.),
        MUTED,
    );
    let camera = app.orbit.camera(rect);
    for (axis, label, c) in [
        (Vec3::X, "+X", Color32::from_rgb(234, 113, 126)),
        (Vec3::Y, "+Y up", Color32::from_rgb(125, 220, 160)),
        (Vec3::Z, "+Z", Color32::from_rgb(113, 166, 247)),
    ] {
        if let Some(pos) = scene::project(&camera, rect, axis * 3.6) {
            p.text(
                egui::pos2(pos.x, pos.y),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::monospace(11.),
                c,
            );
        }
    }
    Frame {
        rect,
        hold_button,
        keyboard: ctx.wants_keyboard_input(),
    }
}

/// Output is off and nothing runs, so a gravity drop may be armed.
fn output_idle(s: &crate::worker::Snapshot) -> bool {
    !s.feedback && s.active.is_none()
}
