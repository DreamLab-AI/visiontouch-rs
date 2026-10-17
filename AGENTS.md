# visiontouch-rs: repository rules for coding agents

Tool-neutral rules for this repository. Estate-wide behaviour and container facts
come from the tiers above this file; do not repeat them here.

## What this is

A lab evaluation, formally part of the VisionFlow research estate:
a clean-room Rust host stack for a lawfully owned 1990s SensAble PHANTOM 1.0A
haptic arm on an EPP parallel port, one of very few surviving units. It is a
proof of principle with no economic use, built as a study for the lab's
VisionFlow installation. The outputs are an academic paper and, later, a
pose-and-button input for grabbing nodes in the VisionClaw graph (ADR-0006). No
software release, sale or service is planned. That posture is load-bearing for
the legal position (`docs/legal/POSITION.md`); any change to it is an ADR and a
legal re-review.

## Governing documents

| Need | Read |
|---|---|
| Invariants and how to amend them | `docs/BASELINE-visiontouch-rs.md` |
| Decisions ledger | `docs/adr/README.md` (generated; edit `PREAMBLE.md`, not the table) |
| Protocol facts | `docs/spec/*.md`, each fact with a `P-NNNN` row in `docs/clean-room/PROVENANCE.md` |
| Clean-room rules | `docs/clean-room/POLICY.md`, `DECOMPILATION-LOG.md` |
| Force calibration and verification protocol | `docs/spec/14-force-calibration.md` |
| Historical device characterisation | `docs/research/historical-calibration-practice.md` |

## Non-negotiables

1. **Spec before code, provenance before spec.** A protocol behaviour is written in
   `docs/spec/` in our own words with a provenance row before the Rust that uses it.
2. **No vendor artefacts in the tree.** No binaries, firmware, headers, manuals,
   screenshots, decompiler output, vendor strings or identifiers. `vendor-original/`
   is ignored and lives only on the owner's machine.
3. **Decompilation is logged or it did not happen.** A `D-NNNN` entry naming the
   artefact hash and the single interoperability question precedes any native
   analysis; its findings are DECOMPILE class and never reach the paper.
4. **Commands are not measurements.** A value labelled force, torque or newtons in
   this code is a model command. Delivered force is established only by an attended
   session with a gauge or mass and an OWNER/OBSERVE provenance row.
5. **The envelope only tightens from the application side.** `ForceLimits` lowers
   SDK ceilings; it cannot raise them. Raising an SDK hardware ceiling is an ADR
   citing PUBLIC characterisation (P-0053 to P-0055) and a model-independent DAC
   bound. Raising an application default or `CONTINUOUS_FORCE_N` needs OWNER or
   OBSERVE delivered-force evidence (ADR-0002, ADR-0003).
6. **CI never opens hardware.** `scripts/quality-gate.sh` is the gate; nothing in it
   touches `/dev/parport0`. Graphics smoke modes are optional and local.
7. **Public Rust APIs are documented.** Every library crate (`hal`, `device`,
   `effects`) enables `missing_docs`; apps and tools are covered by strict rustdoc
   only, which is part of the gate.
8. **Hand-rolled cryptography is forbidden** (none is needed here); the port
   recorder's preload shim is the only C/assembly and stays test-covered.

## Working in this repository

- Run `scripts/quality-gate.sh` before every commit. `SKIP_VIEWER=1` skips the
  Node viewer when `npm` is unavailable.
- Physical tests happen on the owner's host, not in the agent container. Prepare
  the protocol, the app and the evidence template; the owner runs it and reports
  back through the repository (`docs/evidence/`, provenance rows).
- Layout: `crates/hal` (ppdev software-EPP transport), `crates/device` (SDK:
  kinematics, guard, lifecycle), `crates/effects` (hardware-free force models), `apps/calibration` (Calibration
  Studio), `apps/mass-lab` (physics sandbox), `apps/workspace-viewer` (Three.js
  reconstruction), `tools/` (Rust probe, recorder and readout crates plus the audio analysis tool), `scripts/` (launchers, port-access helper, quality gate, ADR index), `archive/` (gitignored local holding area).
- Diagrams for this repository live in the estate canon at
  `VisionFlow/docs/diagrams/visiontouch-rs/` (area `VT`, VT-01 to VT-08), not here.
- UK English, no em-dashes in prose, no trademarks in crate or binary names.
