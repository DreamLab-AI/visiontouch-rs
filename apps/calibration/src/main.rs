//! Calibration Studio: guided, evidence-recording checks of a haptic arm.
//!
//! The interface sends intent to an independent servo worker; rendering never issues
//! a motor command. Nothing here opens hardware except the default mode, and that
//! connects with output off. `--demo` and `--smoke-test` use a simulated plant.
mod arm;
mod controller;
mod coverage;
mod evidence;
mod onboarding;
mod protocol;
mod registration;
mod scene;
mod sim;
mod smoke;
mod ui;
mod worker;

use egui_macroquad::egui;
use macroquad::prelude::*;
use protocol::{Class, StepRequest};
use sim::{Fixture, SimHw, SimPlant};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use worker::{Command, Engine, Snapshot, Worker};

const HELP: &str = "Calibration Studio

  --demo        Mouse-driven simulated plant; never opens hardware
  --smoke-test  Run every step against the simulator in a temporary directory,
                assert the unlock path and exit 0; never opens hardware
  --no-device   Start offline; connect only through the Connect button
  --help        This text

Default: connect with motor output off. Holds explicitly enable attempts.";

/// Where the interface gets its snapshots from.
pub enum Backend {
    /// The servo thread driving the real device.
    Worker(Worker),
    /// An in-process engine over the simulated plant, stepped once per frame.
    Demo(Box<Engine<SimHw>>),
}

/// Interface state.
pub struct App {
    pub backend: Backend,
    pub snapshot: Snapshot,
    pub forms: ui::Forms,
    pub guide: onboarding::Guide,
    pub orbit: scene::Orbit,
    pub held: bool,
    pub hold_since: Option<Instant>,
    pub blocked: bool,
    pub pending: Option<StepRequest>,
    pub notice: String,
    pub depth: f64,
}

impl App {
    fn new(demo: bool, no_device: bool) -> Self {
        let backend = if demo {
            let root = std::env::temp_dir().join("visiontouch-studio-demo");
            let mut e = Engine::new(root, true);
            e.connect(SimHw::new(SimPlant::default()));
            Backend::Demo(Box::new(e))
        } else {
            let w = Worker::spawn(worker::EVIDENCE_DIR.into());
            if !no_device {
                w.send(Command::Connect);
            }
            Backend::Worker(w)
        };
        Self {
            backend,
            snapshot: Snapshot::default(),
            forms: ui::Forms::default(),
            guide: onboarding::Guide::default(),
            orbit: scene::Orbit::default(),
            held: false,
            hold_since: None,
            blocked: false,
            pending: None,
            notice: String::new(),
            depth: 0.,
        }
    }
    pub fn is_demo(&self) -> bool {
        matches!(self.backend, Backend::Demo(_))
    }
    pub fn send(&mut self, cmd: Command) {
        self.notice.clear();
        match &mut self.backend {
            Backend::Worker(w) => w.send(cmd),
            Backend::Demo(e) => match cmd {
                Command::Connect => e.connect(SimHw::new(SimPlant::default())),
                other => e.handle(other),
            },
        }
    }
    /// Apply a world change in the simulator (demo only).
    pub fn sim_world(&mut self, which: u8) {
        if let Backend::Demo(e) = &mut self.backend
            && let Some(hw) = e.hardware_mut()
        {
            let p = &mut hw.plant;
            match which {
                0 => p.float(),
                1 => p.brace(),
                2 => {
                    p.fixture = Fixture::Stack {
                        floor_y: p.pos[1],
                        hung_n: 0.,
                    }
                }
                3 => {
                    p.fixture = Fixture::Dropped {
                        floor_y: p.pos[1] - 0.02,
                    }
                }
                _ => {
                    p.teleport([0.; 3]);
                    p.float();
                }
            }
        }
    }
    fn poll(&mut self) {
        if let Backend::Worker(w) = &self.backend {
            while let Ok(s) = w.snapshots.try_recv() {
                self.snapshot = s;
            }
        }
    }
    /// Step the in-process simulator by the frame time, driven by the mouse.
    fn pump_demo(&mut self, rect: Rect) {
        let held = self.held;
        let mouse = Vec2::from(mouse_position());
        if is_key_down(KeyCode::LeftShift) {
            self.depth = (self.depth + f64::from(mouse_wheel().1) * 0.001).clamp(-0.06, 0.06);
        }
        let hand = (rect.contains(mouse) && is_mouse_button_down(MouseButton::Left)).then(|| {
            [
                f64::from((mouse.x - rect.center().x) / rect.w) * 0.3,
                f64::from((rect.center().y - mouse.y) / rect.h) * 0.3,
                self.depth,
            ]
        });
        let Backend::Demo(e) = &mut self.backend else {
            return;
        };
        let n = ((get_frame_time() / 0.0024).round() as usize).clamp(1, 12);
        if let Some(h) = hand
            && let Some(hw) = e.hardware_mut()
            && matches!(hw.plant.fixture, Fixture::Free)
        {
            hw.plant.set_hand(h);
        }
        for _ in 0..n {
            if e.step(held, 0.).is_err() {
                break;
            }
        }
        self.snapshot = e.snapshot();
    }
    /// Operator pressed a Start button.
    pub fn request(&mut self, req: StepRequest) {
        if matches!(req.class(), Class::Backdrive | Class::Thermal) {
            self.send(Command::Start(req));
        } else if let Err(why) = self.snapshot.session.check(req) {
            self.pending = None;
            self.notice = why;
        } else {
            self.pending = Some(req);
            // A key already held while preparing is not consent for a new attempt.
            // Require release and a fresh hold before dispatching the request.
            if self.held {
                self.blocked = true;
                self.hold_since = None;
            }
            if !self.held {
                self.notice =
                    "Hold the key (Space or the hold button) to begin; release to stop.".into();
            }
        }
    }
    pub fn stop(&mut self) {
        self.blocked = true;
        self.pending = None;
        if let Backend::Worker(w) = &self.backend {
            w.controls.held.store(false, Ordering::Relaxed);
        }
        self.send(Command::Stop);
    }
    /// A step starts only on a fresh hold; a fault never restarts from a held key.
    fn hold(&mut self, input: bool) {
        if !input {
            self.blocked = false;
        }
        let held = input && !self.blocked;
        if held && !self.held {
            self.hold_since = Some(Instant::now());
        }
        if !held {
            self.hold_since = None;
        }
        self.held = held;
        if let Backend::Worker(w) = &self.backend {
            w.controls.pulse.fetch_add(1, Ordering::Relaxed);
            w.controls.held.store(held, Ordering::Relaxed);
        }
        if let (Some(req), Some(since)) = (self.pending, self.hold_since)
            && since.elapsed() >= Duration::from_millis(120)
        {
            self.pending = None;
            self.send(Command::Start(req));
        }
    }
}

fn conf() -> Conf {
    Conf {
        window_title: "Haptics Calibration Studio".into(),
        window_width: 1560,
        window_height: 960,
        high_dpi: true,
        sample_count: 4,
        platform: miniquad::conf::Platform {
            linux_wm_class: "visiontouch-studio",
            swap_interval: Some(1),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    const KNOWN: [&str; 5] = ["--demo", "--smoke-test", "--no-device", "--help", "-h"];
    if let Some(bad) = args.iter().find(|a| !KNOWN.contains(&a.as_str())) {
        eprintln!("Unknown option {bad}\n{HELP}");
        std::process::exit(2);
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{HELP}");
        return;
    }
    if args.iter().any(|a| a == "--smoke-test") {
        let dir = std::env::temp_dir().join(format!(
            "visiontouch-studio-smoke-{}",
            evidence::new_session_id()
        ));
        match smoke::run(&dir) {
            Ok(report) => {
                for l in &report.lines {
                    println!("  {l}");
                }
                println!(
                    "Smoke test passed: {} attempts, {} steps unlocked, evidence verified in {}. Simulated plant; no hardware opened.",
                    report.attempts,
                    report.unlocked.len(),
                    report.dir.display()
                );
                let _ = std::fs::remove_dir_all(dir);
            }
            Err(e) => {
                eprintln!("Smoke test FAILED: {e}");
                std::process::exit(1);
            }
        }
        return;
    }
    let demo = args.iter().any(|a| a == "--demo");
    let no_device = args.iter().any(|a| a == "--no-device");
    if !demo && let Err(error) = visiontouch_device::shutdown::install() {
        eprintln!("Cannot install shutdown handler: {error}");
        std::process::exit(1);
    }
    macroquad::Window::from_config(conf(), graphical(demo, no_device));
}

async fn graphical(demo: bool, no_device: bool) {
    prevent_quit();
    let mut app = App::new(demo, no_device);
    let mut previous = Vec2::from(mouse_position());
    let mut frame = 0_u32;
    loop {
        if is_quit_requested() || (!demo && visiontouch_device::shutdown::requested()) {
            break;
        }
        app.poll();
        clear_background(Color::from_rgba(10, 18, 27, 255));
        let mut out = None;
        egui_macroquad::ui(|ctx: &egui::Context| {
            if frame == 0 {
                ui::theme(ctx);
            }
            out = Some(ui::interface(&mut app, ctx));
        });
        let Some(f) = out else {
            next_frame().await;
            continue;
        };
        // Focus loss and the hold key (L6). miniquad has no focus event of its own; on X11
        // it reports FocusOut as `window_minimized_event`, and macroquad answers that by
        // draining its `keys_down` set, so `is_key_down` turns false and the hold drops.
        // On Wayland miniquad documents that the key-up is not delivered when the window
        // loses focus, and no focus event is raised, so a held Space could stay latched.
        // A "recent key-down" fallback was rejected: it needs key autorepeat to reach the
        // app, whose initial delay is often longer than any safe timeout, and it would
        // break every legitimate hold. The UI heartbeat (250 ms) and the Stop button remain.
        let input = f.hold_button || (!f.keyboard && is_key_down(KeyCode::Space));
        if is_key_pressed(KeyCode::Escape) {
            app.stop();
        }
        if is_key_pressed(KeyCode::Home) && !f.keyboard {
            app.orbit = Default::default();
        }
        app.hold(input);
        app.orbit.input(f.rect, previous);
        previous = Vec2::from(mouse_position());
        if demo {
            app.pump_demo(f.rect);
        }
        scene::draw(&app.snapshot, &app.orbit.camera(f.rect));
        egui_macroquad::draw();
        frame = frame.wrapping_add(1);
        next_frame().await;
    }
    app.stop();
    drop(app); // Worker::drop clears the hold, sends Quit, disarms and joins.
}

#[cfg(test)]
mod interaction_tests {
    use super::*;

    #[test]
    fn preparing_while_space_is_down_requires_release_and_a_new_hold() {
        let mut app = App::new(true, false);
        app.snapshot.session = protocol::Session::connected();
        app.held = true;
        app.hold_since = Some(Instant::now() - Duration::from_secs(1));
        app.request(StepRequest::Liveness { channel: 0 });
        app.hold(true);
        assert!(!app.held);
        assert!(app.pending.is_some());
        app.hold(false);
        app.hold(true);
        assert!(app.held);
        assert!(
            app.pending.is_some(),
            "new hold must complete its own dwell"
        );
        app.stop();
        assert!(app.pending.is_none(), "navigation/Stop cancels preparation");
        app.hold(true);
        assert!(!app.held, "Stop must not restart under a held key");
    }
}
