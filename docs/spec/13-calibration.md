# Registration, coverage display and effects

Design: P-0062 (registration), P-0035 (effects). Replaces the 2026-10-16 working-volume
calibration (P-0034, P-0040 to P-0048). Force verification is specified in
[14-force-calibration.md](14-force-calibration.md); the envelope in
[12-force-feedback.md](12-force-feedback.md). Decision record: ADR-0004.

## Registration

Registration binds the model frame to the arm at the reset pose: near arm horizontal and
square to the base front, far arm plumb (P-0049 to P-0051). The owner places the arm,
holds still, and the host captures all six continuous counters after 250 ms with at most
0.5 mm of model variation per axis. Registered counts are continuous minus the capture;
position and Jacobian are evaluated from them for the three position channels only.
Cartesian force is refused while unregistered, and reconnection clears registration.
Checks (stillness, return-to-reset repeatability within 2 mm, gravity-drop sign, ruler
scale 0.8 to 1.25 with off-axis at most 15 percent) are defined in spec 14 section 4.
Hand placement can leave 10 to 60 mm of tip error (P-0057); the ruler check measures
it. Registration is not absolute homing and not proof of reachable volume.

## Coverage display

The Studio may accumulate a bounded cloud of observed positions in 2 mm cells while the
arm is moved by hand with output off. It is display only: it never gates, authorises or
shapes motor output. Unknown cells are never filled. It exports the viewer's existing
schema-1 workspace capture (`points`, `frames` with `t` and `counts`) using registered
counts. Spatial thinning must not lose extrema of the observed extent (P-0047). A
stable final pose labelled by the owner is provisional and is not a mechanical origin.

## Switchable canonical effects

Centre-mounted cube, plane, water sphere, balloon, three-body springs with central
gravity, viscous field and magnetic detents are original application models, not
protocol knowledge (P-0035). Effects centre on the registered origin, use model metres
and newtons internally, and render both measured pen and contact proxy with the force
caps visible. Cube, plane and balloon use a stiffness parameter (default 150 N/m, clamp
20 to 1500); other effects scale with it. A hard cube is compliant and bounded by the
SDK; never claim a physically rigid wall. Effects are commands, capped by the unlock
ladder (spec 14 section 7) and never above `CONTINUOUS_FORCE_N`. Effect state resets on
selection, reset and attempt start, and effect identity and state are logged.

## Removed workflows

The following 2026-10-16 workflows are removed: the bottom-of-travel reference, the six
entered bounds and working coordinates, the slow move to zero, straight and freehand
reverse-route teaching, the five-minute sweep authority expiry, sample-supported path
review and the 0.1 N pilot cap. They gated output on evidence that registered nothing
and, with the sustained-command release, made every move end after about 0.6 s (P-0060).
Their rows (P-0034, P-0040 to P-0048) remain as history. Replacement: ADR-0004.
