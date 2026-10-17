# Calibration and demo acceptance status

Audit of source revision `63b7ccc`, after the owner powered the unit off.
The overall goal is **not complete**: offline software checks cannot establish
physical force response or release on this aged unit. No hardware was opened
for this audit. This document distinguishes evidence from remaining acceptance
work; it does not authorize a new powered experiment.

## Requirements and evidence

| Requirement | Current evidence | Acceptance still needed |
| --- | --- | --- |
| Guided reference and editable working bounds | `apps/calibration/src/main.rs`, `model.rs`, `worker.rs`; mapping and stale-reference rejection tests | Attended capture, measured axis/sign comparison and usability with the actual pen |
| Curved workspace instead of an assumed reachable box | `sweep.rs`; exact extrema independent of 2 mm display thinning; bounded point cloud; `crates/workspace` continuous segment-support tests | Fresh scan with coverage of the intended working region; observations alone do not certify clearance |
| Slow zero approach against resistance | `model::Move`; target-lead, progress/stall, reverse-motion and settle checks; live out-and-back `teaching.rs` | Observed travel toward zero and bounded resistance on the actual arm; existing owner report of ineffective block resistance remains unresolved |
| Hold/release and interruption behaviour | Worker interlocks, fresh hold edges, SDK expiry, command-limit trip and failed-shutdown classification tests | Physical release observations with matching attempt logs; software cannot establish a hardware watchdog or shutdown during a kernel stall |
| Cube, water sphere, deforming/popping balloon, three-body springs and central gravity | `crates/effects`; direction, damping, pop/reset and integration tests; native seven-scene smoke test | Perceptual and physical response in each scene, particularly opposing object/pen motion |
| Additional canonical scenes | Plane, viscosity and detents in effects crate and native scene renderer | Directional response and stationary-force observations on the unit |
| Baseline evidence and approximately bottom-centre desk rest | `docs/evidence/input-only-sweep-20261016`; 31,271 input-only samples, raw/continuous six-channel data and owner pose annotation | Fresh live reference after reconnection; no exact mechanical origin may be inferred from the old rest pose |
| Three.js reconstruction from owner images and recorded inputs | `apps/workspace-viewer`; supplied photo archive, replay, observed-cell surface, optional convex bound, link edits and GLB export | Independent arm measurements and wrist-channel assignment; current cosmetic dimensions are inferred |
| Overload archaeology, including Windows parallel driver and old demos | `docs/research/overload-and-calibration.md`; P-0036/P-0041 and private D-0001/D-0002 evidence | Exact unit/revision compatibility and physical trip conditions; the complete historical Greatest Hits pack remains unrecovered, so Dice and newer examples are partial coverage |
| Aged-unit derating, sound and temperature observations | SDK `UNIT_DERATING`; per-attempt manual observations and WAV references; offline audio analysis tool | Owner recordings and surface readings correlated to attempts; no validated acoustic labour detector or winding-temperature estimate exists |
| Inline documentation for reusable Rust crates | HAL, device, effects and workspace public APIs documented; `missing_docs` warning enabled; strict rustdoc and doctests | Keep documentation consistent with any changes made after physical diagnosis |
| Commit/push and offline use | `main` contains calibration, viewer and evidence; app README launch commands and handoff | No deployment or hardware connection is needed for offline preview |

## Verification at this audit

- `cargo test --workspace`: 60 unit tests and three documentation tests passed.
- Viewer `npm test`: five tests passed; `npm run format:check` passed.
- `python3 -m unittest discover -s tools/audio-analysis -p 'test_*.py'`: six passed.
- Prior continuation at this source revision: strict workspace rustdoc and
  Clippy all targets passed. Native demo and sweep graphics checks passed in
  preceding implementation turns; these verify rendering, not physical output.
- Existing viewer browser checks exercise rendering, import, replay and GLB
  export. They are not a live hardware test and were not rerun in this audit.

These checks support the numerical, persistence and software lifecycle claims
above. They cannot establish end-to-end physical acceptance.

## Next attended session

Begin with the app's input-only connection and a fresh bottom reference. Compare
small hand-guided movements along each axis with the displayed model position;
record any sign, scale or linkage disagreement before attempting motor output.
Scan the comfortable working region, choose an interior zero and export evidence.
Teach a short straight approach and return with motors off.

Only then assess a short held pilot within the existing 0.1 model N sweep ceiling.
Record whether motion is toward zero, whether resistance is perceptible, and
whether releasing hold stops resistance. Preserve the CSV, metadata and operator
observation even when the attempt fails. A poor response calls for diagnosis of
mapping, command offsets and actuator response; it is not a reason to raise caps.

After basic response and release are understood, assess the canonical scenes and
controlled resistance/collision cases. Distinguish a software command-limit trip
from a transport failure and from the owner's physical sensation of release.
Log motor-surface temperatures and audio with target, time and recording details.
Neither sound nor a surface-temperature reading alone establishes safe torque or
winding temperature. Final unit derating and overload conclusions require those
physical observations; no historical manufacturer peak rating substitutes for them.

The unit is currently off. The next acceptance action depends on the owner being
present and explicitly resuming the physical session. Do not repeatedly rerun
unchanged simulations as a substitute for that evidence.
