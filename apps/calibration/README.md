# Calibration Studio

A native Rust workbench (macroquad and egui) that guides an attended session on the
owner's haptic arm and records evidence for it. It sits on `visiontouch-device` and
`visiontouch-effects`. A servo worker owns device I/O and motor requests; the interface
sends intent and draws snapshots.

Every "newton" it shows is a model command. Delivered force exists only in the
operator readings (gauge, scale, mass) that the studio records beside each attempt.

```sh
scripts/run-calibration --demo          # mouse-driven simulated plant, no hardware
scripts/run-calibration --smoke-test    # headless end-to-end run against the simulator
scripts/run-calibration --no-device     # start offline, connect from the interface
scripts/run-calibration                 # connect with motor output OFF
scripts/run-calibration --help
```

`--demo`, `--smoke-test` and `--help` never open hardware and do not invoke the
port-access helper. Connecting never enables motor output.

## Session unlock ladder

Nothing persists across connections: a reconnect forgets every verdict. A refused
Start names the missing prerequisite. Rung verdicts, with `m` measured and `c`
commanded: **Over** if `m > 1.2c + 0.05 N` or the arm oscillates (blocks the next rung
on that axis), **Pass** if `m >= 0.8c - 0.05 N`, otherwise **Under** (recorded with
its slope, not blocking). A rung is cleared on an axis when one sign has a verdict and
none is Over.

| Unlocks | Requires | Limits |
|---|---|---|
| Backdrive, channel liveness | connected | channels at most 96 counts |
| Sign pulses | liveness on all 3 channels (felt, heard or lock-in), registration, return to reset within 2 mm, gravity drop toward -Y, ruler sign and off-axis on X, Y, Z | 0.5 N, 96 counts |
| 0.1 to 0.5 N rungs, effects at 0.5 N, lift to 0.5 N | sign pulses on X, Y, Z | 0.5 N, 96 counts |
| 1.0 N rung, lift to 1.0 N | 0.5 N rung cleared on X, Y, Z | 1.0 N, 300 counts |
| 2.0 N rung, lift to 2.0 N | 1.0 N rung cleared on X, Y, Z | 2.0 N, 300 counts |
| Effects to 1.0 N, wall ramp, thermal | 1.0 N rung cleared on X, Y, Z | 1.0 N, 300 counts |

The studio is the only consumer that builds limits above the SDK defaults, only inside
fixed-dwell steps, and sets limits explicitly (disarm, set, arm) for every attempt.

## Interlocks (per step class)

Hold key (all but thermal, which needs a Confirm every 30 s with a 10 s grace), interface
heartbeat under 250 ms, sample interval under 25 ms, pen button, filtered speed
(50 mm/s, or 400 mm/s for effects and wall), excursion from the start pose (3 mm ladder
and thermal, 15 mm liveness and sign, 25 mm lift, 150 mm effects and wall), time bound
(4, 1, 4, 60, 60, 660, 180 s for liveness, sign, ladder, lift, effects, thermal, wall),
more than 10 s above 1.0 N, lift-off above 2 mm, and a wall oscillation detector
(5 to 60 Hz band-passed Y velocity, RMS above 15 mm/s over 0.5 s). Any stop zeroes the
demand on the same tick, output is disarmed, and only then is evidence written.

## Evidence

`captures/calibration/<utc>-<session_id>/` holds `session.json` and, per attempt,
`<id>-<step>.csv` plus `<id>-<step>.json` (schema 2) with the SHA-256 of the CSV, the
git commit read at run time, limits, registration, stop reason, telemetry summary,
automatic measurements and the operator entries. Entries (gauge N, scale g,
temperature C, ruler mm, felt and heard) are keyed to the attempt id and accepted only
while output is off. Commit a session directory to `docs/evidence/` after the owner
session. The coverage cloud exports as a plain viewer schema 1 capture.

## Modules

`main` CLI and loop, `ui` panels, `scene` 3D view, `worker` engine and thread,
`controller` interlocks, `protocol` steps and unlock ladder, `registration` stillness
and checks, `evidence` writers, `arm` skeleton geometry (port of `armPose`),
`coverage` display cloud, `sim` simulated plant, `smoke` headless end-to-end run.
Only `worker` touches `visiontouch_device::Device`; the rest is hardware-free and tested.

```sh
cargo test -p visiontouch-studio
cargo clippy -p visiontouch-studio --all-targets -- -D warnings
```
