//! Local, user-private JSON interface. Socket I/O never runs on the servo/UI thread.
//! P-0067: existing UI intent only; no synthetic hold or direct actuator demand.
use crate::{
    App,
    protocol::{Axis, Mark, Reading, Rung, Sign, StepRequest},
    worker::Command,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    fs,
    io::{self, Read, Write},
    os::unix::{
        fs::{DirBuilderExt, MetadataExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const MAX_REQUEST: usize = 8192;
const APPLY_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Status {},
    Stop {},
    Connect {},
    Disconnect {},
    View {
        bench: bool,
    },
    GuideNext {},
    RegisterReference {
        #[serde(default)]
        replace: bool,
    },
    Capture {
        mark: CaptureMark,
    },
    ArmGravity {},
    SubmitRuler {
        axis: Axis,
        measured_mm: f64,
    },
    Observe {
        attempt_id: u64,
        felt: Option<bool>,
        heard: Option<bool>,
        #[serde(default)]
        note: String,
    },
    Readings {
        attempt_id: u64,
        readings: Vec<OperatorReading>,
    },
    Coverage {
        enabled: bool,
    },
    ExportCoverage {},
    StartBackdrive {},
    Prepare {
        test: Test,
    },
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorReading {
    label: String,
    value: f64,
    unit: crate::protocol::Unit,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureMark {
    Return,
    RulerStart,
    RulerEnd,
    EffectCentre,
}
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Test {
    Liveness { channel: usize },
    SignPulse { axis: Axis },
    Ladder { axis: Axis, sign: Sign, rung_n: f64 },
    Lift { mass_g: Option<f64> },
    Effect { name: String },
    WallRamp {},
}
impl Test {
    fn request(self) -> Result<StepRequest, String> {
        Ok(match self {
            Self::Liveness { channel } => StepRequest::Liveness { channel },
            Self::SignPulse { axis } => StepRequest::SignPulse { axis },
            Self::Ladder { axis, sign, rung_n } => StepRequest::Ladder {
                axis,
                sign,
                rung: Rung::ALL
                    .into_iter()
                    .find(|r| (r.newtons() - rung_n).abs() < 1e-9)
                    .ok_or("Unknown ladder rung")?,
            },
            Self::Lift { mass_g } => {
                if mass_g.is_some_and(|m| !m.is_finite() || m <= 0. || m > 100.) {
                    return Err("Mass must be positive and at most 100 g".into());
                }
                StepRequest::Lift { mass_g }
            }
            Self::Effect { name } => StepRequest::Effect {
                kind: visiontouch_effects::Kind::ALL
                    .into_iter()
                    .find(|k| k.name().eq_ignore_ascii_case(&name))
                    .ok_or("Unknown effect name")?,
            },
            Self::WallRamp {} => StepRequest::WallRamp,
        })
    }
}

struct Job {
    request: Request,
    deadline: Instant,
    reply: SyncSender<Value>,
}
pub struct Server {
    pub path: PathBuf,
    jobs: Receiver<Job>,
    quit: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
fn private_directory(path: &Path) -> io::Result<()> {
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    let m = fs::symlink_metadata(path)?;
    let uid = fs::metadata("/proc/self")?.uid();
    if !m.is_dir() || m.uid() != uid || m.mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Control directory must be a user-owned private directory (0700)",
        ));
    }
    Ok(())
}
impl Server {
    pub fn start() -> io::Result<Self> {
        let uid = fs::metadata("/proc/self")?.uid();
        let dir = std::env::var_os("XDG_RUNTIME_DIR")
            .map(|p| PathBuf::from(p).join("visiontouch-studio"))
            .unwrap_or_else(|| std::env::temp_dir().join(format!("visiontouch-studio-{uid}")));
        private_directory(&dir)?;
        Self::bind(dir.join(format!("{}.sock", std::process::id())))
    }
    fn bind(path: PathBuf) -> io::Result<Self> {
        let listener = UnixListener::bind(&path)?; // Never unlink somebody else's socket.
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        listener.set_nonblocking(true)?;
        let (tx, jobs) = mpsc::sync_channel(8);
        let quit = Arc::new(AtomicBool::new(false));
        let stop = quit.clone();
        let thread = thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let response = serve(&mut stream, &tx);
                        let _ = stream.set_write_timeout(Some(Duration::from_millis(250)));
                        if let Ok(bytes) = serde_json::to_vec(&response) {
                            let _ = stream.write_all(&bytes);
                            let _ = stream.write_all(b"\n");
                        }
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10))
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            path,
            jobs,
            quit,
            thread: Some(thread),
        })
    }
    /// Runs only on the UI thread. Expired requests never mutate the interface.
    pub fn pump(&self, app: &mut App) {
        for _ in 0..4 {
            let Ok(job) = self.jobs.try_recv() else {
                break;
            };
            let answer = if Instant::now() > job.deadline {
                error("Request expired before application; no action taken")
            } else {
                match apply(app, job.request) {
                    Ok(disposition) => {
                        json!({"ok": true, "disposition": disposition, "state": state(app)})
                    }
                    Err(e) => error(&e),
                }
            };
            let _ = job.reply.try_send(answer);
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.quit.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        let _ = fs::remove_file(&self.path);
    }
}
fn error(message: &str) -> Value {
    json!({"ok": false, "error": message})
}
fn read_request(stream: &mut UnixStream) -> io::Result<Request> {
    let end = Instant::now() + Duration::from_millis(500);
    let mut bytes = Vec::new();
    loop {
        let left = end.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Request read deadline exceeded",
            ));
        }
        stream.set_read_timeout(Some(left))?;
        let mut b = [0u8; 1];
        if stream.read(&mut b)? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Expected newline-terminated JSON",
            ));
        }
        if b[0] == b'\n' {
            break;
        }
        bytes.push(b[0]);
        if bytes.len() > MAX_REQUEST {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Request exceeds 8192 bytes",
            ));
        }
    }
    serde_json::from_slice(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))
}
fn serve(stream: &mut UnixStream, jobs: &SyncSender<Job>) -> Value {
    let request = match read_request(stream) {
        Ok(r) => r,
        Err(e) => return error(&e.to_string()),
    };
    let (reply, rx) = mpsc::sync_channel(1);
    if jobs
        .try_send(Job {
            request,
            deadline: Instant::now() + APPLY_TIMEOUT,
            reply,
        })
        .is_err()
    {
        return error("Control queue unavailable");
    }
    rx.recv_timeout(Duration::from_secs(2))
        .unwrap_or_else(|_| error("UI did not reply; inspect status before retrying a mutation"))
}
fn idle(app: &App) -> Result<(), String> {
    if app.snapshot.feedback || app.snapshot.active.is_some() || app.pending.is_some() {
        Err("Stop the active or prepared test first".into())
    } else {
        Ok(())
    }
}
fn stable(app: &App) -> Result<(), String> {
    idle(app)?;
    if !app.snapshot.connected {
        return Err("Connect first".into());
    }
    if !app.snapshot.stable {
        return Err("Hold the pen still first".into());
    }
    Ok(())
}
/// Apply an allowlisted UI action, never an actuator command.
pub fn apply(app: &mut App, req: Request) -> Result<&'static str, String> {
    app.guide.sync(&app.snapshot);
    match req {
        Request::Status {} => return Ok("observed"),
        Request::Stop {} => app.stop(),
        Request::View { bench } => {
            app.stop();
            app.guide.bench = bench;
            return Ok("applied_to_ui");
        }
        Request::GuideNext {} => {
            idle(app)?;
            app.guide.advance(&app.snapshot)?;
            return Ok("applied_to_ui");
        }
        Request::Connect {} => {
            idle(app)?;
            if app.snapshot.connected {
                return Err("Already connected".into());
            }
            app.send(Command::Connect);
        }
        Request::Disconnect {} => {
            app.stop();
            app.send(Command::Disconnect);
        }
        Request::RegisterReference { replace } => {
            stable(app)?;
            if app.snapshot.registered && !replace {
                return Err(
                    "Reference exists; use replace:true to replace it and clear checks".into(),
                );
            }
            app.guide.reference_requested();
            app.send(Command::RegisterReset);
        }
        Request::Capture { mark } => {
            stable(app)?;
            if !app.snapshot.registered {
                return Err("Save a reference first".into());
            }
            let mark = match mark {
                CaptureMark::Return => {
                    app.guide.capture_requested(&app.snapshot)?;
                    Mark::ReturnToReset
                }
                CaptureMark::RulerStart => Mark::RulerStart,
                CaptureMark::RulerEnd => Mark::RulerEnd,
                CaptureMark::EffectCentre => Mark::EffectCentre,
            };
            app.send(Command::CaptureStill(mark));
        }
        Request::ArmGravity {} => {
            stable(app)?;
            app.send(Command::ArmGravityDrop);
        }
        Request::SubmitRuler { axis, measured_mm } => {
            idle(app)?;
            if !measured_mm.is_finite() {
                return Err("Ruler value must be finite".into());
            }
            app.send(Command::SubmitRuler { axis, measured_mm });
        }
        Request::Observe {
            attempt_id,
            felt,
            heard,
            note,
        } => {
            idle(app)?;
            app.send(Command::SubmitObservation {
                attempt_id,
                felt,
                heard,
                note,
            });
        }
        Request::Readings {
            attempt_id,
            readings,
        } => {
            idle(app)?;
            if readings.iter().any(|r| !r.value.is_finite()) {
                return Err("Readings must be finite".into());
            }
            app.send(Command::SubmitReading {
                attempt_id,
                readings: readings
                    .into_iter()
                    .map(|r| Reading {
                        label: r.label,
                        value: r.value,
                        unit: r.unit,
                    })
                    .collect(),
            });
        }
        Request::Coverage { enabled } => {
            idle(app)?;
            app.send(Command::Coverage(enabled));
        }
        Request::ExportCoverage {} => {
            idle(app)?;
            app.send(Command::ExportCoverage);
        }
        Request::StartBackdrive {} => {
            idle(app)?;
            app.snapshot.session.check(StepRequest::Backdrive)?;
            app.request(StepRequest::Backdrive);
        }
        Request::Prepare { test } => {
            stable(app)?;
            if app.held {
                return Err("Release the local hold before preparing a test".into());
            }
            let req = test.request()?;
            app.snapshot.session.check(req)?;
            app.request(req);
            // Ignore even a Space press concurrent with this request. The operator
            // must release first, then give a fresh local hold after preparation.
            app.blocked = true;
            app.hold_since = None;
            return Ok("prepared_requires_fresh_local_hold");
        }
    }
    Ok("dispatched_to_worker_check_status")
}
pub fn state(app: &App) -> Value {
    let s = &app.snapshot;
    json!({
        "api_version": 1, "pid": std::process::id(), "simulated": app.is_demo(),
        "snapshot_age_ms": app.snapshot_at.map(|t| t.elapsed().as_millis() as u64),
        "connected": s.connected, "output_armed": s.feedback, "registered": s.registered,
        "reset_counts": s.registration.map(|r| r.reset_counts), "stable": s.stable,
        "counts": s.continuous, "position_m": s.position, "model_velocity_m_s": s.velocity,
        "motor_command_counts": s.telemetry.applied_motor_offsets,
        "rate_hz": s.rate_hz, "held_locally": app.held,
        "active": s.active.map(|r| r.label()), "pending": app.pending.map(|r| r.label()),
        "elapsed_s": s.elapsed_s, "remaining_s": s.remaining_s,
        "limits": {"model_force_n": s.limits.force_n, "dac_counts": s.limits.dac_counts, "slew_n_s": s.limits.slew_n_s},
        "last_result": s.last_result.as_ref().map(|r| json!({"attempt_id": r.attempt_id, "label": r.label, "reason": r.reason, "verdict": r.verdict, "duration_s": r.duration_s})),
        "guide": app.guide.state(), "checks": s.session.checks,
        "liveness": s.session.liveness, "sign": s.session.sign,
        "rungs": s.session.rungs, "status": s.status, "notice": app.notice,
        "evidence_dir": s.session_dir, "log": s.log,
        "physical_hold_required": true,
        "actions": ["status", "stop", "connect", "disconnect", "view", "guide_next", "register_reference", "capture", "arm_gravity", "submit_ruler", "observe", "readings", "coverage", "export_coverage", "start_backdrive", "prepare"]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn demo() -> App {
        let mut a = App::new(true, false);
        if let crate::Backend::Demo(e) = &mut a.backend {
            e.step(false, 0.).unwrap();
            a.snapshot = e.snapshot();
        }
        a.snapshot.stable = true;
        a
    }
    #[test]
    fn refuses_holds_raw_demands_and_unknown_fields() {
        for body in [
            r#"{"action":"hold","value":true}"#,
            r#"{"action":"prepare","test":{"kind":"thermal"}}"#,
            r#"{"action":"stop","force":999}"#,
        ] {
            assert!(serde_json::from_str::<Request>(body).is_err(), "{body}");
        }
    }
    #[test]
    fn prepare_cannot_arm_and_navigation_cancels_it() {
        let mut a = demo();
        apply(
            &mut a,
            Request::Prepare {
                test: Test::Liveness { channel: 0 },
            },
        )
        .unwrap();
        assert!(a.pending.is_some());
        assert!(!a.snapshot.feedback);
        a.hold(true);
        assert!(!a.held, "API preparation must require a fresh local hold");
        apply(&mut a, Request::View { bench: true }).unwrap();
        assert!(a.pending.is_none());
        assert!(a.guide.bench);
    }
    #[test]
    fn refuses_locked_force_test_and_reference_replacement() {
        let mut a = demo();
        assert!(
            apply(
                &mut a,
                Request::Prepare {
                    test: Test::SignPulse { axis: Axis::Y }
                }
            )
            .is_err()
        );
        a.snapshot.registered = true;
        assert!(apply(&mut a, Request::RegisterReference { replace: false }).is_err());
    }
    #[test]
    fn socket_permissions_roundtrip_and_expiry() {
        let dir = std::env::temp_dir().join(format!(
            "vt-control-test-{}",
            crate::evidence::utc_now().replace(':', "")
        ));
        // Process IDs keep simultaneous test processes isolated.
        let dir = dir.with_extension(format!("{}", std::process::id()));
        private_directory(&dir).unwrap();
        let server = Server::bind(dir.join("test.sock")).unwrap();
        assert_eq!(fs::metadata(&server.path).unwrap().mode() & 0o777, 0o600);
        let path = server.path.clone();
        let client = thread::spawn(move || {
            let mut s = UnixStream::connect(path).unwrap();
            s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            s.write_all(b"{\"action\":\"status\"}\n").unwrap();
            let mut result = String::new();
            s.read_to_string(&mut result).unwrap();
            serde_json::from_str::<Value>(&result).unwrap()
        });
        let mut a = demo();
        let deadline = Instant::now() + Duration::from_secs(3);
        while !client.is_finished() && Instant::now() < deadline {
            server.pump(&mut a);
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(client.join().unwrap()["state"]["simulated"], true);
        let (tx, rx) = mpsc::sync_channel(1);
        let (jobs_tx, jobs_rx) = mpsc::sync_channel(1);
        jobs_tx
            .send(Job {
                request: Request::View { bench: true },
                deadline: Instant::now() - Duration::from_secs(1),
                reply: tx,
            })
            .unwrap();
        let mut expired_server = Server {
            path: dir.join("unused.sock"),
            jobs: jobs_rx,
            quit: Arc::new(AtomicBool::new(false)),
            thread: None,
        };
        expired_server.pump(&mut a);
        assert_eq!(rx.recv().unwrap()["ok"], false);
        assert!(!a.guide.bench);
        expired_server.thread = None;
        drop(server);
        drop(expired_server);
        fs::remove_dir(dir).unwrap();
    }
}
