---
id: ADR-0004
title: Register the arm at the published reset pose; remove the bottom-reference, box, sweep-authority and route workflows
date: 2026-10-17
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: 5e658b8e02f094f022eb7268b766489a9c2b0206
verified_paths: [crates/device/src/lib.rs, apps/calibration/src/registration.rs, apps/calibration/src/arm.rs]
owner: jjohare
review_trigger: an OWNER ruler or repeatability record outside the acceptance band, or a physical measurement of the link lengths
repo: visiontouch-rs
---

# ADR-0004: Register the arm at the published reset pose; remove the bottom-reference, box, sweep-authority and route workflows

## Context

The encoders are incremental, so absolute joint angles exist only after the arm
is zeroed in a known pose. Published practice for this device family zeroes at the
pose where all links are orthogonal: near arm horizontal and square to the base
front, far arm hanging plumb (P-0049 to P-0051). The repository's kinematic model
already places its origin at that pose (counts zero give the tip at the origin
with the far arm plumb). The 2026-10-16 workflows instead took a "bottom of
travel" rest as the reference, mapped a hand-entered box onto it, and gated motor
output on swept clouds and taught routes. None of those steps registers the model
to the arm, so every force command evaluated the Jacobian at unregistered angles
(force-audit F8, P-0060).

## Decision

Registration is an SDK concept: `Device::register_reset` records all six
continuous counters while feedback is off and the arm is held still at the reset
pose; positions and the Jacobian are computed from counts minus the reset counts;
a reconnect clears it because a reconnect is a new device. Cartesian force demand
is refused while unregistered; the liveness channel test is not. The Calibration
Studio draws the arm as a registered wireframe model (base, shoulder, upper link,
elbow, lower link, tip, gimbal stub) with a ghost of the reset pose, and validates
registration by three discriminating checks: return-to-reset repeatability within
2 mm, gravity-drop sign (a released pen moves along negative Y), and a per-axis
50 mm ruler move with scale in [0.8, 1.25] (warned, not gating) and off-axis motion at most 15%
(P-0057, P-0062). The motor-off sweep survives only as a 2 mm-cell coverage cloud
for display and viewer export. The bottom reference, six-bound box, working
coordinates, slow move to zero, straight and freehand route teaching, five-minute
authority, sample-support gating and in-app path review are deleted, together
with the `haptics-workspace` crate whose only consumer they were.

## Consequences

The operator needs a set square and a ruler, and must reproduce the reset pose by
hand each session; registration error of the order reported in the literature
(10 to 60 mm at the tip before refinement) is expected and is why the ruler check
exists. Link lengths remain the reference model's 0.1397 m until measured. The
workspace viewer keeps its own JavaScript kinematics and loses the
bottom-reference import branch. P-0034, P-0042, P-0045 to P-0048 become history.

## Verification

Recorded at the commit above: `cargo test -p visiontouch-device
-p visiontouch-studio` including `registered_reset_pose_is_origin_and_reconnect_clears`
and the registration module tests (stillness, ruler scale and off-axis,
gravity-drop sign, repeatability); `arm.rs` matches `geometry.mjs` at counts zero
and at the P-0028 fixture pose.
