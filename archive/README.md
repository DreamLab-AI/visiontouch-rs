# archive/ (gitignored)

Local holding area for superseded captures, logs, exports and scratch material
that should stay off the owner's desk but are not evidence. Everything here is
ignored by git except this file.

Promotion rule: a file becomes evidence only when it gains a `P-NNNN` row in
`docs/clean-room/PROVENANCE.md` and moves under `docs/evidence/<topic>/` with a
README stating its SHA-256 and how it was produced. Vendor artefacts never go
here; they stay under the ignored `vendor-original/` on the owner's machine.
