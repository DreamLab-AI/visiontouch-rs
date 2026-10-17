# Mass Lab

A native Rust desktop app for exploring rigid blocks under gravity. Macroquad
renders the 3D scene, egui supplies the controls, and Rapier simulates collisions,
friction, restitution, rotation and mass-dependent impulses. The minimum-mass
block is free to move and spin; all others move only vertically. Ties select the
lowest block ID. Adding/removing blocks or editing masses updates this choice.

## Run

From the repository root:

```sh
scripts/run-mass-lab
# Simulation only, without opening the device:
cargo run -p mass-lab -- --no-device
```

Requires Rust 1.89 or newer and an OpenGL-capable desktop. Tested on this machine's
Hyprland desktop through XWayland, and with Mesa software rendering under Xvfb.
There are no downloaded scene assets or runtime vendor-library dependencies.
A **Mass Lab** desktop launcher has also been installed for John. It points at
this checkout's `target/debug/mass-lab`; rebuild after changing the code.

The device-connected app starts running with equally sized 0.5 kg, 2 kg and 10 kg blocks at the same
height. Simulation-only mode starts paused; click **Run simulation** to release them. Mass does not affect free-fall
acceleration in this model; it affects momentum, impulses and collision response.

## Controls

| Action | Control |
|---|---|
| Run / pause | Space or the top button |
| Reset current scene | R or Reset |
| Advance one physics step while paused | N or Step |
| Orbit camera | Right mouse drag |
| Pan camera | Middle mouse drag |
| Zoom | Mouse wheel over the scene |
| Restore camera | Home or Home view |
| Select a block | Left click on the block or its sidebar row |
| Remove selected block | Delete or Remove |
| Zero at reset pose (register the device origin) | C or Zero at reset pose |
| Close | Escape or the window manager's close action |

Scroll the sidebar for the impulse experiment, block spawning and view controls.
The presets are **Mass comparison**, **Stack of blocks**, and **Mixed playground**.
Mass edits update the body's mass and inertia immediately. **Push all** gives each
block the same upward impulse, so its velocity change is `impulse / mass`.
Gravity supports zero, Moon, Earth, and a custom slider. Changing gravity or surface
properties wakes sleeping bodies. Reset preserves world settings and pauses.

## Simulation details

- SI units: metres, kilograms, seconds, and newton-seconds.
- Fixed 1/120-second physics steps, independent of rendering frame rate.
- Slow frames cap catch-up work at 0.1 seconds to keep the UI responsive;
  simulation time can lag wall-clock time under sustained load.
- Continuous collision detection, a floor, and low boundary walls.
- Maximum 80 blocks; blocks that fall more than 30 metres below the floor are removed.
- No air resistance or artificial block damping. Contact friction, restitution
  and the pen coupling dissipate energy. Linear and angular momentum persist
  when no external force/torque acts.
- Ground markers are visual aids, not physically calculated shadows.

## Live haptic pointer

The app now connects to `/dev/parport0` through `visiontouch_device::Device`
and shows a cyan pointer and trail. `scripts/run-mass-lab` refreshes port-access
groups through the existing launcher. The desktop entry also uses this launcher.
Reads run on a worker thread so the UI stays responsive; disconnect/error releases
the port, and samples older than 500 ms are marked stale. The simulation-only and
graphics-smoke modes do not connect to hardware.

The pen position is the reference-model Cartesian position **relative to the
registered reset pose**, not a physically calibrated tool-tip pose. Hold the arm at
its reset pose (arm level, square to the base) and press **C** or **Zero at reset
pose**; until then the pointer does not drive the scene and feedback is disabled.
Registration makes that pose the model origin, which maps to the scene point
`(0, -0.3, 0)` beneath the floor, at 40 scene metres per model metre by default.
Sensitivity, height/depth swapping and axis inversion are under **Motion mapping**.
An explicit message indicates when the visual pointer reaches scene limits.

The cursor is a 0.2-metre-radius simulated sphere. Slide beneath a block and lift
to push it upward by collision. Hold the physical pen button near a block to grab
it; release to drop. The free block can rotate around the attachment point; other grips constrain Y only.
A screen-space ring keeps the pointer visible through the floor and blocks.
The target moves at up to 8 scene metres/second. A dynamic 0.2 kg proxy follows
through a stabilized spring/damper; the sphere shows its actual contact position. Pause freezes physics; stale input/disconnect releases grabs.
The button is decoded from three observed physical press/release cycles (P-0026).
Enable **pen force feedback** in the sidebar once the pose is registered. Feedback
turns off whenever you register again.

The force is a **model command, not a measured force**. The UI sends a coupling
descriptor (proxy position and velocity in registered model metres, stiffness and
damping) every frame. The device worker evaluates `F = K (proxy - p) - B (v - v_proxy)`
on **every servo tick** from the sample it has just read, with `K` a device stiffness
in model N/m (default 150, slider 20 to 1500), never the scene's 600 N/unit spring.
A descriptor older than 100 ms is ignored, so a stalled UI cannot tether the pen to a
frozen proxy. The SDK then caps the vector at the default 0.5 N (`DEFAULT_FORCE_N`, ADR-0002), limits
each channel to 96 DAC counts by scaling the whole vector, and slews at up to
100 N/s. A 2 kg block hanging from the proxy lags it by about 3 mm, so the pen is
commanded about 0.45 N at the default stiffness. Next to "Pen force" the sidebar shows why the
force is zero when it is: feedback off, unregistered, paused, time scale not 1.00x
(within 0.5%; the slider snaps to 1.00x), or scene stalled, plus cap, slew and
expiry badges and a count of muted servo ticks. The worker samples on every loop
iteration and drains UI messages without blocking, so UI traffic cannot starve input.

The selected-block panel shows linear/angular momentum, and **Spin lightest block**
applies a 0.1 N·m·s angular impulse. All six input encoders, including wrist/pen
channels and their relative counts, are visible in the sidebar. Optional raw CSV
capture is enabled with `--capture-input=PATH` (truncates that path at startup).

**Known issue (2026-10-16, P-0033):** the owner reported that motors did not stop pen
motion at a block surface. Delivered force has never been measured, and a 0.45 N
block weight cannot stop a hand. Judge delivery with a fingertip or a hands-off
test, then with the Calibration Studio's gauge protocol (`docs/spec/14-force-calibration.md`).

Only main-arm motor-command transport has been verified. The owner's possible two smaller wrist
actuators remain unidentified; direct torque requests are rejected by the SDK.
The configured reference reports six inputs and three outputs, which does not
prove the physical end-effector configuration. Actual force magnitudes have not
been measured. Hardware watchdog behavior and hard real-time timing remain open.
Orderly exit/disconnect/SIGINT/SIGTERM attempt zero/shutdown and join the worker;
software cannot guarantee cleanup after SIGKILL or a kernel stall.
The app holds the port while connected; use Disconnect before running other
hardware tools. The protocol and mapping are documented in
[the relative pointer specification](../../docs/spec/11-relative-pointer.md).

## Verify

```sh
cargo test -p mass-lab
cargo clippy -p mass-lab --all-targets -- -D warnings
xvfb-run -a -s '-screen 0 1280x820x24' \
  env LIBGL_ALWAYS_SOFTWARE=1 target/debug/mass-lab --smoke-test   # needs Xvfb and GL libraries
```

The app tests cover gravity across masses, inverse-mass impulse response, mass edits,
floor collisions, removal and population limits, ray picking, pause behavior,
frame-rate independence, reset behavior, scene/device round trip, coupling expiry,
mute reasons, worker sampling under message floods, the 2 kg block force, stale-data
detection, the owner-requested axis mapping, contact lifting across masses, grab/release,
and loss-of-input/deletion cleanup. The HAL and SDK tests cover button decoding, readout, the transport clamp, registration,
rollover, guard limits, slew, DAC scaling and expiry.
Additional app checks cover weight reactions, inertial reaction, both momenta,
and the free/vertical constraint selection. The graphics smoke test renders and
steps all three presets, edits mass, and spawns/removes a body before exiting.
Set `MASS_LAB_SCREENSHOT=/absolute/path.png` to save the third rendered frame.

Implementation references: [Rapier world and stepping](https://rapier.rs/docs/user_guides/rust/simulation_structures/),
[collider mass properties](https://rapier.rs/docs/user_guides/rust/collider_mass_properties/),
and [egui/Macroquad integration](https://docs.rs/egui-macroquad/0.17.3/egui_macroquad/).
