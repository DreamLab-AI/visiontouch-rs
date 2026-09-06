# Provenance log

Every fact about the device or protocol that enters this repository must have
a row here. The purpose is to be able to show, later, exactly where every bit
of protocol knowledge came from and that none of it was copied from vendor
code. Append-only; never rewrite history in this file.

| ID | Date | Fact / artefact | Source class | Source detail | Who | Notes |
|----|------|-----------------|--------------|---------------|-----|-------|
| P-0001 | 2026-09-06 | Device is a parallel-interface SensAble PHANTOM ("pro v1 parallel desktop" per owner) | OWNER | Owner statement | owner | Model to be confirmed from label |

## Source classes

| Class | Meaning | Risk |
|-------|---------|------|
| PUBLIC | Published paper, thesis, patent, datasheet, manual, standard (IEEE 1284), mainline kernel source | none |
| OWNER | Physical inspection of hardware the project owns; labels, PCB, connectors, continuity | none |
| OBSERVE | Passive capture of signals on the cable, or of I/O port traffic, while the *original* software drives the device | very low (CDPA s50BA / EU Art 5(3)) |
| BLACKBOX | Feeding inputs to the original software / hardware and recording outputs, without inspecting code | very low (s50BA) |
| DECOMPILE | Disassembly or decompilation of vendor binaries | permitted only for interoperability and only where the fact was not obtainable from the classes above (CDPA s50B / EU Art 6). Requires a separate entry in `DECOMPILATION-LOG.md` and must be performed by the spec team, never the implementation team. |

## Rules of thumb

1. Prefer the lowest-risk class that can yield the fact.
2. A DECOMPILE entry must say *why* OBSERVE/BLACKBOX could not produce the fact.
3. Constants, tables and magic numbers must be described functionally in the
   spec ("the enable byte is 0x2A, observed on the wire at power-on"), never
   pasted as code from a decompiler.
