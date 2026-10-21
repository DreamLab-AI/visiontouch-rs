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

The saved reset reference is a white wire box in the 3D view, with a line to the
live pen and a model distance readout in the registration and Live panels. The
box is a visibility aid, not a tolerance region. Reset is the specified physical
arm pose, not the centre of the working volume. For a return check, reproduce
that physical pose independently of the display; steering to the marker does
not establish physical repeatability. Reconnecting requires registration again.

## Session unlock ladder

Nothing persists across connections: a reconnect forgets every verdict. A refused
Start names the missing prerequisite. Rung verdicts, with `m` measured and `c`
commanded: **Over** if `m > 1.2c + 0.05 N` or the arm oscillates (locks every force
step for the rest of the connection; re-registration does not clear it, only a
reconnect does), **Pass** if `m >= 0.8c - 0.05 N`, **None** if the band is missed and
a completed dwell read under 0.05 N (nothing delivered: it neither clears the rung nor
locks), otherwise **Under** (recorded with its slope, clears the rung, not blocking).
A rung is cleared on an axis when one sign is Pass or Under and none is Over.

| Unlocks | Requires | Limits |
|---|---|---|
| Backdrive, channel liveness | connected | channels at most 96 counts |
| Sign pulses | liveness on all 3 channels (felt, heard or lock-in), registration, return to reset within 2 mm, gravity drop toward -Y, ruler sign and off-axis on X, Y, Z | 0.5 N, 96 counts |
| 0.1 to 0.5 N rungs, effects at 0.5 N, lift to 0.5 N | sign pulses on X, Y, Z | 0.5 N, 96 counts |
| 1.0 N rung, lift to 1.0 N | 0.5 N rung cleared on X, Y, Z | 1.0 N, 300 counts |
| 2.0 N rung, lift to 1.5 N | 1.0 N rung cleared on X, Y, Z | 2.0 N, 300 counts |
| Effects to 1.0 N, wall ramp, thermal | 1.0 N rung cleared on X, Y, Z | 1.0 N, 300 counts |

The studio is the only consumer that builds limits above the SDK defaults, only inside
fixed-dwell steps, and sets limits explicitly (disarm, set, arm) for every attempt.

## Interlocks (per step class)

Hold key (all but thermal, which needs a Confirm every 30 s with a 10 s grace), interface
heartbeat under 250 ms, sample interval under 25 ms, pen button, filtered speed
(50 mm/s, or 400 mm/s for effects and wall), excursion from the start pose (3 mm ladder
and thermal, 15 mm liveness and sign, 25 mm lift, 150 mm effects and wall), time bound
(4, 1.5, 4, 60, 180, 660, 60 s for liveness, sign, ladder, lift, effects, thermal, wall),
more than 10 s above 1.0 N, lift-off above 2 mm, and a wall oscillation detector
(5 to 60 Hz band-passed Y velocity, RMS above 15 mm/s over 0.5 s). Any stop zeroes the
demand on the same tick, output is disarmed, and only then is evidence written.

## Evidence

`captures/calibration/<utc>-<session_id>/` holds `session.json` and, per attempt,
`<id>-<step>.csv` plus `<id>-<step>.json` (schema 2) with the SHA-256 of the CSV, the
git commit read at run time, limits, registration, stop reason, telemetry summary,
automatic measurements and the operator entries. Entries (gauge N, scale g,
temperature C, ruler mm, felt and heard) are keyed to the attempt id and accepted only
while output is off (the engine refuses them while an attempt runs; the one exception
is the casing temperature in C during a thermal hold). `session.json` also records the
lift results (F_0, F_m per mass, gain) and `k_marginal_n_mm`. Commit a session directory to `docs/evidence/` after the owner
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

## Guided setup

The default view is a six-stage guide: observe input, save the illustrated reset
pose, check it without moving, check a physical return, record one motor response
at a time, then hand over to the measurement bench. The view leaves most of the
window for the arm and keeps output state and Stop visible. No force gauge is
required for onboarding; measured force remains a later bench task.

Only worker-acknowledged captures advance the reference checks. Changing the
reference resets guide progress. A stopped motor check has its own explanation
and observations attach to that attempt automatically; a stop does not diagnose
a failed motor. The guide offers a retry only for a released hold, not repeated
speed or travel stops. Switching views cancels any prepared test. The guide uses
the existing motor plans and interlocks and does not establish delivered force.

## Local control API

Every graphical Studio instance starts a user-private Unix socket at
`$XDG_RUNTIME_DIR/visiontouch-studio/<pid>.sock` (fallback:
`/tmp/visiontouch-studio-<uid>/<pid>.sock`). The directory is 0700, the socket 0600.
There is no network listener. The PID and simulated/real mode are returned by
status; select a socket explicitly if more than one instance is running.

```sh
scripts/calibration-control --list
scripts/calibration-control status
scripts/calibration-control '{"action":"view","bench":false}'
scripts/calibration-control '{"action":"guide_next"}'
scripts/calibration-control '{"action":"register_reference"}'
scripts/calibration-control '{"action":"capture","mark":"return"}'
scripts/calibration-control '{"action":"stop"}'
```

Send one JSON object followed by a newline per connection. A successful reply
contains `ok`, `disposition` and `state`. `dispatched_to_worker_check_status` means
intent was sent, not that the worker completed it. Poll status and inspect
`status`, `checks`, `registered`, `last_result` and `evidence_dir` for the result.
`snapshot_age_ms` describes the cached worker snapshot; it is null before the
first snapshot. A request expires before application if the UI cannot process it
within one second. If a client times out, inspect status before retrying a mutation.

| Action | Additional fields |
|---|---|
| `status`, `stop`, `connect`, `disconnect`, `guide_next` | none |
| `view` | `bench`: boolean; cancels any prepared test |
| `register_reference` | `replace`: defaults false; explicitly true to replace an existing reference |
| `capture` | `mark`: `return`, `ruler_start`, `ruler_end`, `effect_centre` |
| `arm_gravity` | none; input-only check |
| `submit_ruler` | `axis`: `x`/`y`/`z`, `measured_mm`: signed number |
| `observe` | `attempt_id`, `felt` and `heard`: boolean or null, optional `note` |
| `readings` | `attempt_id`, `readings`: array of `{label,value,unit}`; units `N`, `G`, `C`, `Mm` |
| `coverage` | `enabled`: boolean |
| `export_coverage`, `start_backdrive` | none; output must be idle |
| `prepare` | `test`: object from the table below; needs a fresh local hold |

| Test kind | Additional fields |
|---|---|
| `liveness` | `channel`: 0, 1 or 2 |
| `sign_pulse` | `axis`: `x`/`y`/`z` |
| `ladder` | `axis`, `sign`: `pos`/`neg`, `rung_n`: 0.1, 0.25, 0.5, 1.0 or 2.0 |
| `lift` | `mass_g`: null for bare, or positive up to 100 |
| `effect` | `name`: an effect's displayed name |
| `wall_ramp` | none |

Example preparation (does not arm output):

```sh
scripts/calibration-control '{"action":"prepare","test":{"kind":"liveness","channel":0}}'
```

There is no API hold, raw demand, limit override, direct verdict, thermal Start or
thermal Confirm. Power still requires a fresh local Space/button hold. Navigation
and Stop cancel preparation. Worker checks, force limits and evidence rules are
unchanged. Same-user processes are trusted like the local interface. Headless
smoke tests do not start a server.

## Short camera sequences

```sh
scripts/capture-camera --seconds 3 --fps 2
scripts/capture-camera --device /dev/video0 --size 1920x1080 --seconds 6 --fps 1
```

These commands capture stills only while invoked, with a maximum ten-second
recording after a default ten-second warm-up (`--warmup 0` to skip it). Use
`--format yuyv422 --size 640x480` if the adapter delivers corrupt MJPEG. Decoder
logs, frames and a hash manifest stay in the ignored
`captures/calibration/camera/` directory. They are not synchronised to servo
samples. A successfully extracted frame can still be the capture adapter's
"No Video" screen; inspect the image before using it as evidence of arm motion.
