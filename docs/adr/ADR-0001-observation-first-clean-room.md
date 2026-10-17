---
id: ADR-0001
title: Recover the protocol by observation first, with every fact carrying a provenance class
date: 2026-10-17
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: e6f9d018f1ee88bd4db315937c9d66a3b95d1818
verified_paths: [docs/clean-room/POLICY.md, docs/legal/POSITION.md, .gitignore]
owner: jjohare
review_trigger: any intent to distribute software, any decompilation request, any contact from the rights holder
repo: visiontouch-rs
---

# ADR-0001: Recover the protocol by observation first, with every fact carrying a provenance class

## Context

The device is a lawfully owned SensAble PHANTOM 1.0A on an EPP parallel port whose
vendor stack is unsupported. The rights holder (3D Systems) is active, so this is
not orphan IP. UK CDPA 1988 s50BA permits observing and testing a lawfully owned
program; s50B permits decompilation for interoperability but restricts onward
communication of what it yields (s50B(3)(c)). The project output is a paper, so
anything traced to decompilation cannot be published. The legal analysis is in
`docs/legal/landscape.md` and the applied position in `docs/legal/POSITION.md`.

## Decision

Protocol facts come from the wire, from ppdev traces and from black-box use of the
vendor software. Every fact in `docs/spec/` carries a row in
`docs/clean-room/PROVENANCE.md` with a source class: PUBLIC, OWNER, OBSERVE,
BLACKBOX, DESIGN or DECOMPILE. Specification precedes implementation; commit order
is evidence. Decompilation is a last resort, requires a dated entry in
`docs/clean-room/DECOMPILATION-LOG.md` naming the artefact hash, the single
interoperability question and why observation could not answer it, and its
findings are DECOMPILE class and excluded from the paper. No vendor binaries,
firmware, headers, manuals, screenshots or decompiler output enter the repository;
`.gitignore` enforces the obvious cases. Crate, binary and project names never use
the vendor's trademarks.

## Consequences

Research on the device is slower than a straight disassembly would be, and some
questions (reset-pose joint offsets, control-byte semantics) are answered by
instrumented black-box experiments on the live unit rather than by reading code.
Two bounded decompilation entries exist (D-0001, D-0002) for overload-release
questions; their findings shape conservative software policy but cannot be cited
in the paper. Any new decompilation request must first show that an observation
route (for example querying the vendor library with the arm in a known pose) is
genuinely unavailable. The REA toolkit survey in
`docs/research/rea-toolkit-applicability.md` classifies which of its capabilities
are observation class and which are gated by this record.

## Verification

`docs/clean-room/POLICY.md` states the eight rules; `docs/clean-room/PROVENANCE.md`
holds P-0001 onwards with source classes; `.gitignore` excludes `/vendor-original/`,
`/disasm/`, `*.sys`, `*.vxd`, `*.dll`, `*.exe`, `*.bin`. Verified by reading those
files at the commit above. `git log --follow docs/spec/10-device-readout.md` shows
the readout specification preceding `tools/device-readout`.
