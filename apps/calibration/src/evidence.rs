//! Evidence files (schema 2): a directory per session, a CSV and a JSON per attempt.
//!
//! Commands are model values; delivered force exists only in operator readings. Every
//! writer takes an [`OutputOff`] token, which can only be obtained while feedback is
//! disabled, so a file write can never happen with the motors armed.
use crate::controller::{TickInput, TickOutput};
use crate::protocol::{Reading, StepRequest};
use crate::registration::Checks;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use visiontouch_device::{
    COMMAND_TIMEOUT, Demand, ForceLimits, MAX_DAC_OFFSET, MAX_FORCE_N, MAX_SLEW_N_S, Registration,
    UNIT_DERATING,
};

/// Evidence schema version.
pub const SCHEMA: u32 = 2;
/// The claim restated in every attempt file.
pub const CLAIM: &str = "Commands are model values; delivered force only from operator readings";

/// Proof that motor output is off. Required by every writer.
#[derive(Debug)]
pub struct OutputOff(());
impl OutputOff {
    /// Obtain the token; refused while feedback is enabled.
    pub fn confirm(feedback_enabled: bool) -> Result<Self, String> {
        if feedback_enabled {
            Err("Refusing to write evidence while motor output is armed.".into())
        } else {
            Ok(Self(()))
        }
    }
}

/// Automatic measurements of an attempt.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Auto {
    /// Lock-in amplitude of the best encoder, counts.
    pub lockin_amplitude_counts: Option<f64>,
    /// Its noise estimate, counts.
    pub noise_counts: Option<f64>,
    /// Which encoder responded best.
    pub lockin_encoder: Option<usize>,
    /// Lock-in exceeded five times noise on some encoder.
    pub lockin_pass: Option<bool>,
    /// The sign pulse pair moved the pen the expected way.
    pub sign_agrees: Option<bool>,
    /// Norm of the force the guard applied on the tick before lift-off, newtons (a
    /// model value, not a measurement).
    pub liftoff_force_n: Option<f64>,
    /// Last wall stiffness that held without oscillation, N/mm.
    pub k_marginal_n_mm: Option<f64>,
    /// Oscillation stopped the attempt.
    pub oscillation: bool,
}

/// Per-tick summary of an attempt.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TelemetrySummary {
    /// Servo ticks recorded.
    pub samples: u64,
    /// Longest sample interval, milliseconds.
    pub max_interval_ms: f64,
    /// Ticks where the guard reported an expired request.
    pub stale_ticks: u64,
    /// Ticks where the force cap bound.
    pub force_capped_ticks: u64,
    /// Ticks where the DAC cap bound.
    pub dac_capped_ticks: u64,
    /// Largest absolute channel offset per channel.
    pub max_abs_offsets: [u16; 3],
}

/// Operator entries.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Operator {
    /// Labelled readings.
    pub readings: Vec<Reading>,
    /// Felt something.
    pub felt: Option<bool>,
    /// Heard something.
    pub heard: Option<bool>,
    /// Free note.
    pub note: String,
}

/// Verdict block.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VerdictRecord {
    /// What was concluded. Ladder: `pending-reading`, `aborted`, then `pass`, `over`,
    /// `under` or `none` once read. Liveness, sign and thermal: `auto-pass`, `auto-fail`,
    /// `pass`, `fail`, `inconclusive` or `completed`/`stopped`. Lift: `liftoff` or
    /// `no-liftoff`. Wall: `oscillation` or `stable`. Others: `recorded`.
    pub kind: String,
    /// True for a pass; absent while pending.
    pub pass: Option<bool>,
    /// The rule applied.
    pub rule: String,
    /// Steps this verdict unlocked.
    pub unlocks: Vec<String>,
}

/// Stop block.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StopRecord {
    /// Stop reason name.
    pub reason: String,
    /// Detail text.
    pub detail: String,
}

/// SDK ceilings recorded with every attempt.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SdkRecord {
    /// Force ceiling, newtons.
    pub max_force_n: f64,
    /// DAC ceiling, counts.
    pub max_dac: i16,
    /// Slew ceiling, N/s.
    pub slew_n_s: f64,
    /// Request timeout, milliseconds.
    pub timeout_ms: u64,
    /// Derating factor.
    pub unit_derating: f64,
}

/// Software identity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Software {
    /// `git rev-parse HEAD` at run time, or "unknown".
    pub git_commit: String,
    /// SDK ceilings.
    pub sdk: SdkRecord,
}
impl Software {
    /// Describe the running software.
    pub fn current() -> Self {
        Self {
            git_commit: git_commit(),
            sdk: SdkRecord {
                max_force_n: MAX_FORCE_N,
                max_dac: MAX_DAC_OFFSET,
                slew_n_s: MAX_SLEW_N_S,
                timeout_ms: COMMAND_TIMEOUT.as_millis() as u64,
                unit_derating: UNIT_DERATING,
            },
        }
    }
}

/// Limits in force during an attempt.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LimitsRecord {
    /// Force cap, newtons.
    pub force_n: f64,
    /// Slew cap, N/s.
    pub slew_n_s: f64,
    /// DAC cap, counts.
    pub dac_counts: i16,
}
impl From<ForceLimits> for LimitsRecord {
    fn from(l: ForceLimits) -> Self {
        Self {
            force_n: l.force_n,
            slew_n_s: l.slew_n_s,
            dac_counts: l.dac_counts,
        }
    }
}

/// Registration block.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RegistrationRecord {
    /// Reset-pose counters.
    pub reset_counts: [i64; 6],
    /// When registration was captured.
    pub captured_utc: String,
}
impl RegistrationRecord {
    /// Build from an SDK registration.
    pub fn new(r: Registration, captured_utc: String) -> Self {
        Self {
            reset_counts: r.reset_counts,
            captured_utc,
        }
    }
}

/// CSV reference.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CsvRef {
    /// File name within the session directory.
    pub file: String,
    /// SHA-256 of the file, lower-case hex.
    pub sha256: String,
}

/// One attempt's JSON record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttemptRecord {
    /// Schema number (2).
    pub schema: u32,
    /// Always `haptics-verification-attempt`.
    pub kind: String,
    /// Session UUID.
    pub session_id: String,
    /// Attempt number within the session.
    pub attempt_id: u64,
    /// Step slug.
    pub step: String,
    /// Step parameters.
    pub params: serde_json::Value,
    /// Start time.
    pub started_utc: String,
    /// Duration, seconds.
    pub duration_s: f64,
    /// Software identity.
    pub software: Software,
    /// Limits in force.
    pub limits: LimitsRecord,
    /// Registration, if any.
    pub registration: Option<RegistrationRecord>,
    /// How it ended.
    pub stop: StopRecord,
    /// Tick summary.
    pub telemetry_summary: TelemetrySummary,
    /// Automatic measurements.
    pub auto: Auto,
    /// Operator entries.
    pub operator: Operator,
    /// Verdict.
    pub verdict: VerdictRecord,
    /// CSV file and hash.
    pub csv: CsvRef,
    /// The claim.
    pub claim: String,
}
impl AttemptRecord {
    /// A fresh record before the CSV is written.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session_id: &str,
        attempt_id: u64,
        req: StepRequest,
        started_utc: String,
        limits: ForceLimits,
        registration: Option<RegistrationRecord>,
        software: Software,
    ) -> Self {
        Self {
            schema: SCHEMA,
            kind: "haptics-verification-attempt".into(),
            session_id: session_id.into(),
            attempt_id,
            step: req.slug().into(),
            params: req.params(),
            started_utc,
            duration_s: 0.,
            software,
            limits: limits.into(),
            registration,
            stop: StopRecord {
                reason: "running".into(),
                detail: String::new(),
            },
            telemetry_summary: TelemetrySummary::default(),
            auto: Auto::default(),
            operator: Operator::default(),
            verdict: VerdictRecord {
                kind: "pending".into(),
                ..VerdictRecord::default()
            },
            csv: CsvRef::default(),
            claim: CLAIM.into(),
        }
    }
    /// Base file name without extension, for example `0007-ladder`.
    pub fn stem(&self) -> String {
        format!("{:04}-{}", self.attempt_id, self.step)
    }
}

/// Session record written as `session.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionRecord {
    /// Schema number (2).
    pub schema: u32,
    /// Always `haptics-verification-session`.
    pub kind: String,
    /// Session UUID.
    pub session_id: String,
    /// Connection time.
    pub connected_utc: String,
    /// Registration, if captured.
    pub registration: Option<RegistrationRecord>,
    /// Registration checks.
    pub checks: Checks,
    /// Unlock events in order.
    pub unlock_history: Vec<String>,
    /// Attempt stems written so far.
    pub attempts: Vec<String>,
    /// Lift-off forces and the gain derived from them (additive field, schema stays 2).
    #[serde(default)]
    pub lift: LiftRecord,
    /// Last wall stiffness that held without oscillation, N/mm (additive field).
    #[serde(default)]
    pub k_marginal_n_mm: Option<f64>,
    /// The claim.
    pub claim: String,
}

/// Lift results persisted in the session file.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LiftRecord {
    /// Bare lift-off force F_0, model newtons.
    pub f0_n: Option<f64>,
    /// `(mass_g, F_m)` per hung mass; bare is mass 0.
    pub entries: Vec<(f64, f64)>,
    /// Lift gain `m g / (F_m - F_0)`, averaged (spec 14 section 6.6).
    pub gain: Option<f64>,
}

/// CSV columns in order (spec 14 section 9).
pub fn csv_header() -> String {
    let mut c: Vec<String> = ["t_s", "interval_us", "io_us", "raw_hex"]
        .map(String::from)
        .to_vec();
    c.extend((0..6).map(|i| format!("c{i}")));
    c.extend((0..6).map(|i| format!("k{i}")));
    c.extend((0..3).map(|i| format!("r{i}")));
    for n in [
        "x_m",
        "y_m",
        "z_m",
        "vx_m_s",
        "vy_m_s",
        "vz_m_s",
        "demand_kind",
        "demand_x",
        "demand_y",
        "demand_z",
        "demand_ch0",
        "demand_ch1",
        "demand_ch2",
        "applied_x",
        "applied_y",
        "applied_z",
        "off0",
        "off1",
        "off2",
        "force_capped",
        "dac_capped",
        "slewing",
        "expired",
        "held",
        "ui_age_ms",
        "button",
        "target_x",
        "target_y",
        "target_z",
        "phase",
    ] {
        c.push(n.into());
    }
    c.join(",")
}

/// One CSV line for a servo tick.
pub fn csv_line(i: &TickInput, o: &TickOutput) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(400);
    let _ = write!(
        s,
        "{:.6},{},{},",
        o.elapsed_s,
        (i.interval_s * 1e6).round() as u64,
        i.io_us
    );
    for b in i.raw {
        let _ = write!(s, "{b:02x}");
    }
    for c in i.counts {
        let _ = write!(s, ",{c}");
    }
    for k in i.continuous {
        let _ = write!(s, ",{k}");
    }
    for n in 0..3 {
        match i.registered {
            Some(r) => {
                let _ = write!(s, ",{}", r[n]);
            }
            None => s.push(','),
        }
    }
    for p in i.position {
        let _ = write!(s, ",{p:.7}");
    }
    for v in o.velocity {
        let _ = write!(s, ",{v:.6}");
    }
    match o.demand {
        Demand::Force(f) => {
            let _ = write!(s, ",force,{:.6},{:.6},{:.6},,,", f[0], f[1], f[2]);
        }
        Demand::Channels(c) => {
            let _ = write!(s, ",channels,,,,{},{},{}", c[0], c[1], c[2]);
        }
    }
    let t = i.telemetry;
    let _ = write!(
        s,
        ",{:.6},{:.6},{:.6},{},{},{}",
        t.applied_force[0],
        t.applied_force[1],
        t.applied_force[2],
        t.applied_motor_offsets[0],
        t.applied_motor_offsets[1],
        t.applied_motor_offsets[2]
    );
    let _ = write!(
        s,
        ",{},{},{},{},{},{:.1},{}",
        u8::from(t.force_capped),
        u8::from(t.dac_capped),
        u8::from(t.slewing),
        u8::from(t.expired),
        u8::from(i.held),
        i.ui_age_ms,
        u8::from(i.button)
    );
    let _ = write!(
        s,
        ",{:.7},{:.7},{:.7},{}",
        o.target[0], o.target[1], o.target[2], o.phase
    );
    s
}

/// Buffered CSV rows. Nothing touches the disk until the motors are off.
#[derive(Debug, Default)]
pub struct Recorder {
    lines: Vec<String>,
    keep_every: usize,
    seen: usize,
}
impl Recorder {
    /// Keep one row in `keep_every` (1 keeps all).
    pub fn new(keep_every: usize) -> Self {
        Self {
            lines: Vec::new(),
            keep_every: keep_every.max(1),
            seen: 0,
        }
    }
    /// Offer a tick; the final tick of an attempt is always kept.
    pub fn push(&mut self, i: &TickInput, o: &TickOutput) {
        if self.seen.is_multiple_of(self.keep_every) || o.stop.is_some() {
            self.lines.push(csv_line(i, o));
        }
        self.seen += 1;
    }
    /// Rows kept.
    #[cfg(test)]
    pub fn rows(&self) -> usize {
        self.lines.len()
    }
    fn render(&self) -> String {
        let mut s = csv_header();
        s.push('\n');
        for l in &self.lines {
            s.push_str(l);
            s.push('\n');
        }
        s
    }
}

/// SHA-256 as lower-case hex.
pub fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Write the CSV and the JSON of a finished attempt; fills in `record.csv`.
pub fn write_attempt(
    _off: &OutputOff,
    dir: &Path,
    record: &mut AttemptRecord,
    rows: &Recorder,
) -> std::io::Result<()> {
    let stem = record.stem();
    let csv = rows.render();
    std::fs::write(dir.join(format!("{stem}.csv")), &csv)?;
    record.csv = CsvRef {
        file: format!("{stem}.csv"),
        sha256: sha256_hex(csv.as_bytes()),
    };
    rewrite_attempt(_off, dir, record)
}

/// Rewrite an attempt's JSON, for example after operator entries.
pub fn rewrite_attempt(
    _off: &OutputOff,
    dir: &Path,
    record: &AttemptRecord,
) -> std::io::Result<()> {
    let json = serde_json::to_vec_pretty(record).map_err(std::io::Error::other)?;
    std::fs::write(dir.join(format!("{}.json", record.stem())), json)
}

/// Write `session.json`.
pub fn write_session(_off: &OutputOff, dir: &Path, record: &SessionRecord) -> std::io::Result<()> {
    let json = serde_json::to_vec_pretty(record).map_err(std::io::Error::other)?;
    std::fs::write(dir.join("session.json"), json)
}

/// Create `<root>/<utc>-<session_id>/`.
pub fn create_session_dir(
    _off: &OutputOff,
    root: &Path,
    utc_compact: &str,
    session_id: &str,
) -> std::io::Result<PathBuf> {
    let dir = root.join(format!("{utc_compact}-{session_id}"));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// `git rev-parse HEAD`, or "unknown".
pub fn git_commit() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| s.len() >= 7 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .unwrap_or_else(|| "unknown".into())
}

/// Break Unix seconds into `(year, month, day, hour, minute, second)` UTC.
pub fn civil(unix: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = unix.div_euclid(86_400);
    let s = unix.rem_euclid(86_400) as u32;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d, s / 3600, s % 3600 / 60, s % 60)
}
/// Current UTC time as `2026-10-18T10:01:02Z`.
pub fn utc_now() -> String {
    iso(unix_now())
}
/// Format Unix seconds as ISO 8601 UTC.
pub fn iso(unix: i64) -> String {
    let (y, mo, d, h, mi, s) = civil(unix);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}
/// Format Unix seconds as `20261018T100102Z`, for directory names.
pub fn compact(unix: i64) -> String {
    iso(unix).replace(['-', ':'], "")
}
/// Current Unix seconds.
pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}
/// A random version-4 UUID string.
pub fn new_session_id() -> String {
    let mut b = [0_u8; 16];
    if getrandom::getrandom(&mut b).is_err() {
        let seed = format!("{:?}{}", std::time::SystemTime::now(), std::process::id());
        b.copy_from_slice(&Sha256::digest(seed.as_bytes())[..16]);
    }
    b[6] = b[6] & 0x0f | 0x40;
    b[8] = b[8] & 0x3f | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Axis, Rung, Sign, Unit};
    use std::time::Instant;
    use visiontouch_device::Telemetry;

    fn sample_tick(demand: Demand) -> (TickInput, TickOutput) {
        let i = TickInput {
            now: Instant::now(),
            interval_s: 0.0024,
            io_us: 90,
            held: true,
            ui_age_ms: 12.5,
            button: false,
            counts: [1, -2, 3, 4, 5, 6],
            continuous: [10, 20, 30, 40, 50, 60],
            registered: Some([1, 2, 3, 4, 5, 6]),
            position: [0.001, 0.002, 0.003],
            raw: [0xab; 16],
            telemetry: Telemetry::default(),
        };
        let o = TickOutput {
            demand,
            phase: "dwell",
            target: [0.; 3],
            elapsed_s: 1.5,
            stop: None,
            velocity: [0.; 3],
        };
        (i, o)
    }
    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("haptics-cal-{tag}-{}", new_session_id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn csv_header_matches_design_section_four() {
        let h = csv_header();
        let cols: Vec<&str> = h.split(',').collect();
        assert_eq!(cols.len(), 4 + 6 + 6 + 3 + 6 + 7 + 3 + 3 + 7 + 3 + 1);
        assert_eq!(&cols[..4], ["t_s", "interval_us", "io_us", "raw_hex"]);
        assert_eq!(&cols[4..10], ["c0", "c1", "c2", "c3", "c4", "c5"]);
        assert_eq!(&cols[10..16], ["k0", "k1", "k2", "k3", "k4", "k5"]);
        assert_eq!(&cols[16..19], ["r0", "r1", "r2"]);
        assert_eq!(cols.last(), Some(&"phase"));
        assert!(h.contains("demand_ch0,demand_ch1,demand_ch2,applied_x"));
        assert!(
            h.contains("force_capped,dac_capped,slewing,expired,held,ui_age_ms,button,target_x")
        );
    }

    #[test]
    fn every_csv_line_has_one_field_per_column() {
        let cols = csv_header().split(',').count();
        for d in [Demand::Force([0., -0.5, 0.]), Demand::Channels([80, 0, 0])] {
            let (i, o) = sample_tick(d);
            let line = csv_line(&i, &o);
            assert_eq!(line.split(',').count(), cols, "{line}");
            assert!(line.contains("abababababababababababababababab"));
        }
        let (mut i, o) = sample_tick(Demand::Force([0.; 3]));
        i.registered = None;
        assert_eq!(csv_line(&i, &o).split(',').count(), cols);
    }

    #[test]
    fn attempt_json_round_trips_and_hashes_the_csv() {
        let dir = temp_dir("rt");
        let req = StepRequest::Ladder {
            axis: Axis::Y,
            sign: Sign::Neg,
            rung: Rung::N0_5,
        };
        let mut rec = AttemptRecord::new(
            "sid",
            7,
            req,
            "2026-10-18T10:01:02Z".into(),
            ForceLimits::default(),
            Some(RegistrationRecord::new(
                Registration {
                    reset_counts: [0; 6],
                },
                "t".into(),
            )),
            Software::current(),
        );
        rec.operator.readings.push(Reading {
            label: "dwell".into(),
            value: 0.47,
            unit: Unit::N,
        });
        let mut rows = Recorder::new(1);
        let (i, o) = sample_tick(Demand::Force([0.; 3]));
        rows.push(&i, &o);
        let off = OutputOff::confirm(false).unwrap();
        write_attempt(&off, &dir, &mut rec, &rows).unwrap();
        assert_eq!(rec.csv.file, "0007-ladder.csv");
        let csv = std::fs::read(dir.join("0007-ladder.csv")).unwrap();
        assert_eq!(rec.csv.sha256, sha256_hex(&csv));
        assert_eq!(rec.csv.sha256.len(), 64);
        let back: AttemptRecord =
            serde_json::from_slice(&std::fs::read(dir.join("0007-ladder.json")).unwrap()).unwrap();
        assert_eq!(back, rec);
        assert_eq!(back.schema, 2);
        assert_eq!(back.params["rung_n"], 0.5);
        assert_eq!(back.params["axis"], "y");
        assert_eq!(back.params["sign"], "neg");
        assert!(!back.software.git_commit.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn sha256_matches_published_vectors() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn writing_is_refused_while_output_is_armed() {
        assert!(OutputOff::confirm(true).is_err());
        assert!(OutputOff::confirm(false).is_ok());
    }

    #[test]
    fn recorder_decimates_but_keeps_the_stopping_tick() {
        let mut r = Recorder::new(10);
        let (i, mut o) = sample_tick(Demand::Force([0.; 3]));
        for _ in 0..25 {
            r.push(&i, &o);
        }
        assert_eq!(r.rows(), 3);
        o.stop = Some(crate::controller::StopReason::Completed);
        r.push(&i, &o);
        assert_eq!(r.rows(), 4);
    }

    #[test]
    fn utc_formatting_matches_known_instants() {
        assert_eq!(iso(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso(1_000_000_000), "2001-09-09T01:46:40Z");
        assert_eq!(iso(1_800_000_000), "2027-01-15T08:00:00Z");
        assert_eq!(iso(951_782_400), "2000-02-29T00:00:00Z", "leap day");
        assert_eq!(compact(1_000_000_000), "20010909T014640Z");
    }

    #[test]
    fn session_ids_are_v4_uuids_and_commit_is_hex_or_unknown() {
        let id = new_session_id();
        assert_eq!(id.len(), 36);
        assert_eq!(&id[14..15], "4");
        assert!(matches!(&id[19..20], "8" | "9" | "a" | "b"));
        assert_ne!(id, new_session_id());
        let c = git_commit();
        assert!(c == "unknown" || c.len() >= 7);
    }

    #[test]
    fn session_directory_and_file() {
        let root = temp_dir("sess");
        let off = OutputOff::confirm(false).unwrap();
        let dir = create_session_dir(&off, &root, "20261018T100102Z", "abc").unwrap();
        assert!(dir.ends_with("20261018T100102Z-abc"));
        let rec = SessionRecord {
            schema: 2,
            kind: "haptics-verification-session".into(),
            session_id: "abc".into(),
            connected_utc: utc_now(),
            registration: None,
            checks: Checks::default(),
            unlock_history: vec!["Sign pulses unlocked by x".into()],
            attempts: vec![],
            lift: LiftRecord {
                f0_n: Some(0.5),
                entries: vec![(0., 0.5), (50., 1.25)],
                gain: Some(0.65),
            },
            k_marginal_n_mm: Some(0.7),
            claim: CLAIM.into(),
        };
        write_session(&off, &dir, &rec).unwrap();
        let back: SessionRecord =
            serde_json::from_slice(&std::fs::read(dir.join("session.json")).unwrap()).unwrap();
        assert_eq!(back, rec);
        let _ = std::fs::remove_dir_all(root);
    }
}
