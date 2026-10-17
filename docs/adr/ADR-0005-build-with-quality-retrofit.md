---
id: ADR-0005
title: Retrofit the repository with the estate's build-with-quality gates and a local ADR ledger
date: 2026-10-17
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: e6f9d018f1ee88bd4db315937c9d66a3b95d1818
verified_paths: [scripts/quality-gate.sh, scripts/adr-index-gen.cjs, .github/workflows/ci.yml, AGENTS.md]
owner: jjohare
review_trigger: a gate that cannot run on GitHub-hosted runners, or a change to the estate ADR status contract
repo: visiontouch-rs
---

# ADR-0005: Retrofit the repository with the estate's build-with-quality gates and a local ADR ledger

## Context

Until this record the repository had no CI, no ADR ledger and no agent
instructions file; decisions lived in a long session handoff and in spec
documents. The owner asked for the repository to be brought into the VisionFlow
estate and made usable by others. The estate convention (agentbox, VisionFlow,
VisionClaw) is a living governing document plus a thin ADR ledger validated by
`adr-index-gen`, a per-repository `AGENTS.md` that `CLAUDE.md` imports, and
executed quality gates rather than asserted ones.

## Decision

`docs/BASELINE-visiontouch-rs.md` is the governing document; `docs/adr/` holds the
ledger with the VisionFlow template and three-axis status, indexed by
`scripts/adr-index-gen.cjs` (copied from agentbox, repo enum `visiontouch-rs`,
`verified_paths` staleness gate on). `scripts/quality-gate.sh` is the single
local gate: `cargo fmt --check`, `cargo clippy --workspace --all-targets -D
warnings`, `cargo test --workspace`, `RUSTDOCFLAGS='-D warnings' cargo doc
--workspace --no-deps`, the viewer's `npm test`, the audio tool's `unittest`
suite and the ADR index check. `.github/workflows/ci.yml` runs the same gate on
push and pull request. `AGENTS.md` carries the repository rules for every coding
agent; `CLAUDE.md` contains only `@AGENTS.md`. Expectation-driven evidence for
physical behaviour is recorded as `P-NNNN` rows, never as narrative; software
evidence is a command, its output and the git sha.

## Consequences

Contributors and agents get one command to run before a commit and the same
command in CI. Physical acceptance remains outside CI by construction: the gate
proves the software claims in each ADR, and the attended-session protocol in
`docs/spec/14-force-calibration.md` proves the physical ones. Hardware-opening
tests are never part of the gate. Graphics smoke modes run only where a display
is available and are excluded from the hosted runner.

## Verification

`scripts/quality-gate.sh` exits 0 locally; `node scripts/adr-index-gen.cjs
docs/adr --check` exits 0. The workflow file is validated by the first push.
`verified_commit` is the commit that landed this record.
