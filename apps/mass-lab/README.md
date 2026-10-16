# Mass Lab

A native Rust desktop app for exploring rigid blocks under gravity. Macroquad
renders the 3D scene, egui supplies the controls, and Rapier simulates collisions,
friction, restitution, rotation and mass-dependent impulses. The minimum-mass
block is free to move and spin; all others move only vertically. Ties select the
lowest block ID. Adding/removing blocks or editing masses updates this choice.

## Run

From the repository root:

```sh
tools/run-mass-lab
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
| Recenter the device pointer | C or Recenter |
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

The app now connects to `/dev/parport0` through the existing Rust `DeviceReader`
and shows a cyan pointer and trail. `tools/run-mass-lab` refreshes port-access
groups through the existing launcher. The desktop entry also uses this launcher.
Reads run on a worker thread so the UI stays responsive; disconnect/error releases
the port, and samples older than 500 ms are marked stale. The simulation-only and
graphics-smoke modes do not connect to hardware.

This is **relative reference-model motion**, not a physically calibrated tool-tip pose.
The device SDK now computes reference-model Cartesian positions from the first
three arm counters. Position and force share those axes; the default scale is
40 scene metres per model metre. Physical calibration is still unfinished. The starting physical pose is the bottom of vertical
travel and the centre of the other axes. The software origin is `(0, -0.3, 0)`,
beneath the floor, allowing entry under resting blocks. Hold that pose and press **C** to establish the reference.
Sensitivity, height/depth swapping and axis inversion are under **Motion mapping**.
An explicit message indicates when the visual pointer reaches scene limits.

The cursor is a 0.2-metre-radius simulated sphere. Slide beneath a block and lift
to push it upward by collision. Hold the physical pen button near a block to grab
it; release to drop. The free block can rotate around the attachment point; other grips constrain Y only.
A screen-space ring keeps the pointer visible through the floor and blocks.
The target moves at up to 8 scene metres/second. A dynamic 0.2 kg proxy follows
through a stabilized spring/damper; the sphere shows its actual contact position. Pause freezes physics; stale input/disconnect releases grabs.
The button is decoded from three observed physical press/release cycles (P-0026).
Enable **pen force feedback** in the sidebar, or launch with `--force-feedback`.
The app sends the equal/opposite spring-coupling reaction through `haptics-device`:
weight, acceleration/deceleration, contact and off-centre rotational effects.
The force is averaged across physics steps, scaled to 2%, then limited by the SDK
to 0.5 N, 2 N/s slew and 96 DAC counts. Pressing into a block increases the
computed opposing force until the limit; physical resistance is not validated
(see the known issue below). This does not command 100% motor output or create
an infinitely rigid surface. A zero request bypasses the slew limit. Pause or non-1× simulation
speed sends zero. The independent worker zeros commands older than 25 ms.

The selected-block panel shows linear/angular momentum, and **Spin lightest block**
applies a 0.1 N·m·s angular impulse. All six input encoders, including wrist/pen
channels and their relative counts, are visible in the sidebar. Optional raw CSV
capture is enabled with `--capture-input=PATH` (truncates that path at startup).

**Known issue (2026-10-16, P-0033):** the owner reports that motors do not stop
pen motion when it reaches a block surface. Effective surface resistance remains
unresolved. The 2% scale and 0.5 N cap may contribute, but the cause has not been
established; packet acknowledgements and simulation tests do not prove physical
force delivery.

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
  env LIBGL_ALWAYS_SOFTWARE=1 target/debug/mass-lab --smoke-test
```

Nineteen app tests cover gravity across masses, inverse-mass impulse response, mass edits,
floor collisions, removal and population limits, ray picking, pause behavior,
frame-rate independence, reset behavior, encoder rollover, recentering, stale-data
detection, the owner-requested axis mapping, contact lifting across masses, grab/release,
and loss-of-input/deletion cleanup. Six HAL tests verify button decoding and readout; three SDK tests verify all-six
rollovers, reference model/force fixtures, arming, clamps and stale requests.
Additional app checks cover weight reactions, inertial reaction, both momenta,
and the free/vertical constraint selection. The graphics smoke test renders and
steps all three presets, edits mass, and spawns/removes a body before exiting.
Set `MASS_LAB_SCREENSHOT=/absolute/path.png` to save the third rendered frame.

Implementation references: [Rapier world and stepping](https://rapier.rs/docs/user_guides/rust/simulation_structures/),
[collider mass properties](https://rapier.rs/docs/user_guides/rust/collider_mass_properties/),
and [egui/Macroquad integration](https://docs.rs/egui-macroquad/0.17.3/egui_macroquad/).
