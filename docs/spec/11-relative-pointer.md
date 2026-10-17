# Relative pointer and force coordinates

The initial visual-only mapping used raw counter deltas (P-0024–P-0027).
Force feedback supersedes that mapping with the independently observed reference
kinematics in [12-force-feedback.md](12-force-feedback.md), P-0028–P-0029.
This makes position and force directions consistent. Registration at the reset pose is checked by the ruler and repeatability tests in
14-force-calibration.md; physical accuracy remains unverified until then.

The SDK retains all six counters and accumulates signed 16-bit differences using
wrapping subtraction. Motion between samples must remain below half the counter
range. The app computes the arm's model position from registered counts (continuous
counts minus the reset-pose capture, P-0062) and registers at the reset pose. Reconnect
clears registration and starts a fresh origin.

Default scene scaling is 40 scene metres per physical model metre. The existing
sensitivity control stores 0.004, multiplied by 10000 to obtain this scale.
Independent axis inversions and height/depth swap remain available, defaulting
to no inversion/no swap for the reference model. The inverse of this axis mapping
is applied to force commands. Clamp targets to X ±6, Y -0.3..8, Z ±5.

Mass Lab registers at the reset pose with "Zero at reset pose"; feedback stays
disabled until registered. There is no bottom-of-travel start and no recentre: the
registered origin is the pose of the arm at that capture, mapped to the scene origin
(0,-0.3,0). The floor and walls do not collide with the 0.2-metre-radius pen proxy. A
screen-space ring keeps it visible through geometry. The sphere shows the dynamic proxy;
the trail shows the measured target. The UI converts scene position to device metres
with the exact inverse of the position mapping above.

Force is evaluated at servo rate by the worker from a coupling descriptor, not once per
rendered frame: the spring/damper tether runs on each fresh sample, each request is
stamped at evaluation time because it is recomputed from the sample just read, and a
descriptor older than 100 ms is muted (P-0064; spec 12). Mute reasons are shown beside
"Pen force": paused, time scale not 1.00x (tolerance 0.005), unregistered, scene stalled
and feedback off.

Holding the observed active-low button grabs a nearby block. The lightest block
is free in translation and rotation, using a spherical grip. Other blocks move
only vertically, using a Y-only grip constraint. Release/lost input/deletion
removes the grip. Mass edits that change the free block also release it.

The worker reads independently of rendering and services device force output.
Pause, non-real-time simulation speed, a stale coupling, unregistered state or
disabled feedback send zero. Disconnect/errors close the device; graceful exit joins the worker after
zero/shutdown. Six raw counters and wrist deltas are exposed in the sidebar and
capture files. No axis meaning or rotational scale is assigned to the wrist
counters without evidence.
