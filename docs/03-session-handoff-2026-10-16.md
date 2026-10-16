# Session handoff — 2026-10-16

## Latest continuation — acceptance audit

The documentation turn was progress. The requirement-by-requirement audit is in
`docs/calibration-acceptance.md`. Current checks pass: 60 Rust unit tests, three
doctests, five viewer tests, formatting and six audio tests. The full goal remains
unproven because delivered force, resistance and physical release have not been
validated; the earlier weak-contact report remains unresolved. Exact legacy demo
pack recovery and unit compatibility remain partial research results, not claims
of completed historical coverage. Hardware remains off. The next end-to-end
acceptance step requires an attended physical session; no new live action is
started by automatic goal continuation.

## Earlier continuation — public crate documentation audit

The extrema fix was progress and is pushed. This offline continuation audited
public Rust documentation with `missing_docs`, identifying and documenting 67
previously uncovered API items across HAL, device, effects and workspace crates.
Each crate now enables the missing-docs warning. Documentation distinguishes
model units, retained observations, command telemetry, actual transport I/O,
and cleanup responsibilities. In particular, HAL sampling marks an error but
its caller must initiate shutdown; the device SDK does this automatically.
Strict rustdoc generation and all three existing doctests pass. No device opened.
Physical force response, thermal limits and overload release remain unverified;
API documentation does not close those outstanding requirements.

## Earlier continuation — exact sweep extrema

The prior turn delivered and pushed live route teaching; it was progress. This
continuation found and fixed loss of observed travel inside an already retained
2 mm display cell. Sweep extrema now accumulate independently in constant time,
are exported as `extrema_model_m`, and invalidate prior path review when changed.
No enclosing-box corner is inserted into sample support. The point cap remains
20,000; invalid observations cannot alter bounds. Calibration tests (17) and
Clippy pass, including regression checks for thinning, export and the storage cap.
Hardware remains off; no physical validation is claimed.

## Earlier continuation — live route teaching, offline verified

This section supersedes the initial sweep force-lock notes below. Hardware remains
powered off; no device was opened during this continuation. Owner authorized
completion, commits and push to main.

- Native sweep calibration now offers motor-off out-and-back approach teaching.
  Continuous input, slow speed, a narrow straight corridor and two still dwells
  are required. Delays, Stop, pen button, UI loss and calibration changes cancel it.
- Only the worker's fresh completed trace can authorize swept-map pilot attempts;
  neither exported JSON nor the geometry review can restore authorization.
  Confirmation expires after five minutes. Zero moves require the taught start;
  all held effects require sample support and a 0.1 model N ceiling. Active
  attempts stop on expiry or leaving support, subject to documented I/O latency.
- Trace/policy evidence accompanies exports and attempt metadata. Object radius
  is limited to 20 model mm in this workflow. All seven original demos remain.
- Verified: 58 workspace Rust unit tests and three doc tests; Clippy all targets;
  native build and both offline graphics smoke modes. Neither graphics mode
  creates a device worker. Teaching/expiry rejection tests use synthetic data.
- Next attended work: capture a fresh bottom reference, sweep, choose zero,
  teach a comfortable short route, then assess low-force response. Physical
  force calibration, resistance/overload release, thermal behaviour and precise
  arm geometry remain unresolved; tracing does not validate those quantities.
- Offline launch: `tools/run-calibration --demo`; recorded 3D reconstruction:
  `tools/run-workspace-viewer`. See app READMEs for controls and evidence limits.

## Earlier continuation — sweep evidence and offline reconstruction

This section supersedes earlier runtime/force-session notes below. The owner
powered the unit off after an explicitly requested input-only sweep recording.
The recording stopped cleanly: 31,271 packets in 74.109 s; no force session.
The final resting pose is only approximately bottom-centre on the desk. Evidence
and measured stability are in `docs/evidence/input-only-sweep-20261016/`.

- `crates/workspace` now provides analytic whole-segment sample-support review,
  with native gold-path/red-gap display and exported review evidence. The archived
  sweep's first-to-last straight segment has an unsupported interval around
  1.328%–70.609%; this is missing evidence, not an inferred obstacle. Geometry
  review alone does not enable sweep motor output.
- Calibration Studio now defaults to motor-off sweep acquisition, observed-cloud
  display, manually chosen zero and evidence export. Swept maps do not grant
  force authority. Advanced six-bound box calibration remains available but does
  not prove its corners reachable. Region/path validation needs further work.
- `apps/workspace-viewer` replays the included owner sweep in Three.js with an
  adjustable photo-informed armature, observed-cell surface, optional convex outer
  bound and GLB export. `npm ci && npm start` there serves localhost:8765.
- The uploaded root ZIP was preserved via fast-forward to the GitHub upload.
  Original extracted images remain local/ignored; the root owner archive is tracked.
- Main-arm link defaults are from the existing reference model. No precise
  dimensions, wrist torque assignment, thermal trip or safe usable volume was
  inferred from photographs. Physical resistance testing remains outstanding.
- Verification includes workspace unit/doc tests, audio tests, viewer unit and
  Chromium tests, Clippy and both native graphics smoke modes. Test details are
  in the application READMEs. No live tests after the owner powered down.


## Latest continuation — Calibration Studio

The older running-app notes below are historical. On the latest check no device
application held `/dev/parport0`; no powered tests were run after the input-only
baseline. The owner resumed work after switching to tmux.

- `apps/calibration` is the native guided calibration/demo app. Launch offline
  with `tools/run-calibration --demo`; normal launch connects input-only.
- Capture bottom and six comfortable bounds, enter working coordinates, export
  calibration evidence with output off, then hold for a slow zero move or one of
  seven canonical scenes. Fresh captures are required on every connection.
- `crates/effects` supplies independently written effects with inline docs.
- SDK review fixes share continuous absolute encoder history between pose and
  force calculations and classify failed shutdown as an I/O fault requiring
  reconnection. Command-limit release is an independent software policy.
- Attempts save demand, guarded output, exact motor offsets, timing, command-limit
  status and mapping evidence. Manual sound/surface-temperature observations and
  external WAV paths can be attached. `tools/analyze-motor-audio` performs offline
  descriptive spectral comparisons; it does not diagnose safe motor loading.
- Workspace tests passed (44 unit tests after the additional rollover regression,
  plus two doc tests); audio checks passed (6); Clippy and native graphics smoke
  passed. Later edits should rerun the checks appropriate to those changes.
- Physical force, operator resistance, measured temperature and overload-release
  behaviour remain unvalidated on this aged unit. Keep the existing conservative
  ceilings; do not raise them based on audio or historical product ratings alone.
- D-0001 reference driver investigation is complete. D-0002 old Dice send-wrapper
  evidence confirms disable on nonzero force-send error. Bounded newer example
  review is complete (P-0041): recovery policies differ; no universal physical
  overload threshold established. Vendor artifacts and detailed Evidence remain ignored.


## Current continuation — SDK and physical feedback

**Unresolved physical feedback issue (owner report, P-0033):** “the motors don't
stop me moving when i get to the surface of the block.” Surface contact is not
providing the expected physical resistance. Numerical collision/force tests and
acknowledged motor packets do not establish effective resistance at the pen.
The current 2% force scale, 0.5 N cap, calibration, output mapping and actuator
response need investigation; the cause has not been established. Do not mark
physical surface feedback as working or raise limits solely from packet success.


- `crates/device` is the reusable `haptics-device` SDK: all six counters,
  reference Cartesian model/Jacobian, Wrench API, force guards and device lifecycle.
  HAL handles observed motor packets. No vendor runtime dependencies.
- Native Rust motor-command transport is acknowledged. Zero and -0.1 N bounded
  checks each completed 168 acknowledged samples and zero/shutdown; logs under
  ignored `vendor-original/live-state/native-force-*.log`. These are model command
  values, not measured physical forces. P-0028–P-0032 record findings.
- Actual bit-6 acknowledgement clears on duplicate reads. Start leaves a fresh
  zero packet pending; then read/ack/write each cycle. Initial zero failures were
  rejected before any nonzero check. Force output is explicitly armed and bounded.
- App uses a dynamic pen proxy with stabilized spring/damper coupling. It sends
  reaction force at 2% scale to SDK limits (0.5 N, 2 N/s slew, 96 DAC-count offset).
  Stale commands >25 ms produce immediate zero. Pause/non-1× speed sends zero.
  Native loop runs roughly 400 Hz here; no hard real-time/hardware-watchdog claim.
- The lightest block is free in translation/rotation; others move vertically only.
  Linear/angular momenta are shown, angular impulse control added; release retains
  both velocities. Tests verify supported weight, inertial force and constraints.
- Pointer now uses relative reference-model Cartesian positions, default scene
  scale 40×, no inversion/swap. Start bottom/centre and press C. The old direct
  encoder mapping is superseded so forces and positions share consistent axes.
- All six encoders are visible and captured. Owner reports rotary pen encoder and
  possibly 3 main + 2 small motors. Wrist capture request remains pending: three
  clockwise/back rotations with tip held steady and a stated viewing direction.
  `pen-rotation-01.csv` is preserved; `pen-rotation-02.csv` is logging in current app.
  Last three axes are not yet assigned calibrated angles or physical actuator roles.
- Configured reference reports 6 inputs/3 outputs; wrist-torque requests produce
  no extra commands. This does not identify actual hardware; SDK rejects nonzero
  torque until that protocol is verified. Manufacturer has distinct force-only
  and force/torque variants. Do not conclude that the owner's small units are passive.
- App running as transient user `mass-lab.service`, using `tools/with-port-access`,
  `--force-feedback` and capture path `pen-rotation-02.csv`. Force feedback enabled.
  Signal flags + joined worker handle graceful shutdown; SIGKILL is not covered.
  Workspace 1 currently has Foot on top, app and an Omarchy agent window below.
- 19 app, 6 HAL and 3 SDK tests pass; workspace Clippy and Xvfb graphics smoke pass.
  See `docs/spec/12-force-feedback.md` and
  `crates/device/README.md` for the SDK and limitations.

## Earlier milestone — live connection verified (superseded by continuation above)

The original objective is achieved: drivers downloaded/examined and an independent
Rust connection verified against the actual unit. Owner confirmed continuously
powered, present, and explicitly authorized tests with motors powered. This
supersedes the physical-isolation blocker in the historical entries below.

- Native EPP read stalls; actual diagnostic proves control changes 0x00 to 0x04.
  Linux's ECP/EPP callback sets INIT. Software EPP preserving the reference's low
  INIT yields successful initialization, force disable, encoder query and close.
- Own Rust `haptics_hal::device::DeviceReader` implements a bounded observed
  initialization prefix and 16-byte acquisition without vendor libraries or
  force-output packets. Six signed counters match SDK axis order.
- `tools/with-port-access timeout -s KILL 10s target/debug/device-readout /dev/parport0 50`
  prints CSV. Build with `cargo build -p device-readout` first.
- Verified five packets under strace and 50 on an independent fresh connection;
  port owner is now none. No capture jobs are running. Five focused unit tests
  and workspace clippy pass. See `tools/device-readout/README.md`.
- `docs/spec/10-device-readout.md` preceded Rust implementation; P-0018 onward
  documents owner authorization, real captures, kernel behavior and validation.
- Ignored `vendor-original/live-state/` holds raw traces/CSV; ignored reference
  harness and software-EPP shim are preserved for reproduction. Initial timeout
  capture is incomplete; later successful runs are separately named. No vendor
  installer or library installed globally; no decompilation performed.
- Calibration, force output, watchdog/interlock, packet tail interpretation and
  real-time performance remain future work. User authorization for this capture
  does not establish any of those engineering properties.

## Historical notes (superseded where they conflict with the outcome above)

## Original objective and initial state

Owner wants to revive the parallel-port device with a Rust/Linux stack and suggested obtaining the Windows vendor drivers. We agreed to obtain both Linux and Windows reference packages, preferring observation of the Linux stack if compatible. No driver packages have been downloaded, no tools installed, no protocol code written, no vendor code disassembled, and no hardware transactions performed in this session. No force output or amplifier-enable command was issued. Device power state was not confirmed: the owner was asked to keep motor power off.

## Hardware and host facts

- Owner reads the label as **PHANToM 1.0A, part 02278, RoHS**. Serial/date and PSU ratings remain unknown. Do not infer a different model from online pictures.
- Owner explicitly confirms all one unit, no separate interface box, and a direct DB25 cable from the PC parallel card into the unit. Internal electronics have not been inspected. See P-0002/P-0003 and `hardware/identification.md`.
- Host is **Omarchy 4.0.4, Arch-based**, not the CachyOS presumed by the original research. Do not assume a CachyOS RT kernel is installed.
- Card: **Oxford Semiconductor OXPCIe952**, PCI `02:00.0`, ID `1415:c110`.
- `parport_pc`, `parport`, `ppdev` loaded. `parport0` reports `PCSPP,TRISTATE,EPP,ECP`, base addresses `53264 53248`, IRQ `18`. This advertises EPP; successful EPP transactions remain untested.
- `/dev/parport0` exists on the actual host: character device `99:0`, permissions `0660`, owner `root:lp`. Initial sandbox checks incorrectly suggested it was missing; an unsandboxed read corrected this. Do not create another node.
- Owner ran `sudo usermod -aG lp john`. Account lookup now shows `lp:x:989:cups,john` and `id john` includes lp. Existing agent process `id` still showed only john/wheel and `os.access` returned false for read/write. Owner was told to fully log out/in or reboot, then reopen the app. Verify in the new session.

## Repositories and tools

- `/home/john/Work/haptics-rust`: research-only starting point; retain clean-room policy, provenance, and spec-before-code discipline.
- **Reverse Engineer Anything (REA) is already downloaded/cloned at `/home/john/Work/rea`.** Correct upstream is https://github.com/morluto/rea (not morlutu). It is a sibling of this repository; do not download another copy. Cloned main; no setup or dependency installation performed. Read `/home/john/Work/rea/AGENTS.md` before changes or setup.
- GitHub CLI login completed as **jjohare**; both repositories were cloned successfully. No credentials belong in these notes.
- Node/npm found on PATH under mise Node 26.8.2. `java`, `ghidra`, `strace`, and `qemu-system-i386` were not found on PATH, and package queries did not find the checked packages. Recheck before installing.
- REA can orchestrate native analysis via Ghidra/Hopper/IDA and recorded signal analysis via sigrok. It does not supply a parallel-port driver or capture the wire by itself. Its `native/windows` addon concerns running REA on Windows; it is not required simply because an analysed file is a Windows executable on Linux.

## Driver acquisition findings and limitations

Official starting point: https://www.3dsystems.com/haptics-devices/openhaptics

The current vendor page lists **Phantom Device Driver 5.1.7 or above** for parallel-only Premium 1.5 variants on both Windows and Linux. This is a useful package family to investigate, not confirmed compatibility for the owner's 1.0A. We have not established that Win98/NT4 is necessary.

Official links from that page:

- Windows driver downloads: https://support.3dsystems.com/s/article/Haptic-Device-Drivers
- Linux/OpenHaptics download page: https://support.3dsystems.com/s/article/OpenHaptics-for-Linux-Developer-Edition-v34

The web reader saw only a loading/CSS-error shell on these support pages. Actual package names, direct download URLs, versions, access/registration requirements and licence acceptance remain unverified. A normal browser may work. No reason yet to conclude the drivers are unavailable. Keep packages in ignored `vendor-original/`; record original URL, download date, version, size and SHA-256. Do not commit vendor installers, SDK headers, manuals, firmware or decompiler output.

## Prior work that changes the earlier research

https://github.com/forsslund/vintagehaptics reports a working reverse-engineered userspace parallel driver for Desktop and Premium 6DOF devices, including operation through PCIe parallel cards. At inspection its public repository contained documentation/images, with driver code described as private pending removal of vendor reference material. This is an author claim, not something we tested.

The author also describes some 1.0/1.0A hardware with DB15/DIN and a separate box; that does **not** override the owner's explicit direct-DB25 description. Resolve exact hardware compatibility before reuse.

The project reports Windows-driver disassembly as part of its research. Treat any later protocol details from it with explicit source/method provenance; public availability alone does not establish observation-only origin. No protocol details or vendor disassembly from that project were imported here. The earlier blanket statement that no public parallel-protocol reverse-engineering effort exists is outdated. No messages or issues were sent to the author.

## Corrections and open engineering questions

- Linux userspace/ppdev capture is promising, but actual vendor library use of ppdev versus direct I/O must be established from the acquired binaries' behaviour. Earlier symlink-based reasoning does not prove this conclusively.
- A syscall trace is not automatically a complete register timeline: capture full ioctl and read/write buffers, distinguish EPP address/data modes, check truncation and other I/O paths, and account for timing perturbation.
- QEMU parallel passthrough, complete EPP forwarding and relevant trace-event coverage must be verified against the chosen build/source. Earlier notes state these more confidently than this session established. A successful SPP connection alone is insufficient.
- Do not describe EPP negotiation as passive or as guaranteed to leave data/control signals unchanged. The proposed port-probe in the older handoff includes negotiation; separate capability queries from deliberate mode changes.
- No observed protocol facts exist yet. The mainline PCI PHANTOM driver is a different interface; sibling register layouts are hypotheses, not this unit's specification.

## Next actions, in order

1. Verify new-session group membership and `/dev/parport0` permissions using `id`, `id john`, `ls -l`, and permission queries. Read host device information outside a restrictive sandbox if needed. No blanket sudo access is needed.
2. Obtain compatible Linux and Windows PDD packages and configuration/test utilities from the vendor; inventory and hash them without executing installers. Establish 1.0A support and required OS/runtime first.
3. Prepare minimal capture tooling: strace and archive inspection first; QEMU only if needed. Use Omarchy skill for system configuration changes. Install only what the selected route needs.
4. Build a trace decoder/replay foundation and a query-only ppdev capability tool. Follow repository spec/provenance rules. Do not negotiate or probe arbitrary values on the connected device as part of a supposedly passive check.
5. Plan the first deliberate capture with motor power disabled and an understood interlock: initialization, presence/identification, shutdown, then encoder-only acquisition. Confirm how motor power can be isolated before executing vendor utilities that may enable forces automatically.
6. Write protocol findings in our own words with provenance before Rust protocol implementation. Watchdog, force clamp and amplifier interlock precede any force output.
7. Use REA/Ghidra only for a named unresolved interoperability question after following the existing decompilation-log policy. No need to install the whole RE tool suite in advance.

## Workspace handoff status

Notes are local changes; no commit or push was requested or performed. Review `git diff` before committing. No tests were run because this session only inspected hardware metadata and edited documentation.

## Pickup check — later on 2026-10-16

- Re-read handoff, README, verdict and clean-room policy; preserved existing local changes.
- Host check outside sandbox still reports current process groups `john,wheel`, while account lookup includes `lp`. `/dev/parport0` remains `root:lp`, mode 0660, character device 99:0; current process has neither read nor write access. Owner reports logging out/in. Advised fully quitting/reopening the app; if its parent still retains old groups, reboot. Do not repeat usermod or relax node permissions.
- Vendor support pages still return a JavaScript shell to HTTP/web readers. Download URLs remain unresolved; no packages obtained or executed.
- Additional download lead: https://www.ddd.co.jp/support/haptics-driver/ links Windows PDD 5.1.9 at https://www.ddd.co.jp/downloads/phantom/Phantom_Device_Drivers_5.1.9_RELEASE.exe ; web fetch returned 404. Treat as a broken lead, not an acquired package or proof of 1.0A compatibility.
- `rustc`, `cargo`, and `strace` were not on PATH in either sandbox or host check. Archive tools `bsdtar` and `unzip` are present. No tools installed, no hardware opened or queried through ioctl, no protocol code written.

## Pickup check — current workspace session

- Rechecked process/account groups and access without opening the device: process still has `john,wheel`, account includes `lp`, and `/dev/parport0` remains character device 99:0, `root:lp`, 0660. Read/write permission checks both return false.
- Host metadata still advertises `PCSPP,TRISTATE,EPP,ECP`, bases `53264 53248`, IRQ 18. No ioctl, negotiation, or hardware transfer performed.
- `rustc`, `cargo`, and `strace` remain absent from PATH; `bsdtar` and `unzip` are available.
- Rechecked the official Windows/Linux support links with the web reader; both still expose only a support-page shell. No packages acquired or executed.
- Corrected README and coding handoff to reflect the actual host, known prior work, unverified capture routes, and non-passive nature of negotiation. Existing local edits preserved. No commit or push performed.

## Port access resolved for agent-launched tools

- Owner explicitly authorized passwordless sudo to fix access. `sudo -n -u john -- COMMAND` starts commands as UID 1000 with current supplementary groups, including `lp`; read/write permission checks on `/dev/parport0` pass.
- Added executable `tools/with-port-access COMMAND [ARG ...]`: executes directly when access is available, otherwise re-executes through passwordless sudo as the invoking ordinary user to refresh groups. Use this launcher for port tooling from the stale app session. Ordinary commands in the original app process still retain its old groups.
- Node ownership/mode remain `root:lp` 0660. No group, udev, or global permission changes were needed. No reboot is required to use the launcher.
- Verified launcher syntax, effective UID/groups and read/write permissions without opening the device or issuing ioctls/transfers. This verifies filesystem access, not EPP operation.

## Driver acquisition and Rust work — current goal turn

Active objective: download/examine drivers and establish a verified connection to
the actual unit in Rust. **Not complete**: no successful unit communication yet.

- See `research/driver-examination.md`, `research/driver-downloads.json`, and
  provenance P-0007–P-0013 for acquisition, offline execution and verification.
- Installed Rust 1.98.1, strace 7.0, 7zip, openmotif, libxp, libxpm, Xvfb,
  xdotool and xwininfo through Omarchy. Ubuntu compatibility packages only
  extracted into ignored vendor-original/runtime; no vendor installers run.
- Added a dependency-free Rust workspace with haptics-hal and port-probe. Host
  query succeeds as John with 0x4f capabilities. Strace proves no claim or
  negotiation. This is not peripheral communication. No automated unit tests yet.
- Legacy SDK/library load and configuration generation were verified in an
  offline bubblewrap sandbox. Mock-port trace shows ppdev claim/mode attempts;
  all fail ENOTTY as expected. No hardware exposed to vendor code. The GUI sandbox
  and its virtual display have been stopped; no pending downloads/processes.
- `tools/vendor-sandbox` recreates the offline boundary; tested for absence of
  host port nodes. Existing ignored harness is vendor-original/blackbox-init.py.
- Owner was asked how power is supplied and whether amplifier power can be
  isolated from interface power; reply pending. No live initialization until this
  is understood. Offline config has unverified encoder defaults and a legacy
  0x378 address; do not use it blindly with the actual Oxford card.
- Next: review configuration, prepare full-buffer capture/decoding, acquire real
  initialization/encoder evidence with interlock, then spec and Rust protocol.
- Existing local changes preserved. No commit/push performed.

## Goal continuation — capture instrumentation

Previous goal turn classified as progress (drivers acquired, offline runtime and
host Rust query verified). This turn also makes progress; the objective remains
active and actual unit communication remains unproven.

- Added tools/port-recorder: Rust snapshot/JSON implementation with a small C and
  x86-64 assembly preload shim, plus tools/build-recorder and integration checks.
  Specification was written first in docs/spec/01-call-capture.md; P-0014–P-0016
  record provenance. Dependencies are pinned by Cargo.lock.
- Tests pass for unmodified results/errno, binary read/write buffers, ioctl input
  values, invalid pointers, truncation, thread separation, reused output prefixes,
  and log failures. cargo clippy --workspace --all-targets -- -D warnings passes.
- Repeated the host query to validate recorder output-side capture: PPGETMODES
  returns bytes 4f000000. No claim/negotiation on real hardware.
- Mock reference run and strace agree on three rejected ppdev ioctls; recorder
  captures the compatibility cleanup mode 0x100 that strace leaves as a pointer.
- Explicitly synthetic host acceptance experiments expose the next EPP negotiation
  attempt and subsequent control/address/data operations. Still no real peripheral
  replies; no device protocol was implemented from these simulated traces. Some
  reference control failures do not immediately abort, so partial failure traces
  must not be promoted to a successful connection recipe.
- Reproducer scripts, copied recorder .so, own mock shims, and logs are ignored
  beneath vendor-original/. All sandbox runs have exited. No live processes wait.
- Power arrangement/interlock question is still unanswered. Actual initialization
  and encoder validation remain blocked on that physical information; offline
  capture preparation can continue. No repeat question was sent this turn.
- Read tools/port-recorder/README.md for build/test/use and coverage limitations.
  No commit or push performed; prior local edits preserved.

## Goal continuation — address check and blocked audit

- Previous goal turn classified as progress: tested recorder and reference-call
  evidence changed the next capture action. This turn revalidated files/state and
  resolved the remaining early address-acceptance question via P-0017.
- The actual PCIe base 0xd010 and legacy 0x378 configurations both reach the same
  EPP negotiation attempt in the offline model. No live negotiation was run.
- Verified all nine original/compatibility package hashes and sizes against the
  acquisition inventory. No background capture or GUI process remains live.
- Completion audit: driver download and examination are evidenced; Rust host
  access and recording work. Actual unit identification/encoder connection,
  observed peripheral specification, and working Rust device protocol are missing.
  The objective cannot be marked complete.
- The same missing power/interlock information has prevented live initialization
  across three consecutive goal turns (acquisition, recorder, address comparison).
  This is an external physical-state dependency, not a sudo/software-access issue.
  Required offline preparation is ready; further simulation cannot establish the
  requested actual connection. Marking goal blocked pending owner response.
- Resume with the owner's current power state and description of available power
  plugs/switches/separate motor cutoff. Do not infer isolation or power state from
  a host port capability query. Existing work and goal scope are preserved.
