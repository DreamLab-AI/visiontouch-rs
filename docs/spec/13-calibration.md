# Guided working-volume calibration

Design: P-0034. Reuses only the lifecycle and model in 12-force-feedback.md.

## Coordinates and capture

The native calibration app connects with motor output disabled. Instructions ask
the owner to rest the pen at the bottom of travel, centred horizontally/depth,
then record a bottom reference. Accept captures only after 250 ms of samples
whose reference-model position varies by at most 0.5 mm on each axis.
Accumulate all six wrapping counters; evaluate arm position from accumulated
counts. Reconnection invalidates the reference and all captured extrema.

Enter working-coordinate minimum and maximum for X (left/right), Y (bottom/top),
and Z (back/front), in mm. Defaults of -100 to +100 are editable example
coordinates, not manufacturer dimensions. Record the corresponding six physical
extrema by hand with output off. Bottom capture also records the Y minimum.
Captures are reference-model coordinates relative to that bottom pose, in mm.
Map each axis linearly between its measured extrema and its entered limits.
Do not clamp the displayed position: out-of-volume readings must remain visible.
Motor control uses the inverse map of working (0,0,0) to reference-model metres.
Require strictly ordered, finite bounds, at least 2 mm measured span per axis,
and entered zero strictly inside all three axes. This is a user-defined volume
mapping, not absolute homing, a physical length calibration, or proof that every
corner of the bounding box is mechanically reachable.

## Slow move against resistance

Explicit hold of Space or the move button starts one attempt. Require a fresh
reference, six valid captures, a stable starting pose inside measured bounds,
and no pressed pen button. Freeze the calibration/settings during the attempt.
Releasing hold, pressing Escape/Stop/the pen button, UI heartbeat age over 250 ms,
sample interval over 25 ms, or any transport error ends output. No auto-restart.

The reference-model target follows a straight line from the starting pose to
zero, at an adjustable 0.5–5 mm/s (default 2 mm/s). Limit target lead over the
measured pen to 2 mm so resistance cannot accumulate a large spring deflection.
Use a 150 N/m spring and 4 N s/m velocity damping. Default model-force vector
cap is 0.1 N; the owner can select up to the existing 0.5 N SDK cap. SDK 2 N/s
slew, 96-count channel clamp and 25 ms request expiry remain active. Quantities
labelled force are model commands, not measured forces. Do not raise limits in
response to a failed/stalled attempt.

Stop on model speed above 30 mm/s, pose outside captured bounds by over 2 mm,
or cross-track/reverse travel over 3 mm from the planned segment. Stop after
5 seconds without 0.25 mm progress toward zero; this is a resistance/stall
result, not a force measurement. Settle within 0.75 mm of zero at under 2 mm/s
for 0.5 s, then zero/disarm. Absolute attempt duration is bounded at 180 seconds.
Zero/disarm is attempted before writing logs or doing file I/O. Control runs on
its own worker; graphics and file writes do not generate servo requests.

## Evidence and persistence

Save entered bounds explicitly as JSON under captures/calibration/; loading them
never restores a live reference or arms output. After each attempt save metadata
(entered bounds, captures, reference counts, settings, result) and CSV containing
sample time, six raw/accumulated counters, model and mapped positions, servo
target, requested/guarded forces, bounded motor offsets and acknowledgement bytes.
These logs correlate commands and movement; they do not prove delivered forces.
App-level tests simulate both free movement and sustained user resistance, plus
release, stale input, travel faults, timeout, mapping and counter rollover.
Graphics smoke/demo modes never open hardware. Orderly exit joins the worker
after zero/close; software cannot cover SIGKILL or a stalled kernel.

## Switchable canonical effects

The owner also requests centre-mounted cube, water sphere, deformable/popping
balloon and three-body spring/central-gravity demonstrations. Add a plane,
viscous field and magnetic detents for directional/damping checks. These are
original application models, not additional protocol knowledge. Centre every
effect at the inverse-mapped working origin; size it from the recorded volume.
Use model metres/Newtons internally. Render both measured pen and contact proxy;
the force/position caps remain visible. A hard-cube model is compliant and bounded
by the SDK; do not claim a physically rigid wall.

Cube/plane compute spring penetration plus inward-velocity damping. Water blends
linear/quadratic drag and a small upward force by immersion. Balloon develops
an outward spring reaction, visibly deforms and irreversibly pops past 35%
radial indentation until reset. Three equal masses are linked pairwise by
damped springs, attracted by softened inverse-square central gravity, integrated
in <=1 ms substeps; pen/body contact transfers equal/opposite force. Viscosity
opposes velocity everywhere; detents attract toward smooth periodic wells.
Reset state on selection/reset/attempt start. Effects use the same hold, UI/input,
volume/speed and duration interlocks; the move-only stall/cross-track criteria do
not apply to free exploration. Log effect identity and state. Test force direction,
contact proxy, pop/reset, damping and three-body integration without hardware.

## Independent command-limit release policy

The calibration app configures a stricter SDK force cap, with a 250 ms sustained
command-limit trip. Demand above that cap, or a required actuator offset outside
the configured DAC limit, starts the timer. Returning within both limits resets
it. At expiry the guard outputs zero and latches a command-overload fault; the
device ends the output session and the app requires a new held attempt. This
is an original conservative software policy, not a recovered vendor threshold
or a measured physical overload. The existing SDK default continues clamping
unless a caller explicitly selects a release policy. Its public configuration
can only lower the existing force, slew and DAC ceilings. Record unclamped demand
separately from guarded force and command-limited status.

## Continuous coordinates and failed shutdown

The SDK must use the same unwrapped absolute encoder history for pose and force
transforms. Recentering a relative display must not change this physical model
history. Signed-counter rollover must not discontinuously change the Jacobian.
A command-limit trip is recoverable on the same connection only when shutdown
I/O succeeds. Any shutdown failure takes precedence over that software fault,
invalidates transport health, and requires closing/reconnecting.

Calibration evidence can be exported while output is off, before attempting
force feedback. Include entered bounds, measured extrema, bottom model reference,
raw reference counters and timestamp. Evidence exports are not loadable arming
credentials: a fresh connection still requires fresh physical captures.

## Motor-off workspace sweep (supersedes six-limit live capture)

Record bottom dead centre, then collect a spatially deduplicated cloud while the
operator sweeps comfortable travel at several heights/depths. Preserve observed
model points, derive axis extrema for display only, and show the actual point
cloud without treating its enclosing box or convex hull as reachable volume.
Capture a chosen interior zero manually with output off; entered working labels
are optional and model millimetres about that zero are the default. Export the
cloud, extrema, reference and selected zero as input-only evidence, including
incomplete scans. Bound storage and reset observations on a new reference or
connection. Stop and disconnect end acquisition; they never arm motors.

Boundary-only sampling cannot prove interior reachability or a safe straight
path. This acquisition stage therefore blocks force attempts for swept maps;
path confirmation and conservative usable-region validation remain required
before these maps can authorize powered tests. Report sampled-cell volume only
as observed coverage, never as the full reachable volume.

## Offline arm and shape reconstruction

A local Three.js viewer combines owner photographs with the existing reference
kinematic convention. Cosmetic base/frame/housing/gimbal dimensions are inferred,
not measured; wrist channels remain unassigned. Import baseline CSV (including
unwrapped counts), or native sweep evidence. Preserve original raw evidence and
SHA-256; bundle spatially deduplicated points and timestamped replay frames.
Render sampled cells without filling unknown cells; expose convex hull only as
an optional outer comparison bound, never as motor authority. Support orbit,
pose replay, adjustable link lengths and GLB export. No hardware interface in
this viewer. A stable final resting pose labelled by the owner is provisional,
not an automatic mechanical-origin calibration.

## Sample-supported path review

Provide a reusable, hardware-free workspace geometry crate. Its evidence region
is the union of fixed-radius neighbourhoods of observed points, reduced by an
explicit evidence margin. Defaults are a 5 mm model support radius and 2 mm model
margin (3 mm effective proximity). These are independent review parameters,
not measured clearance, manufacturer limits or proof of physical safety.
Do not fill convex interiors or bridge unsupported gaps.

Check a proposed straight segment analytically: intersect the complete segment
with each evidence sphere, sort the covered parameter intervals, and require
their union to cover [0,1]. Checking only endpoints or fixed-spaced test points is
insufficient. Return the first unsupported interval for display. Non-finite,
out-of-range or oversized inputs fail closed. An explicit conservative numerical
inset avoids accepting marginal floating-point boundary contacts.

The native app may review a straight path from the current still pose to the
manually recorded zero while motors are off. Show the path and its first gap;
export support radius, margin, sample count and the exact reviewed endpoints.
Any added observation or changed zero invalidates the review. Review results
are evidence about model-sample proximity, never persisted arming authority.
Sweep motor control stays locked until the full physical/temporal validation
workflow is complete. Preserve the difference between geometry support and a
physically validated usable region in both UI and crate docs.

The explicitly observed still start and manually observed zero are included as
support observations even if the sweep's spatial deduplication/storage cap did
not retain a cell there. Export the resulting support observation count. These
endpoint observations do not fill any intervening unknown interval.

## Live manual approach teaching

Replace the initial blanket sweep-output lock only after a fresh motor-off,
out-and-back teaching trace on the current connection. The selected zero and
start must be distinct (5–500 model mm). Record a continuous hand-guided approach
to zero, a still dwell, return to start and another still dwell. Require every
sample interval in (0,25 ms], raw model speed <=60 mm/s, perpendicular/endpoint
overshoot <=2 mm, each dwell >=0.3 s, total duration <=180 s, and a live UI
heartbeat. Pen button, Stop, connection/reference changes, input faults and
calibration edits invalidate an unfinished trace. Preserve time-stamped taught
points, with bounded 0.5 mm spatial retention; all samples still undergo checks.

After completion, analytic sample support must cover the entire straight route.
Keep confirmation only in worker memory, with a five-minute expiry. Store trace
and policy as evidence, never reloadable arming authority. A zero-move attempt
must start still within 0.75 mm of its taught start and use the exact taught zero.
Other held demos require the same fresh confirmation and current membership in
the sample-supported region. Every active sample must remain supported; leaving
it releases output. Apply existing hold/UI/velocity/transport/stall guards and a
lower 0.1 model N maximum for swept-map pilot attempts. Use a <=20 mm model effect
radius. No unit is powered or driven during offline implementation/testing.

Manual tracing is evidence of hand-guided motion, not validation of force
magnitude, thermal safety or hard real-time control. Actual controlled resistance
and overload tests remain necessary with the owner present. A stored or synthetic
trace can never create live motor authorization.

## Sweep extrema independent of display thinning

Update all six observed extrema for every finite, in-range input passed to the
sweep, including observations sharing a retained 2 mm cell. Keep these exact
model-coordinate extrema alongside the bounded display cloud and export them.
Compute spans and coordinate bounds from this accumulator in constant time;
spatial thinning must not hide the last portion of travel within a cell.
A changed extremum invalidates an earlier path review. Extrema remain display
and mapping evidence only: never insert enclosing-box corners into path support.
The point-storage cap remains unchanged; additional observed extrema do not imply
that unretained points are available for continuous-path support.

## Freehand reverse route (supersedes straight out-and-back teaching)

Owner requests a different teaching method after the straight-line workflow
proved impractical. With motors off, one explicit action records the current
stable interior pose as working zero and starts a freehand trace. The operator
moves along a comfortable route to the desired starting pose, holds still and
selects Finish. There is no prescribed straight line, return leg or automatic
endpoint dwell to chase. Finishing requires a stable pose 5–500 model mm from
zero. Every live sample remains validated; retain points at 0.5 mm spacing plus
both endpoints. Allow input-only sample gaps up to 100 ms, but no positional
jump above 3 mm or model speed above 60 mm/s. Abort on longer delays, invalid
samples, UI loss, pen button, Stop or duration above 180 s. Powered-loop timing
limits remain 25 ms. Export method/version and the ordered reversed route, plus
the original timestamped observations. Export the latest route status even when
recording failed, so failures can be diagnosed without guessing from path review.

Validate every adjacent route segment against sample support, never the direct
start-to-zero shortcut. The powered planner follows cumulative distance along
this polyline. Limit local position projection to ±3 mm around prior progress to
avoid jumping across nearby loops. Advance target at selected speed and at most
2 mm ahead of projected pen progress. Preserve 3 mm route-deviation stop, 5 s
progress stall, 180 s total timeout and final 0.75 mm/0.5 s settle check. Require
target progress to reach the final route segment before settling at zero, so a
loop passing near zero cannot finish early. Existing force, hold, expiry, sample
support and transport guards remain. Saved routes never restore live authority.
Show the recorded return route and put the two route actions prominently above
the held-experiment controls. Existing straight-path review remains optional
geometry analysis, distinct from the freehand recording workflow.
