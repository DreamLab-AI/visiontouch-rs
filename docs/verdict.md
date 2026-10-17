# Verdict (2026-09-06): proceed, observation-first

## Decision

**Go ahead.** The project proceeds as a private, non-commercial hardware-archaeology effort with an academic paper as the output. Reverse engineering is done by **observation first**; decompilation is a logged last resort and nothing traced to it goes in the paper.

## Why the risk profile is favourable

- Lawfully owned hardware and a 25-year-old product that the rights holder (3D Systems) neither sells nor supports.
- No DRM or technological protection measure on an EPP link, so anti-circumvention law is not engaged.
- UK CDPA 1988 s50BA expressly permits observing and testing a lawfully used program; s296A voids licence terms to the contrary; *SAS v World Programming* (CJEU C-406/10; [2013] EWCA Civ 1482) holds that protocols, interfaces and data formats are not protected expression.
- No distribution is planned, which removes most remaining copyright exposure; s29 non-commercial research fair dealing and Patents Act 1977 s60(5) private/experimental use cover the rest.
- The parallel PHANTOM is a plain IEEE-1284 EPP peripheral and SensAble's own Linux driver was a userspace program over `ppdev`, so the full register-level dialogue is capturable with `strace` or QEMU port-passthrough tracing before anyone opens a disassembler.

## The one constraint that shapes everything

CDPA s50B(3) restricts onward communication of facts obtained by **decompilation**. Facts obtained by **observation** carry no such restriction. Therefore every fact that reaches the paper must trace, through `docs/clean-room/PROVENANCE.md`, to a PUBLIC, OWNER, OBSERVE or BLACKBOX row.

## Before the first capture

1. Confirm the exact model from the base-plate label and fill in `docs/hardware/identification.md`. "Pro" is not a verified SensAble designation.
2. Check whether 3D Systems' support portal still offers the legacy Linux Phantom Device Driver behind free registration. It is the cheapest capture route.
3. Email 3D Systems asking for legacy parallel-interface documentation for a non-commercial archaeology paper. A "no" changes nothing; a "yes" removes most remaining caution.

## Engineering direction

- Userspace only. No kernel module, Rust or otherwise.
- Rust workspace: `hal` (ppdev, ioperm, trace-replay backends), `protocol` (pure, `no_std`-friendly, tested against captures), `device` (state machine, watchdog, force clamp, amp-enable interlock), `daemon` (SCHED_FIFO 1 kHz loop on an isolated core, IPC).
- `linux-cachyos-rt-bore` plus `isolcpus`/`nohz_full` for the servo loop; the default kernel is fine for capture and prototyping.
- Ghidra and Binary Ninja stay on the shelf unless a specific interoperability question survives all three observation routes; if used, log it in `DECOMPILATION-LOG.md` first.

## Revisit triggers

Any intent to release software, any commercial use, any decompilation, or any contact from the rights holder. See `docs/legal/POSITION.md`.
