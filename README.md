# haptics-rust

A private hardware-archaeology project: a clean-room reimplementation, in Rust on Linux (current host: Omarchy, Arch-based), of the host-side protocol stack for a late-1990s **SensAble PHANTOM** haptic device that connects over an **IEEE-1284 EPP parallel cable**. The original stack ran on Windows 98/ME/NT and is fully out of support. The intended output is an academic paper, not a software release.

This repository is not affiliated with, endorsed by, or derived from code belonging to SensAble Technologies, Geomagic, or 3D Systems. Product names are used only to identify the hardware being studied.

## Status

**Live Rust connection verified on the owner's 1.0A.** Windows and Linux reference drivers were downloaded and examined. The reference SDK initializes successfully using a software EPP transport that preserves control lines; the independent Rust reader receives real 16-byte packets and decodes six raw encoder counters. Two Rust runs received 55 complete packets and released the port. The reusable `haptics-device` SDK now provides bounded main-arm force output;
zero and 0.1 N command checks completed with acknowledgements and shutdown.
**Known issue:** the owner reports that motors do not stop pen motion at a block
surface. Effective physical contact resistance remains unresolved; packet
acknowledgements do not validate it. Physical calibration, wrist actuation and
hard real-time guarantees remain unverified. See [driver examination](docs/research/driver-examination.md) and the [observed readout specification](docs/spec/10-device-readout.md).

## Calibration and canonical demos

[Calibration Studio](apps/calibration/README.md) provides guided working-volume
sweep capture, an advanced six-bound held slow move to zero, and cube, water, balloon, three-body springs,
plane, viscosity and detent scenes. Run `tools/run-calibration --demo` for an
entirely offline preview, or `tools/run-calibration` for input-only connection
and guided calibration. Attempts record model demand, guarded commands and
operator sound/temperature observations. See the [overload investigation](docs/research/overload-and-calibration.md)
for evidence and remaining physical uncertainties.

## Workspace reconstruction

[Workspace Observatory](apps/workspace-viewer/README.md) is an offline Three.js
reconstruction of this unit with replay of the owner's recorded sweep. It shows
observed points, sampled-cell surfaces and a separately labelled convex bound;
none certifies a safe operating volume. Adjustable links and GLB export support
comparison with the owner's photographs. It never connects to the device.

```sh
cd apps/workspace-viewer
npm ci
npm start
# http://127.0.0.1:8765
```

## Desktop physics app

[Mass Lab](apps/mass-lab/README.md) is a native Rust 3D sandbox with blocks of
editable mass, gravity, collisions, scene presets, pause/step/reset and impulses.
The lightest block moves and spins freely; other blocks move only vertically.
A compliant pen proxy computes load and inertia reactions for the device SDK. Launch **Mass Lab**
from the desktop launcher or run:

```sh
tools/run-mass-lab
```

Place the stylus at the bottom of vertical travel and centred horizontally/in
depth, then press **C** to set the pointer reference. Motion mapping is adjustable;
Enable pen force feedback in the sidebar, or launch with `--force-feedback`.
The force cap is 0.5 N with a 2% scene-force scale. Physical calibration and direct
wrist torque remain unverified. Use `--no-device`
for simulation only. See the app README for controls and tests.

## Read the unit in Rust

```sh
cargo build -p device-readout
tools/with-port-access timeout -s KILL 10s target/debug/device-readout /dev/parport0 50
```

This performs active device setup and reads 50 samples at about 10 Hz, printing
CSV with six raw signed counters and the complete packet. It sends no force-output
packets and loads no vendor libraries. Use with the identified unit, a clear
workspace, and accessible power cutoff. See [readout instructions](tools/device-readout/README.md)
for scope and limitations.

## Run the Rust host query

```sh
cargo build --workspace
tools/with-port-access target/debug/port-probe /dev/parport0
```

This queries advertised host capabilities without claiming or negotiating. It does
not detect the unit or prove working EPP transactions. The current card reports
`0x4f`: PCSPP, TRISTATE, EPP, ECP, SAFEININT.

The [reference-call recorder](tools/port-recorder/README.md) captures ppdev ioctl
arguments and data buffers for observation. It has been checked against ordinary
files, the host capability query, and reference-driver captures on the actual unit.

## Latest session handoff

Read [docs/03-session-handoff-2026-10-16.md](docs/03-session-handoff-2026-10-16.md) first for the identified hardware, verified host state, corrections to earlier research, and immediate next steps. Earlier research includes assumptions that remain unverified.

## Read in this order

| Doc | What it is |
|---|---|
| [docs/00-charter.md](docs/00-charter.md) | Scope, goals, non-goals, release posture |
| [docs/01-verdict.md](docs/01-verdict.md) | **The decision: proceed, observation-first, and why** |
| [docs/02-handoff-to-coding-agents.md](docs/02-handoff-to-coding-agents.md) | Non-negotiables, first tasks, and layout for implementers |
| [docs/legal/POSITION.md](docs/legal/POSITION.md) | Our applied legal position (UK-based, private, paper as output) |
| [docs/legal/landscape.md](docs/legal/landscape.md) | The full UK / EU / US legal research with citations |
| [docs/research/device-phantom-parallel.md](docs/research/device-phantom-parallel.md) | What the device is, how it interfaces, what prior art exists |
| [docs/research/capture-strategy.md](docs/research/capture-strategy.md) | The three observation routes, in order of preference |
| [docs/research/linux-parport-rust.md](docs/research/linux-parport-rust.md) | ppdev, PCIe cards, real-time on CachyOS, proposed Rust crate layout |
| [docs/research/re-tooling.md](docs/research/re-tooling.md) | Ghidra, Binary Ninja, emulators, logic analysers |
| [docs/research/windows-binary-analysis-on-linux.md](docs/research/windows-binary-analysis-on-linux.md) | Linux-native toolchain for the Windows `.sys`/`.vxd`/DLL artefacts, if decompilation is ever needed |
| [docs/clean-room/POLICY.md](docs/clean-room/POLICY.md) | The rules we work under |
| [docs/clean-room/PROVENANCE.md](docs/clean-room/PROVENANCE.md) | Where every fact came from (append-only) |
| [docs/hardware/identification.md](docs/hardware/identification.md) | Fill-in sheet for the physical device and host port |

## Key findings so far

- **The protocol is not copyrightable; the driver binaries are.** UK CDPA 1988 s50BA permits observing and testing a lawfully owned program; s296A voids licence terms to the contrary; *SAS v World Programming* (CJEU C-406/10, and [2013] EWCA Civ 1482) holds that functionality, interfaces and data formats are not protected expression. Decompilation (s50B) is allowed for interoperability but restricts onward communication of what it yields, which matters for a paper.
- **The rights holder is 3D Systems**, an active company, so this is not orphan IP. Nothing has ever been open-sourced by the owner. The foundational MIT patent (US 5,587,937) expired in 2013; UK private/experimental-use exemptions cover the rest.
- **The identified unit is a PHANToM 1.0A with a direct DB25 connection.** The independent Rust implementation now exchanges packets using software EPP through ppdev; physical force calibration remains unverified.
- **Prior parallel-interface work exists.** [vintagehaptics](https://github.com/forsslund/vintagehaptics) reports a working userspace driver; compatibility with this unit and the provenance of any reused protocol facts require review. The mainline `drivers/misc/phantom.c` covers a different PCI interface.
- **The Rust workspace is implemented.** It includes transport, device SDK, capture/readout tools and the desktop simulator. Real-time kernel and scheduling choices still require measured requirements on the actual host.

## Plan

1. **Identify** the exact model and record the host port details (`docs/hardware/identification.md`).
2. **Capture** the protocol by observation, Route 1 → 2 → 3 in `docs/research/capture-strategy.md`, logging every fact in `PROVENANCE.md`.
3. **Specify** the protocol in `docs/spec/` in our own words: enumeration, mode negotiation, amp enable, the servo cycle, homing, shutdown.
4. **Implement** the Rust workspace: `hal` (ppdev + ioperm + trace-replay backends), `protocol` (pure, tested against captures), `device` (state machine, watchdog, force clamp), `daemon` (RT loop, IPC).
5. **Validate** against captured traces, then against the real device with the stylus parked and the amplifier interlocked.
6. **Write it up.**

## Safety

This device applies force to a human hand. Nothing in this repository should be run with a hand in the gimbal until the stack has a working deadline watchdog and force clamp, and the amplifier-enable path is understood.


## Rust API documentation

The reusable `haptics-hal`, `haptics-device`, `haptics-effects` and
`haptics-workspace` crates document their public APIs inline, including coordinate
units, software versus physical limits, and transport cleanup responsibilities.
Missing public documentation emits a compiler warning.

```sh
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps
cargo test -p haptics-device -p haptics-effects -p haptics-workspace --doc
```

Open `target/doc/haptics_device/index.html` for the device lifecycle and links to
its types. HAL calls perform real I/O; the effects and workspace geometry crates
are hardware-free. Documentation examples do not connect to a device.
