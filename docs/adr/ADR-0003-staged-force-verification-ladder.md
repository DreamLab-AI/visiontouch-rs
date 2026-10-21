---
id: ADR-0003
title: Verify force output through a staged, per-session unlock ladder before any demo or application feels it
date: 2026-10-17
decision_status: accepted
implementation_status: complete
activation_status: staged
supersedes: []
superseded_by: []
verified_commit: 3f9f2dd8657fc4cadb29e29aea3a6b80b5991c91
verified_paths: [apps/calibration/src/protocol.rs, apps/calibration/src/controller.rs, docs/spec/14-force-calibration.md]
owner: jjohare
review_trigger: a committed OWNER verification record (docs/evidence/force-verification-*), after which the per-session ladder may become a spot check
repo: visiontouch-rs
---

# ADR-0003: Verify force output through a staged, per-session unlock ladder before any demo or application feels it

## Context

Three powered sessions in 2026-10-16 produced acknowledged packets and no
perceptible force (P-0030, P-0033). The audit (P-0060) found software causes but
could not rule out an unenabled amplifier or wrong registration (force-audit F6,
F8). The published practice for this device family is a sequence of discriminating
physical tests: backdrive baseline, sign check, static force against a gauge,
hanging mass, virtual-wall stiffness ramp and thermal hold
(`docs/research/historical-calibration-practice.md` section 4, P-0053 to P-0058).
The previous calibration app offered a slow move to zero and taught routes instead,
none of which can distinguish "no force" from "too little force".

## Decision

The Calibration Studio implements `docs/spec/14-force-calibration.md`: steps run as
held attempts with operator interlocks, each writing a schema-2 evidence record,
and a `Session` unlock ladder gates every step on what has already passed on this
connection. Actuator liveness (±80-count square wave on one channel, inside the
proven 96-count envelope) runs first and needs no registration. Sign pulses,
0.1 to 0.5 N ladder rungs and effects at 0.5 N require liveness, registration and
the sign checks. The 1.0 N rung requires the 0.5 N rung on all three axes; the
2.0 N rung, the lift test to 1.5 N and the wall ramp require the 1.0 N rung on all
three axes; effects never exceed 1.0 N. A rung is scored from
the operator's reading against the command c: Pass within [0.8c - 0.05, 1.2c + 0.05] N
with no oscillation; Over above that band or on oscillation; Under below the band
with at least 0.05 N read; None when a completed dwell reads below 0.05 N absolute
and the band is missed (nothing delivered). Pass and Under clear the rung: an Under
is recorded with its slope and does not block, because the DAC bound delivers less
rather than more. None neither clears the rung nor locks. Over on any rung, axis or
sign locks every force step for the whole connection: re-registration does not
clear it, only a reconnect does. Only a completed dwell can be scored; an attempt
stopped early for any reason grants nothing. Anything above 1.0 N exists only
inside fixed-dwell steps of at most 10 s. No unlock persists across connections or
restarts; saved evidence never arms output. Readings are typed only while output
is off and are keyed to the attempt id.

## Consequences

The first attended session answers the liveness question in minutes and produces
the first delivered-force numbers (gauge slope per axis, lift gain) that can
replace the inferred gains and set `UNIT_DERATING` by a later ADR. Mass Lab runs at the P-0030 default (0.5 N, 96 counts) and never above
`CONTINUOUS_FORCE_N`; only the Studio's effects are ladder-gated. The owner must
own a gauge or 1 g scale, 50 g and 100 g masses, a ruler and a set square. The
workflow is slower than "hold Space and feel", by design. Thermal testing is a
separate Confirm-driven attempt class because a 600 s hold cannot live inside a
key-held 180 s interlock.

## Verification

Recorded at the commit above: `cargo test -p visiontouch-studio`
(protocol unlock table, controller interlocks, evidence round trip, smoke run of
every step against the simulated plant). Physical verification is the owner's
session per the spec appendix; its records become OWNER rows and a
`docs/evidence/force-verification-<date>/` directory.

2026-10-21 verification: the full quality gate and 25-attempt simulator smoke
run pass with the spec 18 gradual channel diagnostic. Dedicated tests cover
signed ramp endpoints, registration and fresh-hold prerequisites, immediate
stop paths, recorded observations and absence of force-step unlocks. The fixed
40-count diagnostic does not change force ceilings or prove physical motion.

The P-0069 boundary interpolation correction also passes the full gate and
25-attempt simulator smoke run. Regression tests cover irregular cadence,
startup, duplicate timestamps and a preserved registered owner-host trace. That
trace still triggers the unchanged 50 mm/s speed stop. No new physical trial or
force calibration is claimed.
