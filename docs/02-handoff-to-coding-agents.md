# Handoff brief for coding agents

Read `README.md`, `docs/03-session-handoff-2026-10-16.md`, `docs/01-verdict.md`, and `docs/clean-room/POLICY.md` before writing anything. The latest session handoff corrects assumptions in the earlier research and takes precedence for host facts and immediate task order.

## Non-negotiables

1. **No vendor artefacts in the repo.** No binaries, firmware, SDK headers, manual text, screenshots of documentation, or decompiler output. `.gitignore` catches the obvious cases; judgement covers the rest.
2. **No verbatim vendor strings, identifiers, comments, or tables.** Constants dictated by hardware are described functionally with a provenance ID.
3. **Spec before code.** Protocol behaviour goes in `docs/spec/` first, in our own words, with `PROVENANCE.md` rows. Rust follows the spec, never a trace directly.
4. **No decompilation without a `DECOMPILATION-LOG.md` entry** stating why observation failed. Default answer is "don't". If it does become necessary, the Linux-native toolchain is in `docs/research/windows-binary-analysis-on-linux.md`.
5. **Own names.** No "phantom", "sensable", "ghost", or "openhaptics" in crate names, binary names, module paths, or identifiers. Documentation may use them referentially.
6. **Safety code is not optional.** Deadline watchdog, force clamp, and amp-enable interlock ship in the first version of `device`, before any force is commanded on real hardware.

## Suggested first tasks, in order

1. **Trace decoder** (`tools/trace-decode`): parse verified full-buffer strace output and supported QEMU parallel trace logs into a common timeline format `(t_us, register, dir, value)`; emit JSON/CSV. Also import sigrok VCD. This is the input to everything else and needs no hardware.
2. **`hal` crate**: a `PortIo` trait; `ppdev` backend via `nix` ioctl bindings for `include/uapi/linux/ppdev.h` (PPCLAIM/PPRELEASE/PPEXCL, PPRDATA/PPWDATA, PPRCONTROL/PPWCONTROL/PPFCONTROL, PPRSTATUS, PPDATADIR, PPNEGOT, PPSETMODE/PPGETMODE, PPGETMODES, PPSETTIME, PPWCTLONIRQ/PPCLRIRQ) plus EPP `read`/`write`; a `replay` backend that plays a decoded timeline and asserts the caller's writes match. `ioperm` backend later.
3. **Query-only port capability tool** (`tools/port-probe`): report advertised capabilities without negotiation, register writes, or data transfers. Audit open/close and claim/release behaviour before hardware use. Advertised EPP support does not establish working EPP transactions. Treat negotiation as a separate deliberate hardware operation: it can change port signals and requires the capture safety prerequisites in the latest handoff.
4. **`protocol` crate**: empty until `docs/spec/` has its first section. Then implement exactly what the spec says, with replay tests against the captures that produced the spec.
5. **`device` and `daemon`**: after `protocol` decodes a full servo cycle from a capture.

## Environment facts

- Current host: Omarchy (Arch-based), with `parport`, `parport_pc`, and `ppdev` already loaded. `/dev/parport0` exists as `root:lp` mode 0660; the account belongs to `lp`, but the agent process still lacks that supplementary group. Use `tools/with-port-access COMMAND [ARG ...]` to launch port tools with refreshed account groups from the stale session. Access has been verified this way as John. Do not recreate the node or relax its permissions.
- PCIe cards do not sit at 0x378; always discover the base address via ppdev, never hardcode.
- USB-to-parallel adapters cannot be used; printer-class only.
- Reassess available Rust bindings when implementation begins; the earlier crate-availability research is not a current dependency inventory.
- Real-time scheduling, memory locking, absolute deadlines, and kernel choices come after capture and measurement; do not assume a CachyOS kernel on this host.

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
