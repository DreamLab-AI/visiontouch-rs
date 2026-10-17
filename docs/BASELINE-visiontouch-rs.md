---
title: BASELINE for visiontouch-rs
version: 1
verified_commit:
status: living governing document; amended only through docs/adr records
---

# BASELINE for visiontouch-rs

This is the living governing document for the repository. Its *Invariants* are
the compliance surface for every change; the ADR ledger in `docs/adr/` amends
it. Protocol facts live in `docs/spec/` and carry provenance rows in
`docs/clean-room/PROVENANCE.md`. History lives in `docs/history/` and is never
authority.

## 1. What this repository is

A lab evaluation, formally part of the VisionFlow research estate
(ADR-0006): a clean-room Rust host stack for a lawfully owned 1990s SensAble
PHANTOM 1.0A haptic arm on an IEEE-1284 EPP parallel port. It is a proof of
principle with no economic use. Outputs: an academic paper on the method, and
later a pose-and-button input for selecting and dragging nodes in the VisionClaw
graph. Product names are referential; the project is not affiliated with the
rights holder.

## 2. Method (ADR-0001)

Observation first. Facts come from the wire, from ppdev traces and from black-box
use of lawfully held software. Every fact in a spec has a provenance row with a
source class (PUBLIC, OWNER, OBSERVE, BLACKBOX, DESIGN, DECOMPILE). Decompilation
is a logged last resort and its findings never reach the paper. No vendor
artefact enters the tree. Spec precedes code; commit order is the evidence.

## 3. Architecture

| Layer | Crate or app | Responsibility |
|---|---|---|
| Transport | `crates/hal` | ppdev claim, software EPP cycles preserving INIT, 16-byte packet read, 8-byte motor packet with sequence acknowledgement, zero-only arming handshake, transport clamp `MAX_MOTOR_OFFSET` |
| SDK | `crates/device` | Six-counter continuous state, reset-pose registration, reference kinematics and Jacobian, `ForceGuard` (arm, expiry, vector cap, uniform DAC scaling, slew), `Demand` (force or raw channel), lifecycle with failed-shutdown precedence |
| Effects | `crates/effects` | Hardware-free force models (cube, plane, water, balloon, three-body, viscosity, detents) with tunable stiffness |
| Calibration Studio | `apps/calibration` | Registration, staged force verification with evidence, coverage cloud, demos; worker owns I/O, UI sends intent |
| Mass Lab | `apps/mass-lab` | Physics sandbox; servo-rate coupling descriptor evaluated per tick |
| Workspace viewer | `apps/workspace-viewer` | Offline Three.js reconstruction and replay |
| Tools | `tools/` | Port probe, reference-call recorder, bounded readout, audio analysis |
| Scripts | `scripts/` | Launchers, port-access helper, quality gate, ADR index |

## 4. Invariants

Each invariant names where it is enforced. A change that weakens one is an ADR.

1. **No vendor artefacts tracked.** `.gitignore` excludes `/vendor-original/`,
   `/disasm/`, binaries and installers; `scripts/quality-gate.sh` fails if any
   such file is tracked. (ADR-0001)
2. **Spec and provenance precede code.** A protocol behaviour appears in
   `docs/spec/` with a `P-NNNN` row before the Rust that uses it. (ADR-0001)
3. **Decompilation is logged first.** `docs/clean-room/DECOMPILATION-LOG.md`
   carries a `D-NNNN` entry before any native analysis; findings are DECOMPILE
   class. (ADR-0001)
4. **The transport clamps every channel.** `crates/hal/src/device.rs`
   `write_motor_offsets` clamps to ±`MAX_MOTOR_OFFSET` (300) on every packet.
   (ADR-0002)
5. **SDK ceilings derive from the DAC bound and cannot be raised by callers.**
   `MAX_FORCE_N` 2.0, `MAX_DAC_OFFSET` = transport clamp, `MAX_SLEW_N_S` 100,
   `COMMAND_TIMEOUT` 25 ms; `ForceLimits::set_limits` refuses anything higher
   and refuses changes while armed. (ADR-0002)
6. **Defaults are the proven envelope.** `ForceLimits::default()` is 0.5 N and
   96 counts, the envelope armed live in P-0030; `CONTINUOUS_FORCE_N` 1.0 caps
   content without a fixed dwell. (ADR-0002)
7. **Stale demand is zero.** A request older than `COMMAND_TIMEOUT`, or a servo
   gap longer than it, outputs zero; zero is never slewed. (ADR-0002)
8. **Output is armed explicitly and never rearmed automatically.** The zero-only
   handshake precedes any nonzero packet; every sampling error ends the session;
   a failed shutdown outranks any other error and requires reconnection. (ADR-0002)
9. **Cartesian demand requires registration.** `Device::request(Demand::Force)`
   is refused until `register_reset` has recorded the reset pose on this
   connection; a reconnect clears registration. (ADR-0004)
10. **Verification unlocks are per session and per connection.** The Studio's
    `Session` ladder gates each step on passes recorded on this connection;
    saved evidence never arms output; nothing above 1.0 N runs outside a
    fixed-dwell step of at most 10 s. (ADR-0003)
11. **Zero and disarm precede persistence.** Every attempt writes its evidence
    after output has been zeroed and the guard disarmed. (ADR-0003)
12. **Commands are not measurements.** Any value labelled force in code, logs or
    UI is a model command; delivered force exists only in OWNER or OBSERVE rows
    from an attended session. (ADR-0003)
13. **CI never opens hardware.** `scripts/quality-gate.sh` and
    `.github/workflows/ci.yml` touch no device node. (ADR-0005)
14. **Public APIs are documented.** Every crate enables `missing_docs`; strict
    rustdoc is in the gate. (ADR-0005)
15. **Posture.** Private lab evaluation, no economic use; any intent to
    publish, distribute or demonstrate externally re-opens ADR-0001's legal
    review first. (ADR-0006)

## 5. Calibration and verification workflow (ADR-0003, ADR-0004)

Connect input-only → actuator liveness (raw channel square wave, ≤ 96 counts) →
register at the reset pose (near arm horizontal and square to the base front, far
arm plumb) → repeatability, gravity-drop sign and ruler checks → sign pulses →
static ladder 0.1 / 0.25 / 0.5 N → effects at 0.5 N → 1.0 N rung → lift test →
2.0 N rung, wall ramp → thermal hold. Acceptance rules, interlocks, evidence
schema and the owner runbook are normative in `docs/spec/14-force-calibration.md`.

## 6. Quality gates (ADR-0005)

`scripts/quality-gate.sh`: ADR ledger check, `cargo fmt --check`, Clippy with
warnings as errors on all targets, `cargo test --workspace`, strict rustdoc,
viewer `npm test` and format check, audio tool tests, vendor-artefact scan. CI
runs the same script. Graphics smoke modes are local and optional.

## 7. Change process

1. Copy `docs/adr/TEMPLATE.md` to the next free number; fill the three-axis
   status honestly; cite the invariant you change or add.
2. Update the affected section of this document, bump `version`, and re-record
   `verified_commit` in the same change.
3. Regenerate the ledger index (`node scripts/adr-index-gen.cjs docs/adr`).
4. Run `scripts/quality-gate.sh`.
5. A physical claim needs a provenance row from an attended session before an
   ADR may record it as verified.
