# Calibration Studio

A native Rust calibration and haptics workbench built on `haptics-device`,
`haptics-effects`, macroquad and egui. The servo worker owns device I/O and motor
requests; graphics sends operator intent and displays snapshots.

```sh
# Offline, mouse-driven preview; never opens the device
tools/run-calibration --demo

# Start offline and explicitly connect through the interface
tools/run-calibration --no-device

# Connect the parallel device with motor output OFF
tools/run-calibration
```

The normal launcher refreshes the current user's parallel-port group access.
The demo and smoke paths do not invoke that helper. `--no-device` refreshes
permissions for a later explicit connection but opens no device at launch. A connection
never enables motor output. No test in the verification commands below opens
hardware.

## Sweep calibration (default live workflow)

1. With output off, rest at a comfortable bottom-centre pose, hold still and
   capture the bottom reference. This is an operator reference, not a hard stop.
2. Select **Start / continue sweep**. Move through comfortable arcs at multiple
   heights and depths. Points are retained once per 2 mm model cell, up to 20,000.
   Exact observed extrema are tracked separately, including movement within an
   existing cell; display thinning does not shrink the recorded axis spans.
3. Select **Finish sweep**, move to a comfortable central pose, hold still and
   **Record this pose as zero**. Extrema and coordinate spans are derived by the
   app. Coordinates default to model millimetres about this chosen zero.
4. **Export sweep evidence** retains the cloud, extrema, bottom reference and
   chosen zero. Incomplete scans can also be exported. Reconnect or recapture
   invalidates the old live mapping.

The actual point cloud is displayed. Its enclosing box is only a display extent;
empty space is unverified. Sweep maps require fresh live route teaching before
any held pilot attempt (see below). Physical force and overload validation remain
outstanding. The
advanced six-bound workflow remains available for controlled box-based tests;
it does not certify the corners as reachable. No scan restores motor authority.

For an offline UI check, use `tools/run-calibration --sweep-smoke`.
The [Three.js viewer](../workspace-viewer/README.md) replays recorded arm motion
and displays observed-cell surfaces and a separately labelled convex bound.

## Advanced six-bound calibration

Select **Advanced: six-bound box calibration** and capture a fresh bottom
reference to start this workflow. Existing sweep observations never authorize it.


1. With motor output off, place the pen at the **bottom of travel**, centred
   left/right and front/back. Keep it still for 0.3 seconds and select **Capture
   bottom reference**. The app also records the bottom Y limit.
2. Enter your working-coordinate minimum and maximum in mm for X (left/right),
   Y (bottom/top), and Z (back/front). Each minimum must be negative and each
   maximum positive. The initial −100/+100 values are example coordinate labels,
   not manufacturer dimensions or measured lengths.
3. Follow the on-screen guidance to record left, right, top, back (away from
   you), and front (toward you). Move by hand with output off, use comfortable
   limits, and hold still before each capture. Do not push mechanical stops.
4. Confirm all six captures are present and the mapping is valid. The model
   maximum must exceed the minimum by at least 2 mm on each axis. A failed
   direction check is a reason to inspect and recapture that axis.
5. Choose **Slow move to (0, 0, 0)** or a centre-mounted demonstration. Keep the
   pen still inside the recorded volume before starting.
6. **Hold Space or the hold button** for one attempt. Releasing, Escape, Stop,
   or the pen button ends output. A held key cannot restart a failed attempt:
   release it and start a new hold. Settings and calibration are frozen while
   held. Closing the window clears hold, stops the worker and joins its thread.

Start the slow move at its default 2 model mm/s and 0.1 model N cap. The target
cannot lead the measured position by more than 2 model mm; sustained resistance
without progress ends the attempt after five seconds. Reaching zero and settling
also ends output. The app does not increase force to overcome resistance.

## Centre demonstrations

| Scene | What to explore |
| --- | --- |
| Hard cube | Compare resistance on all six faces; distinguish measured pen penetration from the contact proxy. |
| Water sphere | Compare slow/fast drag and the small upward force inside the sphere. |
| Balloon | Push inward slowly; deformation grows until a pop at 35% indentation. Release and hold again to reinflate. |
| Three-body springs | Nudge three masses linked by pairwise springs and attracted toward a central anchor. |
| Virtual plane | Compare vertical contact resistance with free tangential motion. |
| Viscous field | Move in every direction, then stop; drag opposes velocity and vanishes at rest. |
| Magnetic detents | Feel periodic, smooth wells along each axis. |

Each scene resets at the next held attempt. Reset Scene clears the offline preview
and trail. The hard cube and plane are bounded compliant models, not a claim of
physically rigid surfaces. The app reuses the same hold, timing, volume, velocity
and SDK guards for the physical experiments.

Mint marks the pen, gold marks the contact proxy / moving target, and the orange
arrow shows guarded model force (8 view units per model N). The volume is drawn
in your working coordinates, which can stretch physical geometry when the axis
mappings differ. Positions outside bounds remain visible. Right drag orbits,
wheel zooms and Home restores the camera. In demo mode, left drag moves a virtual
pen in X/Y and **Left Shift + wheel** adjusts depth; hold Space to simulate the
selected effect. Demo telemetry and encoder placeholders are labelled simulation.

## Baseline records

**Save bounds** writes entered coordinates and selected speed/force settings to a
named JSON file in `captures/calibration/`. **Load bounds** loads only those
entries; it cannot restore a physical reference, measured extrema or output.
Reconnect and bottom recapture invalidate the physical mapping as appropriate.

Every physical held attempt writes a timestamped CSV and JSON metadata after
output is disabled. The diagnostics show the last CSV path. After releasing output, use the operator
observation card to annotate that attempt: Quiet, Normal, Laboured, Chatter or
Other, plus free text. Optional optical temperature entries accept ambient °C
and up to five user-labelled surface readings; blank values remain unset. Add
sensor/target/settings/time notes, then select **Save observation with last
attempt**. This writes `<attempt>.csv.observation.json` with an epoch timestamp.
For the owner's Pixel 9 Pro, use **Object temperature**, hold the sensor beside
the rear camera about 5 cm from the selected surface, and record the material
setting and exact target. The app pre-fills these sensor notes for editing; it
does not access the phone or import readings. See [Google's object-temperature
instructions](https://support.google.com/pixelphone/answer/14103759?hl=en-uk).
The ambient field is a separately obtained ambient reading, not an inference
from the phone's object-temperature measurement.

The optional **Audio recording (WAV)** field stores a reference to a separately
recorded file, alongside the sound label for later baseline comparison. It never
opens the microphone, starts recording, or uses acoustic analysis to control
motors. An empty audio field remains unset.

These are manually entered surface readings and listening impressions, not
winding-temperature, force or recorded acoustic measurements; the app invents
no temperature thresholds. Saving again updates that attempt's observation. Logs include raw and
unwrapped counters for all six channels, model/working coordinates, target,
requested and guarded model forces, motor offsets, acknowledgements and effect
state. Wrist channels are observed; direct wrist torque is not implemented.

The force chart displays command magnitudes, **not load-cell measurements**.
The SDK applies its unit-wide `UNIT_DERATING` gain once across all applications;
the diagnostics display that gain and the effective selected cap. Changing this
constant scales the existing conservative envelope, not a manufacturer rating.
Command caps, slew limits, deadlines and latched command-limit release do not
measure the force actually delivered by the pen. The current Cartesian model
and physical contact response remain under investigation. A user-defined box is
not an absolute homing procedure, dimensional calibration, or proof every corner
is mechanically reachable. See [the calibration specification](../../docs/spec/13-calibration.md)
for exact interlocks and known limitations.

## Offline verification

```sh
cargo test -p haptics-calibration -p haptics-effects
cargo clippy -p haptics-calibration --all-targets -- -D warnings
cargo build -p haptics-calibration
xvfb-run -a -s '-screen 0 1480x960x24' \
  env LIBGL_ALWAYS_SOFTWARE=1 \
  CALIBRATION_SCREENSHOT=/tmp/calibration-studio.png \
  target/debug/haptics-calibration --smoke-test
```

The graphics smoke test renders every scene, exercises the zero planner and
coordinate mapping, rejects an incomplete mapping, displays telemetry and
hardware-disabled controls, then exits automatically. `--demo` and `--smoke-test`
do not even spawn the device worker. The screenshot environment variable exports
a rendered frame; the app never captures the desktop. These checks cannot
establish physical force or release behaviour.

**Export calibration evidence** saves the measured extrema and bottom reference
with entered bounds while output is off. This record cannot restore an armed
calibration; each new connection requires fresh physical captures.

Attempt metadata includes the start wall-clock timestamp; CSV rows include
monotonic elapsed time, servo interval, sample I/O time and command-limit status.
Use a recorded synchronization cue or measured offset to align an external WAV;
the wall-clock stamp alone does not establish microphone sample synchronization.

The SDK reads a packet and services the previously queued force request before
the worker evaluates the new position and interlocks. A stop therefore has
servo-cycle and transport latency; it is not instantaneous mechanical isolation.
Neither a process stall nor a failed shutdown transaction establishes zero
physical motor force. Shutdown I/O failure invalidates the connection.

## Reviewing a proposed straight path

After recording zero, place the pen at a proposed starting pose, hold still and
select **Review straight path from this pose**. The gold line is the exact
reviewed segment; red marks its first unsupported interval. Moving the pen does
not move those reviewed endpoints. New sweep samples or a changed zero invalidate
previous review evidence.

The hardware-free `haptics-workspace` crate merges analytic sphere/segment
intersections over the entire line. The current review uses 5 mm neighbourhoods
minus a 2 mm evidence margin, with a tiny conservative numerical inset. Recorded
samples and the explicitly observed still endpoints supply evidence. These are
model-space proximity parameters, not physical clearance or force limits. A
supported review alone does not authorize powered movement.


## Teach a live pilot route

With motors off, move to a still starting pose 5–500 model mm from zero and select
**Teach approach: to zero and back**. Follow the gold line to zero slowly, hold
still, return along the same line, and hold still again. On-screen instructions
show the current stage. Stay within 2 model mm of the line and below 60 model
mm/s. Delayed input, a UI timeout, the pen button or Stop cancels teaching.

A completed trace must support the entire straight segment. Its live confirmation
expires after five minutes and is discarded on reconnect or calibration changes.
A saved trace cannot restore it. During this window, a fresh held zero attempt
requires the pen within 0.75 model mm of the taught start; held objects require
a still pose inside the sample-supported region. Pilot commands are capped at
0.1 model N, with object radius at most 20 model mm. Leaving sample support or
expiry during an attempt stops output, alongside the existing interlocks.

The trace and support policy are exported with attempt metadata. This workflow
has been tested offline only: hand-guided motion does not verify physical force,
thermal limits, or overload release. Run controlled physical checks with the
owner present before interpreting the demos as validated device behaviour.
