# Project charter

## What this is

A clean-room, open-source reimplementation in Rust on Linux (CachyOS) of the
host-side protocol stack for a SensAble PHANTOM haptic device that uses the
original **parallel-port (EPP/LPT) interface**, circa 1998–2003. The vendor
stack ran on Windows 98/ME/NT and is fully out of support. The intellectual
property is understood to have passed SensAble → Geomagic (2012) → 3D Systems
(2013); see `docs/legal/`.

## Current state (2026-09-06)

- Device on hand: user describes it as a "PHANTOM pro v1 parallel desktop".
  Exact model (PHANTOM Desktop vs PHANTOM Premium 1.0) to be confirmed from
  the base-plate label: see `docs/hardware/identification.md`.
- Host: CachyOS (Arch-based), PCIe parallel-port card installed, DB25 cable
  connected, and the kernel enumerates the port.
- No code yet. This repository currently holds research and legal notes only.

## Release posture

**Private hardware-archaeology project. No external software release is
planned.** The intended output is an academic paper describing the device,
its interface, and the reimplementation. This matters legally:

- Without distribution, most copyright exposure collapses to private study of
  a lawfully owned copy, which UK CDPA s50BA / EU Art 5(3) expressly permit.
- Non-commercial research fair dealing (CDPA s29) is additionally available.
- The one trap: CDPA s50B(3) restricts *communicating* information obtained
  by **decompilation** beyond what interoperability needs. Information
  obtained by **observation** carries no such restriction. So anything that
  goes in the paper must be traceable (via `docs/clean-room/PROVENANCE.md`)
  to PUBLIC, OWNER, OBSERVE or BLACKBOX sources, not DECOMPILE.
- Patents are not infringed by private, non-commercial experimental use in
  the UK (Patents Act 1977 s60(5)(a),(b)), and the relevant patents should
  be expired anyway. To be confirmed in `docs/legal/`.

## Goals

1. Establish, **before any reverse engineering**, a written legal position and
   a clean-room process that is proportionate to a small open-source project.
2. Identify the device precisely and collect the publicly available technical
   record (papers, theses, patents, manuals): the "readily available
   information" that UK/EU law requires us to exhaust before decompiling.
3. Capture the wire protocol by **observation** (logic analyser / port trace)
   from the original hardware + software, in preference to decompilation.
4. Implement a layered Rust stack: port HAL → protocol → device state machine →
   real-time servo daemon → application API, with safety interlocks.

## Non-goals

- Redistributing any vendor binaries, firmware, SDK headers, or documentation.
- Reimplementing the GHOST / OpenHaptics *API surface* (a rendering SDK). We
  need a device driver, not an SDK clone. Any higher-level API will be our own.
- Supporting the later FireWire (Omni) or Ethernet PHANTOM models, or the
  PCI-card interface (that one already has a mainline Linux driver,
  `drivers/misc/phantom.c`).

## Naming

Do not use "PHANTOM", "SensAble", "GHOST" or "OpenHaptics" in the crate names,
binary names, or project title. They are (or were) trademarks. Refer to the
device descriptively in documentation only ("the parallel-interface PHANTOM").
