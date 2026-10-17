**How to work against this pack** (engineering / build-with-quality agents start here):

The ADR pack for this repository is **its living governing document plus the
ledger records below that amend it**. The living document is normative: its
*Invariants* section is the compliance surface and its *Change process* section
says how to amend it.

| Domain | Governing document |
|---|---|
| Clean-room method, provenance classes, legal posture, safety envelope, calibration workflow, quality gates | [`../BASELINE-visiontouch-rs.md`](../BASELINE-visiontouch-rs.md) |

**Lookup order:** governing document → its `file:line` citations into code and
specs → the ledger records below → `docs/spec/` for protocol facts (each with a
`P-NNNN` provenance row) → `docs/history/2026-10-16-session-handoff.md` only for
history, never as authority.

**Making a decision:** copy [`TEMPLATE.md`](TEMPLATE.md) to `ADR-NNNN-slug.md`
(next free number), fill the three-axis status honestly, update
`BASELINE-visiontouch-rs.md` **in the same change**, and regenerate this index
(`node scripts/adr-index-gen.cjs docs/adr`). CI runs `--check`: invalid
frontmatter, asymmetric supersession edges and stale `verified_commit` +
`verified_paths` claims fail the build.

**Physical claims are separate from software claims.** An ADR may record that a
force path is implemented and tested offline; it may not record that the device
delivers a force until an attended session has produced a `P-NNNN` OWNER or
OBSERVE row. The estate status contract lives in VisionFlow
(`docs/architecture/adr-status-contract.md`); the numbering here is local
(`ADR-0001` onwards) and VisionFlow's own record of this repository is
`visionflow:ADR-2017`.
