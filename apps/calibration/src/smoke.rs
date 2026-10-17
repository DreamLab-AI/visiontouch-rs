//! Headless end-to-end run of every step against the simulated plant.
//!
//! This drives the production [`Engine`] (controller, protocol, evidence and the real
//! encoder and guard code) through the whole unlock ladder with a simulated plant of
//! gain 0.6 and weight 0.3 N, writes evidence into a directory, and verifies it.
//! No hardware is opened. `--smoke-test` is a thin wrapper around [`run`].
use crate::evidence::{self, AttemptRecord, SessionRecord};
use crate::protocol::{Axis, Mark, Reading, Rung, Sign, StepKind, StepRequest, Unit};
use crate::sim::{Fixture, SimHw, SimPlant};
use crate::worker::{Command, Engine};
use std::path::{Path, PathBuf};
use std::time::Duration;
use visiontouch_effects::Kind;

/// What the smoke run proved.
#[derive(Debug)]
pub struct Report {
    /// Attempts written.
    pub attempts: usize,
    /// Steps unlocked at the end.
    pub unlocked: Vec<StepKind>,
    /// Evidence directory.
    pub dir: PathBuf,
    /// One line per step run.
    pub lines: Vec<String>,
}

struct Rig {
    e: Engine<SimHw>,
    lines: Vec<String>,
}
impl Rig {
    fn plant(&mut self) -> &mut SimPlant {
        &mut self.e.hardware_mut().expect("connected").plant
    }
    fn ticks(&mut self, n: usize) -> Result<(), String> {
        for _ in 0..n {
            self.e.step(true, 0.)?;
        }
        Ok(())
    }
    fn settle(&mut self, secs: f64) -> Result<(), String> {
        let dt = self.e.hardware_mut().expect("connected").dt.as_secs_f64();
        self.ticks((secs / dt).ceil() as usize)
    }
    /// Teleport the pen and float it there, at rest.
    fn place(&mut self, p: [f64; 3]) -> Result<(), String> {
        let plant = self.plant();
        plant.teleport(p);
        plant.float();
        self.settle(0.6)
    }
    fn start(&mut self, req: StepRequest) -> Result<(), String> {
        self.e.handle(Command::Start(req));
        if self.e.active().is_none() {
            return Err(format!("{} refused: {}", req.label(), self.e.status()));
        }
        Ok(())
    }
    fn run_step(&mut self, req: StepRequest) -> Result<String, String> {
        self.start(req)?;
        for _ in 0..600_000 {
            self.e.step(true, 0.)?;
            if self.e.active().is_none() {
                let s = self.e.snapshot().last_result.ok_or("no result")?;
                let line = format!("{}: {} ({})", s.label, s.reason, s.verdict);
                self.lines.push(line.clone());
                return Ok(s.reason);
            }
        }
        Err(format!("{} never ended", req.label()))
    }
    fn expect_normal(&mut self, req: StepRequest) -> Result<(), String> {
        let reason = self.run_step(req)?;
        let normal = reason == "completed" || reason == "lift-off";
        normal
            .then_some(())
            .ok_or_else(|| format!("{} ended with {reason}: {}", req.label(), self.e.status()))
    }
    fn gauge(&mut self, value: f64) {
        let id = self.e.snapshot().last_result.map_or(0, |r| r.attempt_id);
        self.e.handle(Command::SubmitReading {
            attempt_id: id,
            readings: vec![Reading {
                label: "dwell".into(),
                value,
                unit: Unit::N,
            }],
        });
    }
}

fn check(cond: bool, what: &str) -> Result<(), String> {
    cond.then_some(())
        .ok_or_else(|| format!("smoke assertion failed: {what}"))
}

/// Run the whole ladder against the simulator, writing evidence below `root`.
pub fn run(root: &Path) -> Result<Report, String> {
    let mut r = Rig {
        e: Engine::new(root.to_path_buf(), true),
        lines: Vec::new(),
    };
    r.e.connect(SimHw::new(SimPlant::default()));
    r.settle(0.6)?;
    check(
        r.e.snapshot().session.unlocked() == vec![StepKind::Backdrive, StepKind::Liveness],
        "fresh session unlocks only backdrive and liveness",
    )?;

    // Backdrive: motors stay off.
    r.start(StepRequest::Backdrive)?;
    r.ticks(100)?;
    check(!r.e.snapshot().feedback, "backdrive keeps output off")?;
    r.e.handle(Command::Stop);
    r.ticks(2)?;
    r.gauge(0.05);

    // Everything above liveness refuses before registration.
    let refused = r.start(StepRequest::SignPulse { axis: Axis::X });
    check(
        refused.is_err() && r.e.status().contains("Locked"),
        "sign pulse refused with a named prerequisite",
    )?;

    // Liveness on all three channels, unregistered.
    for channel in 0..3 {
        r.expect_normal(StepRequest::Liveness { channel })?;
    }
    check(
        r.e.session.liveness == [Some(true); 3],
        "liveness passes on all channels",
    )?;

    // Registration, return-to-reset, gravity drop, ruler.
    r.settle(1.0)?;
    r.e.handle(Command::RegisterReset);
    check(r.e.snapshot().registered, "registration captured")?;
    r.plant().set_hand([0.04, 0., 0.]);
    r.settle(2.5)?;
    r.plant().set_hand([0.; 3]);
    r.settle(4.0)?;
    r.e.handle(Command::CaptureStill(Mark::ReturnToReset));
    check(
        r.e.session.checks.return_ok() == Some(true),
        "return to reset within 2 mm",
    )?;
    r.e.handle(Command::ArmGravityDrop);
    check(
        r.e.snapshot().gravity_armed,
        "gravity drop armed on request",
    )?;
    r.plant().fixture = Fixture::Dropped { floor_y: -0.02 };
    r.settle(1.0)?;
    check(
        r.e.session.checks.gravity == Some(true),
        "gravity drop confirms -Y",
    )?;
    for axis in Axis::ALL {
        r.place([0.; 3])?;
        r.e.handle(Command::CaptureStill(Mark::RulerStart));
        let mut p = [0.; 3];
        p[axis.index()] = 0.05;
        r.place(p)?;
        r.e.handle(Command::CaptureStill(Mark::RulerEnd));
        r.e.handle(Command::SubmitRuler {
            axis,
            measured_mm: 50.,
        });
    }
    check(
        r.e.session.checks.ruler_ok(),
        "ruler sign and off-axis for X, Y, Z",
    )?;

    // Sign pulses.
    for axis in Axis::ALL {
        r.place([0.; 3])?;
        r.expect_normal(StepRequest::SignPulse { axis })?;
    }
    check(r.e.session.sign == [Some(true); 3], "sign pulses pass")?;

    // Ladder: 1.0 N is refused until the 0.5 N rung is cleared on X, Y and Z.
    let rungs = [
        (Axis::Y, Sign::Neg),
        (Axis::X, Sign::Pos),
        (Axis::Z, Sign::Pos),
    ];
    let ladder = |r: &mut Rig, rung: Rung, axis: Axis, sign: Sign| -> Result<(), String> {
        r.place([0.; 3])?;
        r.plant().brace();
        r.settle(0.3)?;
        r.expect_normal(StepRequest::Ladder { axis, sign, rung })?;
        let m = r.plant().brace_peak_n;
        r.gauge(m);
        Ok(())
    };
    for rung in [Rung::N0_1, Rung::N0_25] {
        for (a, s) in rungs {
            ladder(&mut r, rung, a, s)?;
        }
    }
    let refused = r.start(StepRequest::Ladder {
        axis: Axis::Y,
        sign: Sign::Neg,
        rung: Rung::N1_0,
    });
    check(
        refused.is_err() && r.e.status().contains("0.5 N rung"),
        "1.0 N refused before the 0.5 N rung",
    )?;
    for (a, s) in rungs {
        ladder(&mut r, Rung::N0_5, a, s)?;
    }
    for (a, s) in rungs {
        ladder(&mut r, Rung::N1_0, a, s)?;
    }
    ladder(&mut r, Rung::N2_0, Axis::Y, Sign::Neg)?;
    let u = r.e.snapshot().unlocked;
    check(
        u.contains(&StepKind::Ladder(Rung::N2_0))
            && u.contains(&StepKind::Thermal)
            && u.contains(&StepKind::WallRamp),
        "1.0 N rungs unlock 2.0 N, wall and thermal",
    )?;

    // Effects at the 1.0 N cap.
    r.place([0.01, 0.01, 0.01])?;
    r.start(StepRequest::Effect { kind: Kind::Cube })?;
    r.ticks(800)?;
    r.e.handle(Command::Stop);
    r.ticks(2)?;
    check(r.e.active().is_none(), "effect stops on request")?;

    // Lift: bare, then 50 g.
    for (mass, hung) in [(None, 0.), (Some(50.), 0.490_33)] {
        r.place([0.; 3])?;
        r.plant().fixture = Fixture::Stack {
            floor_y: 0.,
            hung_n: hung,
        };
        r.expect_normal(StepRequest::Lift { mass_g: mass })?;
    }
    let gain = r.e.session.lift.gain().ok_or("no lift gain")?;
    check(
        (0.5..0.7).contains(&gain),
        // The plant delivers 0.6 N per commanded newton. Lift-off of a hung mass needs
        // (F_m - F_0) commanded newtons to deliver m g extra, so g = m g / (F_m - F_0)
        // is delivered per commanded: 0.6, equal to the ladder slope, not its reciprocal.
        &format!(
            "lift gain is about 0.6 (delivered per commanded), got {gain} from {:?}",
            r.e.session.lift.entries
        ),
    )?;

    // Wall ramp, lightly pressed 1 mm in.
    r.place([0.; 3])?;
    r.plant().set_hand([0., -0.001, 0.]);
    r.settle(1.0)?;
    r.expect_normal(StepRequest::WallRamp)?;

    // Thermal, braced, on a coarse 20 ms tick, with a Confirm whenever one falls due.
    r.place([0.; 3])?;
    r.plant().brace();
    r.e.hardware_mut().ok_or("no device")?.dt = Duration::from_millis(20);
    r.start(StepRequest::Thermal)?;
    let id = r.e.snapshot().attempt_id.ok_or("no attempt id")?;
    let mut minute = 0;
    for _ in 0..40_000 {
        if r.e.confirm_in_s().is_some_and(|s| s <= 0.) {
            r.e.handle(Command::Confirm);
        }
        let elapsed = r.e.snapshot().elapsed_s;
        if elapsed >= f64::from(minute) * 60. && minute <= 10 {
            r.e.handle(Command::SubmitReading {
                attempt_id: id,
                readings: vec![Reading {
                    label: "casing".into(),
                    value: 24. + 0.3 * f64::from(minute),
                    unit: Unit::C,
                }],
            });
            minute += 1;
        }
        r.e.step(false, 0.)?;
        if r.e.active().is_none() {
            break;
        }
    }
    let last = r.e.snapshot().last_result.ok_or("no thermal result")?;
    r.lines.push(format!(
        "{}: {} ({})",
        last.label, last.reason, last.verdict
    ));
    check(last.normal, "thermal completes with Confirm and no hold")?;
    r.e.hardware_mut().ok_or("no device")?.dt = Duration::from_micros(2400);

    // Evidence on disk: every JSON parses, every CSV hash matches.
    let dir = r.e.session_dir().cloned().ok_or("no evidence directory")?;
    let session: SessionRecord = serde_json::from_slice(
        &std::fs::read(dir.join("session.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    check(
        session.schema == 2 && !session.unlock_history.is_empty(),
        "session.json written with unlock history",
    )?;
    let mut attempts = 0;
    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name == "session.json" || !name.ends_with(".json") || name == "coverage.json" {
            continue;
        }
        let rec: AttemptRecord =
            serde_json::from_slice(&std::fs::read(&path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        let csv = std::fs::read(dir.join(&rec.csv.file)).map_err(|e| e.to_string())?;
        check(
            evidence::sha256_hex(&csv) == rec.csv.sha256,
            "csv hash matches",
        )?;
        check(
            rec.schema == 2 && rec.claim == evidence::CLAIM,
            "attempt schema",
        )?;
        attempts += 1;
    }
    check(
        attempts == session.attempts.len() && attempts >= 25,
        "every attempt has json and csv",
    )?;

    let unlocked = r.e.snapshot().unlocked;
    // A reconnect forgets everything.
    r.e.connect(SimHw::new(SimPlant::default()));
    check(
        r.e.snapshot().unlocked == vec![StepKind::Backdrive, StepKind::Liveness],
        "reconnect resets the ladder",
    )?;
    Ok(Report {
        attempts,
        unlocked,
        dir,
        lines: r.lines,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_ladder_against_the_simulator() {
        let root =
            std::env::temp_dir().join(format!("haptics-cal-smoke-{}", evidence::new_session_id()));
        let report = run(&root).unwrap_or_else(|e| panic!("{e}"));
        assert!(report.unlocked.contains(&StepKind::Thermal));
        assert!(report.attempts >= 25);
        let _ = std::fs::remove_dir_all(root);
    }
}
