---
id: ADR-0002
title: Bound motor output with a DAC-derived software envelope that applications can only tighten
date: 2026-10-17
decision_status: proposed
implementation_status: none
activation_status: inactive
supersedes: []
superseded_by: []
verified_commit:
verified_paths: [crates/device/src/lib.rs, crates/hal/src/device.rs]
owner: jjohare
review_trigger: the first OWNER delivered-force record (gauge slope or lift gain), or any change to crates/hal MAX_MOTOR_OFFSET
repo: visiontouch-rs
---

# ADR-0002: Bound motor output with a DAC-derived software envelope that applications can only tighten

## Context

The unit is about 25 years old; its amplifier gain, thermal protection and
watchdog are uncharacterised, and no delivered force has ever been measured. The
force-to-DAC gains came from black-box queries (P-0028) and are uncertain by about
a factor of two (`docs/research/historical-calibration-practice.md` section 3.4).
The 2026-10-16 envelope (0.5 N, 2 N/s, 96 counts, a 250 ms sustained-command-limit
release) was safe but useless: the audit (P-0060) shows the release pre-empts every
calibration move and the layered caps put commanded force below published backdrive
friction (P-0053). The only model-independent physical bound is the DAC word.

## Decision

The transport clamps every actuator channel to ±`MAX_MOTOR_OFFSET` = 300 counts of
the 2047-centred word (14.7% of half-span; 2 N × 134 counts/N on joint channels 1 and 2
at any pose, with margin) in `crates/hal/src/device.rs`. The SDK ceilings
in `crates/device/src/lib.rs` are derived from that bound: `MAX_FORCE_N` 2.0,
`MAX_DAC_OFFSET` equal to the transport clamp, `MAX_SLEW_N_S` 100, `COMMAND_TIMEOUT`
25 ms. `ForceLimits::default()` is the envelope already armed live (P-0030):
0.5 N and 96 counts. `CONTINUOUS_FORCE_N` 1.0 caps any content without a fixed
dwell. DAC limiting scales the force vector uniformly so direction is preserved.
A raw channel demand for actuator liveness bypasses the force model and slew but
not arming, expiry or a 96-count cap. Cartesian demand is refused until the device
is registered at the reset pose (ADR-0004). There is no sustained-command-limit
release and no recoverable force fault: every sampling error is a transport fault
that ends the session, and a failed shutdown outranks a rejected request.
`ForceLimits` can only lower the ceilings. Raising an SDK ceiling is an ADR citing
PUBLIC characterisation and a DAC bound; raising an application default or
`CONTINUOUS_FORCE_N` needs OWNER or OBSERVE delivered-force evidence.

## Consequences

Applications inherit the proven 0.5 N / 96-count envelope by construction; only
the Calibration Studio constructs higher limits, inside fixed-dwell verification
steps after lower rungs pass in the same session (ADR-0003). Walls can be stiff
enough to feel because slew no longer dominates onset. The first attended session
can establish liveness without any registration. Mass Lab's 0.02 scene scale and
its frame-rate request path are deleted in favour of a servo-rate coupling
descriptor (P-0064). Numbers reported by this path remain commands, not measured
forces, until the verification ladder produces OWNER rows.

## Verification

To be recorded when the implementation lands: `cargo test -p visiontouch-device
-p visiontouch-hal` including `default_limits_are_the_p0030_envelope`,
`limits_cannot_exceed_ceilings`, `dac_scaling_preserves_direction`,
`channel_demand_bypasses_slew_but_not_expiry_or_96_cap`,
`force_requires_arm_registration_and_expires_to_zero` and
`shutdown_failure_outranks_rejection`; strict rustdoc. No delivered-force claim
exists at this record's commit.
