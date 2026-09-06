# Handoff brief for coding agents

Read `README.md`, `docs/01-verdict.md`, and `docs/clean-room/POLICY.md` before writing anything.

## Non-negotiables

1. **No vendor artefacts in the repo.** No binaries, firmware, SDK headers, manual text, screenshots of documentation, or decompiler output. `.gitignore` catches the obvious cases; judgement covers the rest.
2. **No verbatim vendor strings, identifiers, comments, or tables.** Constants dictated by hardware are described functionally with a provenance ID.
3. **Spec before code.** Protocol behaviour goes in `docs/spec/` first, in our own words, with `PROVENANCE.md` rows. Rust follows the spec, never a trace directly.
4. **No decompilation without a `DECOMPILATION-LOG.md` entry** stating why observation failed. Default answer is "don't".
5. **Own names.** No "phantom", "sensable", "ghost", or "openhaptics" in crate names, binary names, module paths, or identifiers. Documentation may use them referentially.
6. **Safety code is not optional.** Deadline watchdog, force clamp, and amp-enable interlock ship in the first version of `device`, before any force is commanded on real hardware.

## Suggested first tasks, in order

1. **Trace decoder** (`tools/trace-decode`): parse `strace -tt -T` output and QEMU `parallel_ioport_*` trace logs into a common timeline format `(t_us, register, dir, value)`; emit JSON/CSV. Also import sigrok VCD. This is the input to everything else and needs no hardware.
2. **`hal` crate**: a `PortIo` trait; `ppdev` backend via `nix` ioctl bindings for `include/uapi/linux/ppdev.h` (PPCLAIM/PPRELEASE/PPEXCL, PPRDATA/PPWDATA, PPRCONTROL/PPWCONTROL/PPFCONTROL, PPRSTATUS, PPDATADIR, PPNEGOT, PPSETMODE/PPGETMODE, PPGETMODES, PPSETTIME, PPWCTLONIRQ/PPCLRIRQ) plus EPP `read`/`write`; a `replay` backend that plays a decoded timeline and asserts the caller's writes match. `ioperm` backend later.
3. **Port probe tool** (`tools/port-probe`): open `/dev/parport0`, claim, report `PPGETMODES`, negotiate EPP, release. Confirms the PCIe card can do EPP at all. Safe to run with the device connected; it writes nothing to the data lines.
4. **`protocol` crate**: empty until `docs/spec/` has its first section. Then implement exactly what the spec says, with replay tests against the captures that produced the spec.
5. **`device` and `daemon`**: after `protocol` decodes a full servo cycle from a capture.

## Environment facts

- CachyOS: `parport`, `parport_pc`, `ppdev`, `parport_serial` are modules; `modprobe` them and add a udev rule for `/dev/parport0` group access.
- PCIe cards do not sit at 0x378; always discover the base address via ppdev, never hardcode.
- USB-to-parallel adapters cannot be used; printer-class only.
- No `parport`/`ppdev` crates exist on crates.io; write the bindings with `nix` or `rustix`.
- Real-time: `thread-priority` crate or raw `libc` for SCHED_FIFO; `mlockall`; `clock_nanosleep` with `TIMER_ABSTIME`; `linux-cachyos-rt-bore` for the final loop.

## Where things go

```
docs/spec/            protocol specification, one file per phase (enumerate, negotiate, amp, servo, home, shutdown)
docs/clean-room/      provenance and decompilation logs (append-only)
captures/             decoded timelines only (JSON/CSV); raw .sr/.vcd stay out of git, referenced by SHA-256
crates/hal            port I/O trait + backends
crates/protocol       pure protocol logic
crates/device         state machine + safety
crates/daemon         RT loop + IPC
tools/                trace-decode, port-probe
```
