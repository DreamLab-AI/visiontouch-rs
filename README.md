# haptics-rust

A private hardware-archaeology project: a clean-room reimplementation, in Rust on Linux (CachyOS), of the host-side protocol stack for a late-1990s **SensAble PHANTOM** haptic device that connects over an **IEEE-1284 EPP parallel cable**. The original stack ran on Windows 98/ME/NT and is fully out of support. The intended output is an academic paper, not a software release.

This repository is not affiliated with, endorsed by, or derived from code belonging to SensAble Technologies, Geomagic, or 3D Systems. Product names are used only to identify the hardware being studied.

## Status

**Research phase complete; verdict is to proceed.** See [docs/01-verdict.md](docs/01-verdict.md). Implementation is starting from [docs/02-handoff-to-coding-agents.md](docs/02-handoff-to-coding-agents.md). No protocol code exists yet; the first deliverables are the trace decoder, the ppdev HAL, and a port probe.

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
| [docs/clean-room/POLICY.md](docs/clean-room/POLICY.md) | The rules we work under |
| [docs/clean-room/PROVENANCE.md](docs/clean-room/PROVENANCE.md) | Where every fact came from (append-only) |
| [docs/hardware/identification.md](docs/hardware/identification.md) | Fill-in sheet for the physical device and host port |

## Key findings so far

- **The protocol is not copyrightable; the driver binaries are.** UK CDPA 1988 s50BA permits observing and testing a lawfully owned program; s296A voids licence terms to the contrary; *SAS v World Programming* (CJEU C-406/10, and [2013] EWCA Civ 1482) holds that functionality, interfaces and data formats are not protected expression. Decompilation (s50B) is allowed for interoperability but restricts onward communication of what it yields, which matters for a paper.
- **The rights holder is 3D Systems**, an active company, so this is not orphan IP. Nothing has ever been open-sourced by the owner. The foundational MIT patent (US 5,587,937) expired in 2013; UK private/experimental-use exemptions cover the rest.
- **The device is a plain EPP peripheral.** No interface card. SensAble's own Linux driver was a userspace binary talking to `/dev/parport0` via ppdev. That means the whole register-level dialogue can be captured with `strace`, or via QEMU port passthrough tracing for the Windows stack, before anyone reaches for a logic analyser or a disassembler.
- **Nobody has done this before.** No public reverse engineering of the parallel PHANTOM protocol was found. The mainline `drivers/misc/phantom.c` covers only the later PCI-card interface.
- **The Rust side is greenfield.** There are no `parport`/`ppdev` crates on crates.io. CachyOS ships `parport`, `parport_pc`, `ppdev` as modules; `linux-cachyos-rt-bore` plus CPU isolation and `SCHED_FIFO` is the right setup for a 1 kHz loop, all in userspace.

## Plan

1. **Identify** the exact model and record the host port details (`docs/hardware/identification.md`).
2. **Capture** the protocol by observation, Route 1 → 2 → 3 in `docs/research/capture-strategy.md`, logging every fact in `PROVENANCE.md`.
3. **Specify** the protocol in `docs/spec/` in our own words: enumeration, mode negotiation, amp enable, the servo cycle, homing, shutdown.
4. **Implement** the Rust workspace: `hal` (ppdev + ioperm + trace-replay backends), `protocol` (pure, tested against captures), `device` (state machine, watchdog, force clamp), `daemon` (RT loop, IPC).
5. **Validate** against captured traces, then against the real device with the stylus parked and the amplifier interlocked.
6. **Write it up.**

## Safety

This device applies force to a human hand. Nothing in this repository should be run with a hand in the gimbal until the stack has a working deadline watchdog and force clamp, and the amplifier-enable path is understood.
