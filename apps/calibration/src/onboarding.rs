//! Guided presentation of existing worker operations (P-0066).
//! This module grants no hardware authority and never supplies a force demand.
use crate::App;
use crate::protocol::{Mark, StepRequest};
use crate::worker::{Command, Snapshot};
use egui_macroquad::egui::{self, Color32, RichText};
use visiontouch_effects::math::norm;

const INK: Color32 = Color32::from_rgb(229, 237, 245);
const GREEN: Color32 = Color32::from_rgb(118, 231, 201);
const MUTED: Color32 = Color32::from_rgb(151, 172, 190);
const AMBER: Color32 = Color32::from_rgb(248, 197, 116);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Stage {
    #[default]
    Connect,
    Pose,
    Still,
    Return,
    Motors,
    Measure,
}
impl Stage {
    fn title(self) -> &'static str {
        match self {
            Self::Connect => "First, see the arm respond",
            Self::Pose => "Give the arm a reference",
            Self::Still => "Check zero without moving",
            Self::Return => "Return to the physical pose",
            Self::Motors => "Check one motor at a time",
            Self::Measure => "Ready for physical measurements",
        }
    }
    fn number(self) -> usize {
        match self {
            Self::Connect => 1,
            Self::Pose => 2,
            Self::Still => 3,
            Self::Return => 4,
            Self::Motors => 5,
            Self::Measure => 6,
        }
    }
}

#[derive(Default)]
pub struct Guide {
    pub bench: bool,
    pub stage: Stage,
    reference: Option<[i64; 6]>,
    connected: bool,
    waiting_reference: bool,
    waiting_capture: Option<usize>,
    captured: bool,
    moved: bool,
    channel: usize,
    after_attempt: u64,
    felt: Option<bool>,
    heard: Option<bool>,
    note: String,
    replace_confirm: bool,
}
impl Guide {
    pub fn state(&self) -> serde_json::Value {
        serde_json::json!({"view": if self.bench { "bench" } else { "guide" },
            "stage": format!("{:?}", self.stage).to_lowercase(), "title": self.stage.title(),
            "channel": self.channel, "capture_acknowledged": self.captured,
            "movement_observed": self.moved, "waiting_reference": self.waiting_reference,
            "waiting_capture": self.waiting_capture.is_some()})
    }
    pub fn reference_requested(&mut self) {
        self.waiting_reference = true;
    }
    pub fn capture_requested(&mut self, s: &Snapshot) -> Result<(), String> {
        if !self.bench && self.stage == Stage::Return && !self.moved {
            return Err("Move away from the reference before capturing a return".into());
        }
        self.waiting_capture = Some(s.session.checks.returns.len());
        self.captured = false;
        Ok(())
    }
    /// Advance presentation only when the same evidence required by the buttons exists.
    pub fn advance(&mut self, s: &Snapshot) -> Result<(), String> {
        match self.stage {
            Stage::Connect if s.connected && s.rate_hz >= 300. => self.stage = Stage::Pose,
            Stage::Pose if s.registered => {
                self.stage = Stage::Still;
                self.captured = false;
            }
            Stage::Still if self.captured_ok(s) => {
                self.stage = Stage::Return;
                self.captured = false;
                self.moved = false;
            }
            Stage::Return if self.captured_ok(s) && self.moved => {
                self.stage = Stage::Motors;
                self.captured = false;
                self.after_attempt = s.last_result.as_ref().map_or(0, |r| r.attempt_id);
            }
            Stage::Motors if self.motor_passed(s) => {
                self.channel += 1;
                self.after_attempt = s.last_result.as_ref().map_or(0, |r| r.attempt_id);
                self.felt = None;
                self.heard = None;
                self.note.clear();
                if self.channel == 3 {
                    self.stage = Stage::Measure;
                }
            }
            Stage::Measure => self.bench = true,
            _ => return Err("Complete the current guided task before advancing".into()),
        }
        Ok(())
    }
    /// Synchronise only with acknowledged worker evidence; button presses are not passes.
    pub fn sync(&mut self, s: &Snapshot) {
        if !s.connected && self.connected {
            let bench = self.bench;
            *self = Self {
                bench,
                ..Self::default()
            };
        }
        self.connected = s.connected;
        let reference = s.registration.map(|r| r.reset_counts);
        if reference != self.reference {
            self.reference = reference;
            self.captured = false;
            self.waiting_capture = None;
            self.moved = false;
            self.channel = 0;
            if self.stage != Stage::Connect {
                self.stage = if reference.is_some() {
                    Stage::Still
                } else {
                    Stage::Pose
                };
            }
        }
        if self.waiting_reference && s.registered && s.session.checks.returns.is_empty() {
            self.waiting_reference = false;
            self.stage = Stage::Still;
            self.captured = false;
            self.waiting_capture = None;
        }
        if self
            .waiting_capture
            .is_some_and(|n| s.session.checks.returns.len() > n)
        {
            self.waiting_capture = None;
            self.captured = true;
        }
        if self.stage == Stage::Return && s.position.is_some_and(|p| norm(p) > 0.005) {
            self.moved = true;
        }
    }
    fn captured_ok(&self, s: &Snapshot) -> bool {
        self.captured && s.session.checks.return_ok() == Some(true)
    }
    fn current_result<'a>(&self, s: &'a Snapshot) -> Option<&'a crate::worker::LastResult> {
        s.last_result.as_ref().filter(|r| {
            r.attempt_id > self.after_attempt && r.label == format!("Liveness ch{}", self.channel)
        })
    }
    fn motor_passed(&self, s: &Snapshot) -> bool {
        self.current_result(s)
            .is_some_and(|r| r.reason == "completed")
            && s.session.liveness.get(self.channel) == Some(&Some(true))
    }
}

fn button(ui: &mut egui::Ui, enabled: bool, label: &str) -> bool {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(label).size(16.))
            .min_size(egui::vec2(ui.available_width(), 42.)),
    )
    .clicked()
}
fn copy(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(16.).color(INK));
    ui.add_space(8.);
}
fn note(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(13.).color(MUTED));
}
fn stop_explanation(reason: &str) -> &'static str {
    match reason {
        "speed" => {
            "The model speed exceeded the test limit. This is a software stop, not a diagnosis of motor failure or hardware overload. Keep output off and review the recording before another attempt."
        }
        "excursion" => {
            "The pen travelled beyond this test's allowed distance. The attempt stopped; it did not establish motor health."
        }
        "hold-released" => {
            "The hold was released before the test finished. Output was stopped. This is not a failed motor."
        }
        "sample-gap" | "ui-heartbeat-stale" => {
            "The control or display timing was interrupted. Resolve the timing issue before another powered attempt."
        }
        "pen-button" => "The pen button stopped the test.",
        "completed" => {
            "The two-second test finished. Record what you felt and heard below; completion alone does not prove delivered force."
        }
        _ => {
            "The test stopped. Read the recorded reason and session details before deciding on another attempt. A stopped test is not a motor diagnosis."
        }
    }
}
fn choice(ui: &mut egui::Ui, label: &str, value: &mut Option<bool>) {
    ui.label(label);
    ui.horizontal(|ui| {
        ui.selectable_value(value, Some(true), "Yes");
        ui.selectable_value(value, Some(false), "No");
        ui.selectable_value(value, None, "Not sure");
    });
}

/// Our schematic of the specified physical pose, independent of live counters.
fn pose_diagram(ui: &mut egui::Ui) {
    let (r, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 164.), egui::Sense::hover());
    let p = ui.painter_at(r);
    let a = r.min + egui::vec2(40., 56.);
    let b = r.min + egui::vec2(r.width() - 65., 56.);
    let c = b + egui::vec2(0., 66.);
    p.line_segment([a, b], egui::Stroke::new(6_f32, GREEN));
    p.line_segment([b, c], egui::Stroke::new(6_f32, GREEN));
    for v in [a, b, c] {
        p.circle_filled(v, 7., INK);
    }
    p.line_segment(
        [a, a + egui::vec2(0., 65.)],
        egui::Stroke::new(4_f32, MUTED),
    );
    p.text(
        r.min + egui::vec2(12., 4.),
        egui::Align2::LEFT_TOP,
        "SIDE VIEW · REFERENCE SCHEMATIC",
        egui::FontId::proportional(11.),
        MUTED,
    );
    p.text(
        (a + b.to_vec2()) / 2. + egui::vec2(0., -12.),
        egui::Align2::CENTER_BOTTOM,
        "near arm level",
        egui::FontId::proportional(14.),
        INK,
    );
    p.text(
        b + egui::vec2(10., 33.),
        egui::Align2::LEFT_CENTER,
        "far arm\nvertical",
        egui::FontId::proportional(12.),
        INK,
    );
    p.text(
        c + egui::vec2(0., 14.),
        egui::Align2::CENTER_TOP,
        "pen",
        egui::FontId::proportional(13.),
        INK,
    );
}

fn advance_guide(app: &mut App) {
    if let Err(e) = app.guide.advance(&app.snapshot) {
        app.notice = e;
    }
}

fn capture(app: &mut App) {
    if let Err(e) = app.guide.capture_requested(&app.snapshot) {
        app.notice = e;
        return;
    }
    app.send(Command::CaptureStill(Mark::ReturnToReset));
}
fn replace_reference(app: &mut App, ui: &mut egui::Ui, idle: bool) {
    ui.add_space(12.);
    if ui
        .add_enabled(idle, egui::Button::new("Set a different reference…"))
        .clicked()
    {
        app.guide.replace_confirm = true;
    }
    if app.guide.replace_confirm {
        note(
            ui,
            "This clears the existing return checks and force verification for this reference. Earlier attempt files remain saved.",
        );
        if button(ui, idle, "Go back to reference setup") {
            app.stop();
            app.guide.stage = Stage::Pose;
            app.guide.replace_confirm = false;
            app.guide.captured = false;
        }
        if ui.button("Keep this reference").clicked() {
            app.guide.replace_confirm = false;
        }
    }
}

/// One task panel, leaving the rest of the window to the arm view.
pub fn panel(app: &mut App, ctx: &egui::Context) {
    app.guide.sync(&app.snapshot);
    egui::SidePanel::left("onboarding")
        .exact_width((ctx.screen_rect().width() * 0.38).clamp(300., 390.))
        .resizable(false)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let idle = app.snapshot.active.is_none() && !app.snapshot.feedback && app.pending.is_none();
                let ready = idle && app.snapshot.connected && app.snapshot.stable;
                ui.add_space(12.);
                ui.label(RichText::new(format!("GUIDED SETUP  /  {} OF 6", app.guide.stage.number())).color(GREEN).size(12.));
                ui.add_space(10.);
                ui.label(RichText::new(app.guide.stage.title()).size(27.).strong());
                ui.add_space(12.);
                match app.guide.stage {
                    Stage::Connect => {
                        copy(ui, "Support the pen and move it gently. Watch the live arm in the view.");
                        note(ui, "No gauge is needed for this setup. Motor output stays off until you explicitly prepare a motor check and hold Space.");
                        ui.add_space(12.);
                        if !app.snapshot.connected {
                            if button(ui, idle, "Connect with output off") { app.send(Command::Connect); }
                        } else {
                            ui.label(RichText::new(format!("Connected · {:.0} samples/s", app.snapshot.rate_hz)).color(GREEN));
                            ui.label(if app.snapshot.stable { "Pen is still" } else { "Pen is moving" });
                            note(ui, "The unregistered drawing is approximate. At this point, check that it responds to movement, not its exact shape or position.");
                            if button(ui, idle && app.snapshot.rate_hz >= 300., "The view responds · continue") { let _ = app.guide.advance(&app.snapshot); }
                        }
                    }
                    Stage::Pose => {
                        pose_diagram(ui);
                        copy(ui, "Place the near arm level and square to the front of the base. The far arm hangs vertically.");
                        note(ui, "This physical pose defines zero. It is not the centre of the sweep or the resting position on the desk. Support it lightly and hold still.");
                        ui.add_space(12.);
                        if app.snapshot.registered {
                            note(ui, "A reference already exists. Saving here replaces it and clears its checks.");
                            if button(ui, idle, "Use the existing reference") { let _ = app.guide.advance(&app.snapshot); }
                        }
                        if button(ui, ready && !app.guide.waiting_reference, if app.snapshot.registered { "Replace reference with this pose" } else { "Save this pose as zero" }) {
                            app.guide.reference_requested();
                            app.send(Command::RegisterReset);
                        }
                        if !app.snapshot.stable { note(ui, "Waiting for a steady pen (a quarter of a second)."); }
                    }
                    Stage::Still | Stage::Return => {
                        let returning = app.guide.stage == Stage::Return;
                        if returning {
                            copy(ui, "Move a small distance away, then reproduce the physical arm pose you just saved. Hold still.");
                            note(ui, "Use the arm's physical alignment, not the cursor, to find the pose. Chasing the screen target cannot measure physical repeatability.");
                            pose_diagram(ui);
                            ui.label(if app.guide.moved { "Movement away from zero detected" } else { "First move at least 5 model mm away" });
                        } else {
                            copy(ui, "Keep supporting the pen in the pose you just saved. Do not deliberately move it yet.");
                            note(ui, "We are checking that saving zero and reading it back agree. This is not yet a return test.");
                        }
                        if let Some(p) = app.snapshot.position {
                            ui.label(RichText::new(format!("{:.1} mm", norm(p) * 1000.)).size(34.));
                            note(ui, "Live model distance from the saved reference");
                        }
                        if button(ui, ready && (!returning || app.guide.moved), if returning { "Capture this return" } else { "Check this stationary pose" }) { capture(app); }
                        if app.guide.captured {
                            let checks = &app.snapshot.session.checks;
                            ui.label(format!("Captured distance: {:.2} mm", checks.return_error_mm.unwrap_or(0.)));
                            if let Some(r) = checks.repeat_mm { ui.label(format!("Spread between captures: {r:.2} mm")); }
                            if app.guide.captured_ok(&app.snapshot) {
                                ui.label(RichText::new("Within the 2 mm model tolerance").color(GREEN));
                                if button(ui, idle, if returning { "Continue to motor checks" } else { "Now check a physical return" }) {
                                    advance_guide(app);
                                }
                            } else {
                                ui.label(RichText::new("Reference check needs attention").color(AMBER));
                                note(ui, "The capture is outside tolerance. Check the physical pose and the pen support. If the reference was wrong, deliberately set a new one below. Further captures do not erase earlier spread.");
                            }
                        }
                        replace_reference(app, ui, idle);
                    }
                    Stage::Motors => motors(app, ui, ready, idle),
                    Stage::Measure => {
                        copy(ui, "The initial reference and motor response checks are recorded. Delivered force is not calibrated yet.");
                        note(ui, "Next in the bench: gravity direction, ruler checks on X/Y/Z, then sign pulses and measured force. A gauge or known masses are needed for force measurements, not for the setup you just completed.");
                        if button(ui, idle, "Open measurement bench") { app.stop(); app.guide.bench = true; }
                    }
                }
                ui.add_space(18.);
                ui.separator();
                ui.collapsing("Session details & saved evidence", |ui| {
                    ui.label(format!("Sampling: {:.0} Hz", app.snapshot.rate_hz));
                    ui.label(format!("Motor command counts: {:?}", app.snapshot.telemetry.applied_motor_offsets));
                    note(ui, "Command counts are not a delivered-force measurement.");
                    if let Some(path) = &app.snapshot.session_dir { ui.label(path); }
                    else { note(ui, "A session folder is created when the first reference or attempt is saved."); }
                    for line in app.snapshot.log.iter().rev().take(6) { ui.label(line); }
                });
            });
        });
}

fn motors(app: &mut App, ui: &mut egui::Ui, ready: bool, idle: bool) {
    let channel = app.guide.channel;
    copy(
        ui,
        &format!(
            "Channel {channel} of 0, 1, 2. Support the pen near the saved pose. Notice any tug or sound; do not push against it."
        ),
    );
    note(
        ui,
        "This is the existing 80-count, 3 Hz, two-second check. The physical channel directions are not yet verified.",
    );
    if app.pending.is_some() {
        ui.add_space(16.);
        ui.label(RichText::new("HOLD SPACE TO START").size(24.).color(GREEN));
        copy(
            ui,
            "Keep holding for two seconds. Release to stop immediately.",
        );
        if button(ui, true, "Cancel prepared test") {
            app.stop();
        }
    } else if app.snapshot.active.is_some() {
        ui.add(
            egui::ProgressBar::new((app.snapshot.elapsed_s / 2.).clamp(0., 1.) as f32).text(
                format!(
                    "{:.1} / 2.0 seconds · release Space to stop",
                    app.snapshot.elapsed_s
                ),
            ),
        );
    } else if let Some(result) = app.guide.current_result(&app.snapshot).cloned() {
        ui.add_space(12.);
        let completed = result.reason == "completed";
        ui.label(
            RichText::new(if completed {
                "Test completed"
            } else {
                "Test stopped early"
            })
            .size(23.)
            .color(if completed { GREEN } else { AMBER }),
        );
        ui.label(format!(
            "Attempt #{} · {} · {:.3} seconds",
            result.attempt_id, result.reason, result.duration_s
        ));
        copy(ui, stop_explanation(&result.reason));
        choice(ui, "Did you feel a response?", &mut app.guide.felt);
        choice(ui, "Did you hear a response?", &mut app.guide.heard);
        ui.add(
            egui::TextEdit::multiline(&mut app.guide.note)
                .hint_text("Anything else you noticed (optional)")
                .desired_rows(2),
        );
        if button(ui, idle, "Save observations for this attempt") {
            app.send(Command::SubmitObservation {
                attempt_id: result.attempt_id,
                felt: app.guide.felt,
                heard: app.guide.heard,
                note: app.guide.note.clone(),
            });
        }
        if app.guide.motor_passed(&app.snapshot) {
            if button(
                ui,
                idle,
                if channel == 2 {
                    "Continue to measurement setup"
                } else {
                    "Prepare the next channel"
                },
            ) {
                advance_guide(app);
            }
        } else {
            note(
                ui,
                "No next channel is offered from this result. The recording and your observations are the next diagnostic evidence.",
            );
            if result.reason == "hold-released"
                && button(ui, ready && !app.held, "Prepare one retry")
            {
                app.guide.after_attempt = result.attempt_id;
                app.guide.felt = None;
                app.guide.heard = None;
                app.guide.note.clear();
                app.request(StepRequest::Liveness { channel });
            }
        }
    } else {
        let checked =
            app.snapshot.registered && app.snapshot.session.checks.return_ok() == Some(true);
        if button(
            ui,
            ready && checked && !app.held,
            "Prepare this motor check",
        ) {
            app.request(StepRequest::Liveness { channel });
        }
        note(
            ui,
            "Preparing does not drive the motor. You will then hold Space to start. Escape or STOP cancels it. Limits remain active.",
        );
    }
    replace_reference(app, ui, idle);
}

#[cfg(test)]
mod tests {
    use super::*;
    use visiontouch_device::Registration;
    fn snapshot() -> Snapshot {
        Snapshot {
            connected: true,
            registered: true,
            registration: Some(Registration {
                reset_counts: [0; 6],
            }),
            ..Snapshot::default()
        }
    }
    #[test]
    fn capture_needs_worker_ack_and_cannot_pass_by_clicking() {
        let mut s = snapshot();
        let mut g = Guide::default();
        g.sync(&s);
        g.stage = Stage::Still;
        g.waiting_capture = Some(0);
        g.sync(&s);
        assert!(!g.captured_ok(&s));
        s.session.checks.returns.push([0.; 3]);
        s.session.checks.return_error_mm = Some(0.5);
        g.sync(&s);
        assert!(g.captured_ok(&s));
        s.session.checks.repeat_mm = Some(3.);
        assert!(!g.captured_ok(&s));
    }
    #[test]
    fn reference_change_and_disconnect_invalidate_progress() {
        let mut s = snapshot();
        let mut g = Guide::default();
        g.sync(&s);
        g.stage = Stage::Motors;
        g.captured = true;
        s.registration.as_mut().unwrap().reset_counts[0] = 10;
        g.sync(&s);
        assert_eq!(g.stage, Stage::Still);
        assert!(!g.captured);
        s.connected = false;
        s.registration = None;
        g.sync(&s);
        assert_eq!(g.stage, Stage::Connect);
    }
    #[test]
    fn an_old_or_different_channel_result_is_not_this_attempt() {
        let mut s = snapshot();
        s.last_result = Some(crate::worker::LastResult {
            attempt_id: 3,
            label: "Liveness ch1".into(),
            ..Default::default()
        });
        let mut g = Guide::default();
        assert!(g.current_result(&s).is_none());
        g.channel = 1;
        assert!(g.current_result(&s).is_some());
        g.after_attempt = 3;
        assert!(g.current_result(&s).is_none());
    }

    #[test]
    fn an_aborted_attempt_cannot_advance_even_with_an_old_channel_pass() {
        let mut s = snapshot();
        s.session.liveness[0] = Some(true);
        s.last_result = Some(crate::worker::LastResult {
            attempt_id: 1,
            label: "Liveness ch0".into(),
            reason: "speed".into(),
            ..Default::default()
        });
        let g = Guide::default();
        assert!(!g.motor_passed(&s));
        s.last_result.as_mut().unwrap().reason = "completed".into();
        assert!(g.motor_passed(&s));
        s.session.liveness[0] = Some(false);
        assert!(!g.motor_passed(&s));
    }
}
