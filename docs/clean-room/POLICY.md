# Clean-room policy ("clean-room lite")

This is a single-developer, private, non-commercial project. A formal two-team Chinese wall is more process than the risk warrants (`docs/legal/landscape.md` §8). Instead:

1. **Observation first.** Protocol facts come from the wire, from ppdev traces, and from black-box use of the vendor software. See `docs/research/capture-strategy.md`.
2. **Spec before code.** Every protocol behaviour is written up in `docs/spec/` (prose, tables, timing diagrams, pseudocode in our own words) **before** the corresponding Rust is written. Commit order is evidence.
3. **Provenance for everything.** Every fact gets a row in `PROVENANCE.md` with its source class. Nothing enters the spec without a row.
4. **Decompilation is a last resort.** It requires a `DECOMPILATION-LOG.md` entry stating why observation failed, is confined to the question at hand, and its results feed only the spec. Nothing traced to a DECOMPILE row may appear in the paper (CDPA s50B(3)).
5. **No vendor artefacts in the repo.** No binaries, firmware, SDK headers, manuals, screenshots of documentation, or decompiler output. `.gitignore` enforces the obvious cases; judgement covers the rest.
6. **No verbatim anything.** No vendor strings, comments, identifiers, table contents, or manual text. Constants dictated by the hardware are described functionally with their observed source ("0x2A, observed on the wire as the amp-enable value, P-0042").
7. **Own names.** Crates, binaries, and the project do not use "PHANTOM", "SensAble", "GHOST", or "OpenHaptics".
8. **Revisit before any release.** The posture above assumes no distribution. See `docs/legal/POSITION.md` trigger points.
