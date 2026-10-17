# Force calibration and verification

Status: specification, not yet run on the unit. Design rows: P-0061 to P-0064.
Decisions: ADR-0002 (envelope), ADR-0003 (staged ladder), ADR-0004 (registration).
Related: [12-force-feedback.md](12-force-feedback.md), [13-calibration.md](13-calibration.md).

## 1. Purpose

No force delivered by this unit has ever been observed. Every nonzero command sent so
far was a model command (P-0030, P-0033), and the 2026-10-16 envelope held commanded
force below published friction (P-0060). This specification defines the attended
procedure that establishes, in order: that the actuators respond at all, that the
model frame is registered to the arm, that command signs match motion, and how much
force the unit delivers per commanded newton.

Rules that apply to every step:

1. A value labelled force, torque or newtons in software or logs is a model command.
   Delivered force exists only as an operator gauge or mass reading and is promoted
   only through an OWNER or OBSERVE provenance row (AGENTS.md rule 4).
2. Each higher force or content class unlocks only after the lower rung cleared on the
   same connection (section 7, P-0063). No unlock survives a reconnect.
3. No step opens hardware from CI. The procedure runs on the owner's host (AGENTS.md
   rule 6); the agent prepares the protocol and evidence template.
4. A protocol outcome never raises a code ceiling by itself. Raising an application
   default or `CONTINUOUS_FORCE_N` needs a committed OWNER or OBSERVE record and an ADR.

## 2. Prerequisites

| Prerequisite | Requirement | Basis |
|---|---|---|
| Connection | Native session established with output disabled; all six counters change when the arm is moved | P-0023, P-0037 |
| Servo rate | Median sample interval near 2.4 ms (about 420 Hz); no interval over 25 ms. Stop below 300 Hz | P-0037 |
| Arm support | Arm supported at power-on; nothing else in the workspace; owner within reach of the amplifier power switch | P-0055 |
| Backdrive baseline | Motors off, pen pulled slowly with a gauge along X, Y and Z near mid pose. Expected order 0.04 N. Stop above 0.2 N and inspect cables | P-0053 |
| Software state | Quality gate passed; limits at `ForceLimits::default()` until a rung unlocks a higher set | ADR-0002 |

The backdrive baseline is an operator reading. It is not compared with the published
0.04 N as an acceptance test (the published figure is for a different model, P-0053);
it is a screen for binding cables and a reference for the lift test (section 6.5).

## 3. SDK envelope

All numbers are code constants in `crates/device/src/lib.rs` and
`crates/hal/src/device.rs`. They bound commands. They are not measured forces.

| Quantity | Value | Meaning | Basis |
|---|---|---|---|
| `MAX_FORCE_N` | 2.0 N | Model-force ceiling; top of the test ladder | P-0061, P-0053 |
| `DEFAULT_FORCE_N` | 0.5 N | Default for every consumer; the envelope armed live in P-0030 | P-0030, P-0061 |
| `CONTINUOUS_FORCE_N` | 1.0 N | Cap for content without a fixed dwell (effects, demos, Mass Lab); below the 1.4 N continuous rating | P-0053, P-0061 |
| `MAX_DAC_OFFSET` | 300 counts | Equals the HAL transport clamp `MAX_MOTOR_OFFSET`; 14.7 percent of the 2047 half-span | P-0061 |
| `DEFAULT_DAC_OFFSET` | 96 counts | Default per-channel limit; the P-0030 envelope | P-0030, P-0061 |
| `MAX_CHANNEL_TEST_OFFSET` | 96 counts | Raw channel demand never leaves the P-0030 envelope | P-0061 |
| `MAX_SLEW_N_S` | 100 N/s | 0.24 N per 2.4 ms tick; 0 to 2 N in 9 ticks. Stiffest content (1.0 N/mm) at a brisk 0.1 m/s must not bind | P-0037, P-0061 |
| `COMMAND_TIMEOUT` | 25 ms | A request older than this yields zero; about ten ticks | P-0037, P-0061 |
| `UNIT_DERATING` | 1.0 | Unchanged; a measured gauge slope may lower it by ADR | ADR-0002 |

Derivation of the DAC ceiling (P-0061). The Jacobian columns for channels 1 and 2 have
equal norm L at every pose, so those channels carry at most 960 x L = 134 counts per
model newton anywhere in the workspace (gains from P-0028). A 2 N command therefore
needs 268 counts; adding 10 percent margin and rounding gives 300. Channel 0 carries
825 x r counts per newton with r = L(cos q1 + sin q2), which equals L at the reset
pose and grows away from it; where channel 0 would exceed the limit first, uniform
scaling applies (below).

The physical reading of 300 counts depends on an unmeasured gain. Under the repository
gains it is about 2.24 N on channels 1 and 2. Under a datasheet-linear reading
(2047 counts equal 8.5 N peak) it is about 1.25 N. Both are at or below the 2 N test
ceiling and at most 26 percent of the published peak (P-0053). The DAC limit, not the
model, is the only model-independent bound until a gauge measures the slope.

Behavioural rules:

- **Uniform DAC scaling.** When the largest channel demand exceeds the DAC limit the
  whole force vector is scaled by one factor so that channel fits. Per-channel
  clamping rotates the force and leaks tangential force into walls. The HAL clamp
  remains as an independent backstop.
- **No release trip.** The 2026-10-16 sustained-command-limit release is removed
  (P-0060). Demand above a cap is clamped and reported in telemetry, never latched.
- **Zero is never slewed.** A zero or invalid demand, a release, an expired request,
  or an interlock stop returns the output to zero on the same tick.
- **Tighten only.** `ForceLimits` can lower ceilings, never raise them, and cannot be
  changed while armed.
- **Time bound.** Within a step, cumulative time with applied force above
  `CONTINUOUS_FORCE_N` is at most 10 s, after which the attempt stops (P-0055 shows a
  2 N hold heats the casing by 34.5 C with the stock amplifier).

## 4. Registration

Registration binds the model frame to the arm. It is owned by the connection, so a
reconnect clears it by construction; it is recorded for all six counters and applied to
the three position channels. Cartesian force demand is refused while unregistered. The
raw channel test is not (section 6.2), because it needs no Jacobian.

**Reset pose.** The reset pose is the all-links-orthogonal pose: near arm horizontal and
square to the base front, far arm hanging plumb, the four linkage joints forming a
rectangle (P-0049, P-0050, P-0051). The repository model places counts zero at this
pose with the far arm plumb (P-0050). The owner places the arm there with a set square
and holds still. Registering at the reset pose rather than a bottom rest is design
choice P-0062 (ADR-0004).

**Capture.** Registration is accepted only when the reference-model position varies by
within a 0.5 mm extent (bounding-box diagonal) over 250 ms. The captured continuous counters become the
origin; all later positions are registered counts.

**Why checks are needed.** A hand-set reset plus nominal kinematics gives 10 to 60 mm of
tip error in published work (P-0057). The earlier "drawn arm matches after zeroing"
check is true by construction and is not a test. The discriminating checks are:

| Check | Procedure | Acceptance | Failure handling |
|---|---|---|---|
| Stillness | Counters quiet at capture | 250 ms, within a 0.5 mm extent (bounding-box diagonal) | Capture refused |
| Repeatability | Move away, return to reset, compare | Within 2 mm | Redo registration |
| Gravity-drop sign | Arm the test in the Studio, then release the pen from mid height onto the palm about 20 mm below | Model position moves in -Y; a fail is sticky until re-registration | Stop; record an OBSERVE row; fix the model, never an ad hoc flip |
| Ruler scale | Mark start, slide 50 mm along a square on each of X, Y, Z, mark end, enter millimetres | Model/ruler ratio 0.8 to 1.25 | Recorded and warned, not a stop (the ladder measures force directly) |
| Ruler off-axis | Same move | Off-axis component at most 15 percent of travel | Stop |
| Sign agreement | Same move | Ruler direction agrees with model sign | Stop |

The operator records which physical direction is +X and +Z. A reconnect, a power cycle
or `clear_registration` discards registration and every unlock that depended on it
(P-0062).

## 5. Interlocks

Every attempt uses these guards. Zero or disarm precedes every file write.

| Guard | Rule |
|---|---|
| Hold | Key held for the whole attempt, except thermal (section 6.7) which uses a Confirm every 30 s with a 10 s grace |
| UI heartbeat | Pulse age at most 250 ms |
| Sample interval | At most 25 ms, else stop |
| Pen button | Press stops the attempt |
| Request expiry | 25 ms, zero on expiry |
| Speed | At most 50 mm/s for liveness, sign, ladder, lift and thermal; at most 400 mm/s for effects and wall |
| Excursion from start | 3 mm (ladder, thermal); 15 mm (liveness, sign); 25 mm (lift); 150 mm (effects, wall) |
| Time bound | Per class in `controller.rs`: ladder 4 s, lift and wall 60 s, effects 180 s, thermal 660 s; liveness and sign are bounded to their plan length plus margin |
| Continuous cap | Cumulative time above 1.0 N at most 10 s |

## 6. Verification steps

Steps run in this order. Each lists plan, interlocks, acceptance and operator entries.
Auto-scoring uses only encoder data. Operator entries are keyed to `attempt_id` and
made after release with output off; gauge readings cannot be typed while a key is held.

### 6.1 Backdrive baseline

Plan: motors off. Interlocks: none beyond the connection. Operator entries: gauge reading
on X, Y and Z. Acceptance: below 0.2 N on each axis. Outcome: reference only; it informs
the lift test and does not unlock anything.

### 6.2 Actuator liveness

Plan: for each channel in turn, a 3 Hz square wave of plus and minus 80 counts for 2 s
with the pen held lightly near the reset pose, phone recording 10 cm from the drive
housing. Limits: raw channel demand, at most 96 counts (P-0030). It needs no
registration and no Jacobian, and it is the go/no-go for the session because amplifier
enable is unproven (force-audit F6, P-0060). Interlocks: section 5 with 15 mm excursion
and a 4 s bound per channel (the 2 s plan plus margin).

Auto-scoring: each channel is cross-correlated against all three encoders, because the
channel-to-encoder map from P-0013 is unproven; only the best encoder is recorded
(`auto.lockin_encoder`). The noise estimate comes from the off-frequency bins of the
driven record itself. A channel passes if that encoder's lock-in amplitude at 3 Hz
exceeds five times the noise. Operator entries: felt and heard per channel; they count
only on a completed drive, never on an aborted one. Acceptance: a channel passes on
felt, heard or lock-in. Failure handling: if no channel passes, end the session; the next session
captures input-only packets with feedback off and on and compares bytes 12, 13 and 15
(F6.1), then runs the vendor black-box live check (F6.3). If one or two channels fail,
stop and report which.

### 6.3 Registration checks

As section 4. Unlocks sign pulses only when stillness, repeatability, gravity-drop sign
and ruler sign pass in this session.

### 6.4 Sign pulses

Plan per axis: +0.25 N for 300 ms, 200 ms of zero, -0.25 N for 300 ms, pen floated on a
fingertip. Limits: force 0.5 N, DAC 96 counts, slew 100 N/s. Auto-scoring: the encoder
displacement difference between the two pulses must have the commanded sign. Acceptance:
agreement on X, Y and Z. Failure handling: stop on any disagreement, report, and do not
continue to the force rungs. A sign error is an OBSERVE row and a model fix.

### 6.5 Force ladder

Plan per (axis, sign, rung): 0.5 s ramp, 2 s dwell, 0.5 s ramp, 1 s zero. Rungs are 0.1,
0.25, 0.5, 1.0 and 2.0 N. One held attempt per (axis, sign, rung). The operator reads the
steady gauge value during the dwell and enters it after release.

| Setup | Axis | Notes |
|---|---|---|
| Pen pressing down on a kitchen scale, tared at zero command | -Y | Arm weight biases +Y, so only -Y is read directly |
| Gauge against a rigid post | X and Z | One sign minimum, both if time allows |

Verdict, with m the measured value and c the commanded value (P-0063):

| Verdict | Rule | Effect |
|---|---|---|
| Over | m > 1.2 c + 0.05 N, or oscillation | Locks every force step for the rest of the connection; the Studio refuses each Start naming the rung and axis |
| Aborted | Attempt stopped before the plan completed (the whole 4 s), for any reason | Recorded; no verdict; no unlock |
| Pass | m at least 0.8 c - 0.05 N | Counts towards the unlock |
| Under | below the pass band, with m at least 0.05 N | Recorded with the slope; clears the rung and does not block, because the DAC bound delivers less rather than more |
| None | below the pass band and m below 0.05 N absolute, on a completed dwell | Recorded as "nothing delivered"; does not clear the rung and does not lock |

Under is not a failure of safety: a slope near 0.55 is plausible given the factor-of-two
gain uncertainty. The recorded slope is the real counts-per-newton constant and is the
principal OWNER result. A single-point match is inconclusive; the slope across rungs is
the evidence. This differs from the historical acceptance of 0.8 to 1.2 slope because
that band assumes calibrated gains; here the slope is the unknown.

### 6.6 Differential lift test

Arm weight confounds a direct hold, and link masses are NOT FOUND for the 1.0, so no
gravity model exists. Plan: with the pen resting on a book stack at reset height and
hands off, ramp +Y from 0 at 0.05 N/s up to 1.0 N and at 0.1 N/s above it, up to the
unlocked cap (1.5 N at most, so the 10 s budget above 1.0 N is never reached), and detect lift-off
automatically (rise above 2 mm). Repeat with a 50 g mass on a light hook, then 100 g.
With F_0 the lift-off force bare and F_m with mass m:

    g = m g / (F_m - F_0)

The ratio cancels friction to first order and needs no gauge. Units matter when
comparing: the ladder slope is s = m / c (delivered per commanded), and F_m - F_0 is
commanded while m g is delivered, so the lift gain g is also delivered per commanded.
Expect g close to s. (Its reciprocal, 1 / g, is commanded per delivered newton.) F_0 is recorded as the
first gravity data point. Stop if the arm walks sideways more than 25 mm or rises
quickly. Operator entries: the masses. Acceptance: reported, not gated. The gain g is
compared with the ladder slope s for consistency; they should agree to within the
scatter of the readings.

### 6.7 Thermal hold

A 600 s hold cannot sit inside the held-key interlock, so thermal is its own attempt
class. Plan: pen braced down on the scale, -Y 1.0 N for 600 s with a Confirm every 30 s
and a 10 s grace, 3 mm motion window, 660 s bound. The operator enters a casing
temperature each minute at the same spot. Stop on a rise above 10 C over the first
reading, any reading above 50 C, or a missed Confirm. Rationale: 2 N for 10 min raised
the casing 34.5 C with the stock amplifier (P-0055); at half the force a quarter of the
heat is expected, so more than 10 C at 1 N is a warning. Run in a later session.

### 6.8 Virtual-wall stiffness ramp

Purpose: measure the marginal stable stiffness K_marginal so effects and demos can be
scaled to the unit.

Passivity reasoning (P-0058). A sampled wall is passive roughly while K is below 2b/T,
where b is the physical damping and T the update period. The published 2.0 N/mm was
obtained at 1 kHz (T = 1 ms, P-0054). This host servos at about 2.4 ms (P-0037), and
K scales with 1/T, so the same device would be expected near 0.83 N/mm. Encoder to DAC
latency here is about two loops (about 5 ms), which lowers the limit further. The
expected marginal stiffness is therefore 0.5 to 0.8 N/mm, well below the published
figure. The constant in the bound is not confirmed from the originals (P-0058), so the
ramp measures K rather than trusting the formula.

Plan: plane normal +Y through the start pose, one-sided, no damping, force cap 1.0 N
(`CONTINUOUS_FORCE_N`). Penetration is held at about 1 mm. At 1 mm the expected
0.5 to 0.8 N/mm stays under the cap, so oscillation is not hidden. A 5 mm penetration
would saturate a 2 N cap at 0.4 N/mm and mask the instability. Stiffness K steps from
0.1 to 1.5 N/mm in 0.1 N/mm steps of 3 s each. The display shows "cap reached" when the
force cap binds.

Stop on oscillation: the 5 to 60 Hz band-passed Y velocity RMS above 15 mm/s over 0.5 s,
computed from the interlock velocity, which is displacement over a 9.6 ms window
(four nominal ticks; one count per 2.4 ms is about 10 mm/s raw) and so does not
depend on the loop cadence (P-0065), or on operator release. The structural resonance of the family is near
125 Hz (P-0054), above the detector band by design. Operator entries: the step at which a sustained buzz began. Result: K_marginal is
the last stiffness before the stop. Unlocked only after the 1.0 N rung clears on X, Y
and Z.

## 7. Unlock ladder

Each row unlocks only on conditions met in this session and on this connection (P-0063).

| Unlocks | Requires | Limits set |
|---|---|---|
| Channel test | Connected | Channels only, at most 96 counts |
| Sign pulses | Liveness pass on all three channels; registration; gravity-drop and ruler sign | Force 0.5 N, DAC 96, slew 100 N/s |
| 0.1 to 0.5 N rungs; lift test to 0.5 N; effects at 0.5 N | Sign pulses pass on X, Y and Z | Force 0.5 N, DAC 96, slew 100 N/s |
| 1.0 N rung; lift test to 1.0 N | 0.5 N rung cleared (Pass or Under, never None) on X, Y and Z (one sign each minimum) | Force 1.0 N, DAC 300 |
| 2.0 N rung; lift test to 1.5 N | 1.0 N rung cleared on X, Y and Z | Force 2.0 N, DAC 300 |
| Effects up to 1.0 N; wall ramp; thermal | 1.0 N rung cleared on X, Y and Z | Force at most 1.0 N, DAC 300 |

Content that has no fixed dwell (effects, demos, Mass Lab) never exceeds
`CONTINUOUS_FORCE_N`. Only Calibration Studio may construct limits above the default,
and only inside fixed-dwell steps after the lower rung cleared. After a committed OWNER
verification record an ADR may raise `DEFAULT_FORCE_N` or replace the per-session
ladder with a one-rung spot check; that is outside this specification.

## 8. Effects and demo stiffness

| Rule | Value | Basis |
|---|---|---|
| Stiffness for cube, plane, balloon | `stiffness_n_m`, default 150 N/m, clamp 20 to 1500 | P-0061 |
| Other effects | Keep their constants, scaled by `stiffness_n_m` / 150 | design record section 3.3 |
| Demo stiffness after measurement | 0.7 x K_marginal (operator rule; the Studio does not apply it, its effects always run at 150 N/m) | P-0063 |
| Demo stiffness before measurement | 150 N/m | P-0061 |
| Force ceiling for effects | 0.5 N after sign pulses; 1.0 N after the 1.0 N rung; never above 1.0 N | section 7 |

Effects remain unclamped model commands; the SDK envelope clamps them. A hard-cube model
is compliant; do not claim a physically rigid wall (spec 13 effects section). The
0.7 factor, which is an operator rule and not applied by the Studio, leaves a 30 percent margin below the measured onset of oscillation, matching
the "back off 30 percent" practice cited in P-0054's source.

## 9. Evidence schema 2

One directory per session: `captures/calibration/<utc>-<session_id>/` containing
`session.json` (connect time, registration, checks, unlock history, `lift` with F_0, the
F_m per mass and the gain, and `k_marginal_n_mm`; the last two are additive fields and
the schema stays 2) and, per attempt,
`<attempt_id>-<step>.csv` and `<attempt_id>-<step>.json`. Schema 1 remains the viewer's
workspace capture format and is unchanged (spec 13).

```json
{
  "schema": 2,
  "kind": "haptics-verification-attempt",
  "session_id": "f89602ad-b4bf-4610-972d-297d1db88b02",
  "attempt_id": 16,
  "step": "ladder",
  "params": {"axis": "z", "csv_row_every": 1, "dwell_s": 2.0, "ramp_s": 0.5, "rung_n": 0.5, "sign": "pos"},
  "started_utc": "2026-10-17T12:15:07Z",
  "duration_s": 4.0008,
  "software": {"git_commit": "f9e245bc0ed9b80f22c17102590451a9e9d1165c", "sdk": {"max_force_n": 2.0, "max_dac": 300, "slew_n_s": 100.0, "timeout_ms": 25, "unit_derating": 1.0}},
  "limits": {"force_n": 0.5, "slew_n_s": 100.0, "dac_counts": 96},
  "registration": {"reset_counts": [2000, -601, 866, 0, 0, 0], "captured_utc": "2026-10-17T12:15:06Z"},
  "stop": {"reason": "completed", "detail": "phase zero at 4.00 s, position [-0.0, -0.0010039353197059035, -3.313524741388463e-5]"},
  "telemetry_summary": {"samples": 1667, "max_interval_ms": 2.4, "stale_ticks": 0, "force_capped_ticks": 0, "dac_capped_ticks": 0, "max_abs_offsets": [0, 67, 0]},
  "auto": {"lockin_amplitude_counts": null, "noise_counts": null, "lockin_encoder": null, "lockin_pass": null, "sign_agrees": null, "liftoff_force_n": null, "k_marginal_n_mm": null, "oscillation": false},
  "operator": {"readings": [{"label": "dwell", "value": 0.2997494648604751, "unit": "N"}], "felt": null, "heard": null, "note": ""},
  "verdict": {"kind": "under", "pass": false, "rule": "pass: m >= 0.8c-0.05; over: m > 1.2c+0.05 or oscillation; none: below the pass band and m < 0.05 N (nothing delivered); else under; m 0.300 N of c 0.50 N, slope 0.60", "unlocks": ["Rung 1.0 N"]},
  "csv": {"file": "0016-ladder.csv", "sha256": "c71e286225bfb2b9d020698f73152568fc1aad0632808110f046a4bc41cd5685"},
  "claim": "Commands are model values; delivered force only from operator readings"
}
```

The example is `0016-ladder.json` from a smoke-test run against the simulated plant
(the simulator delivers 0.6 of the command, so a 0.5 N rung reads 0.30 N and scores
Under). It is regenerated, not hand-written; it is not a measurement.

The `claim` string is mandatory in every attempt file. The CSV has one row per servo
tick (thermal: one in ten, plus the final tick; see `params.csv_row_every`) with these
columns:

| Group | Columns |
|---|---|
| Timing | `t_s`, `interval_us`, `io_us` |
| Raw packet | `raw_hex` (all 16 bytes), `c0` to `c5` (raw i16) |
| Counters | `k0` to `k5` (continuous), `r0` to `r2` (registered) |
| Kinematics | `x_m`, `y_m`, `z_m`, `vx_m_s`, `vy_m_s`, `vz_m_s` |
| Demand | `demand_kind`, `demand_x`, `demand_y`, `demand_z`, `demand_ch0` to `demand_ch2` |
| Applied | `applied_x`, `applied_y`, `applied_z`, `off0`, `off1`, `off2` |
| Flags | `force_capped`, `dac_capped`, `slewing`, `expired`, `held`, `ui_age_ms`, `button` |
| Target | `target_x`, `target_y`, `target_z`, `phase` |

`phase` names the plan segment (ramp, dwell, zero, square_hi, square_lo, k=0.3). All 16
raw bytes are logged because bytes 12 and 15 are unidentified (P-0022) and the earlier
logs dropped them (force-audit F9).

## Appendix A. Owner runbook

Equipment: kitchen scale (1 g), push-pull gauge if available (0.01 N, 0 to 5 N) else the
scale on edge against a block, a rigid post, ruler and set square, 50 g and 100 g
masses on a light hook, a contact thermometer or phone, a phone audio recorder, a book
stack.

**Global stop.** Anything unexpected (grinding, smell, heat, motion you did not
command, growing oscillation) means: release the key, press the pen button, power the
amplifier off, and report the attempt id.

| Step | Action | Stop criterion |
|---|---|---|
| 1 | Power on with the arm supported. Start the Studio, Connect (output off). Check the rate badge (about 420 Hz) and that all six counters move | Rate under 300 Hz or any interval over 25 ms |
| 2 | Backdrive (motors off): pull the pen slowly with the gauge along X, Y, Z near mid pose; enter readings | Any reading above 0.2 N: inspect cables |
| 3 | Liveness on ch0, ch1, ch2 in turn: pen near the reset pose, fingertips only, phone 10 cm from the drive housing, hold 2 s, enter felt and heard | No channel passes: end the session. One or two fail: stop and report which |
| 4 | Registration: place the arm in the reset pose with the set square, hold still, Register. Move away and return | Return-to-reset disagrees by more than 2 mm: redo |
| 5 | Gravity drop: arm it in the Studio first, then release the pen from mid height onto your palm 2 cm below. Ruler: for X, Y, Z mark start, slide 50 mm along the square, mark end, enter mm. Record which physical direction is +X and +Z | Sign disagreement or off-axis above 15 percent. Scale outside 0.8 to 1.25 is warned, not a stop |
| 6 | Sign pulses X, Y, Z with the pen floated on a fingertip | Any disagreement: report, do not continue to rungs |
| 7 | Ladder rungs 0.1, 0.25, 0.5 N: Y with the pen pressing down on the scale (tare at zero command, -Y only); X and Z with the gauge against the post. Read the steady value during the dwell; enter it after release | Any reading above 1.2 x commanded + 0.05 N |
| 8 | Effects at 0.5 N (cube, plane, detents first): fingertip contact, then light grip. Report what you feel; this is the first moment the P-0033 question is answered | Anything unexpected |
| 9 | Rung 1.0 N on X, Y, Z (unlocks after step 7) | Same as step 7 |
| 10 | Lift test: rest the pen on the book stack at reset height, hands off. Run bare, then 50 g, then 100 g. The app records F_0 and the gain g = m g / (F_m - F_0), delivered per commanded; compare it with the ladder slope s = m / c, expecting g close to s | Arm walks sideways more than 25 mm or rises fast |
| 11 | Rung 2.0 N (Y on the scale is enough for session one). Optional in session one | Same as step 7 |
| 12 | Wall ramp (after step 9): touch the plane lightly, about 1 mm in, loose grip. Release at the first sustained buzz; the app also stops itself. Do not push deeper | First sustained buzz |
| 13 | Thermal (later session): pen braced down on the scale, 1.0 N for 10 min, Confirm every 30 s, enter casing temperature each minute at the same spot | Rise of 10 C |
| 14 | Commit the session directory to `docs/evidence/force-verification-<date>/` and add OWNER rows for liveness, registration checks, each rung slope, lift gain and K_marginal (see the [evidence guide](../evidence/README.md)) | n/a |

## What this specification does not claim

- It does not claim any force has been delivered. No step has run on the unit.
- It does not claim the 300-count ceiling equals 2 N or any other force. The gains are
  inferred (P-0028) and uncertain by about a factor of two.
- It does not claim the amplifier is enabled by the observed lifecycle. Enable is
  unproven (force-audit F6).
- It does not claim the Jacobian is correct for this unit. Gains and polarity were
  derived under a sandbox configuration (P-0013) and the swept spans in P-0044 differ
  from the nominal workspace; the sign and ruler checks test this.
- It does not claim a gravity model. Link masses for the 1.0 are NOT FOUND.
- It does not claim the passivity bound, the 0.5 to 0.8 N/mm expectation or the
  Over/Pass/Under thresholds are anything but project choices (P-0058, P-0063).
- It does not claim registration is absolute. The reset pose is hand-set and the error
  may reach 10 to 60 mm (P-0057); the ruler check bounds it.
- It does not claim the explanation of P-0033 by P-0060 is tested. It is an audit
  finding; only an attended session with the actuators confirmed live can test it.
