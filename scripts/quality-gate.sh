#!/usr/bin/env bash
# Single local quality gate (ADR-0005). CI runs the same steps.
# Never opens hardware: no step touches /dev/parport0 or uses scripts/with-port-access.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd -- "$repo_dir"

step() { printf '\n== %s\n' "$*"; }

step "ADR ledger frontmatter, supersession edges and staleness"
node scripts/adr-index-gen.cjs docs/adr --check

step "cargo fmt --check"
cargo fmt --all -- --check

step "cargo clippy (all targets, warnings are errors)"
cargo clippy --workspace --all-targets -- -D warnings

step "cargo test --workspace"
cargo test --workspace

step "strict rustdoc for every crate"
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps

if [[ "${SKIP_VIEWER:-0}" != 1 ]]; then
  step "workspace viewer unit tests and formatting"
  if [[ ! -f apps/workspace-viewer/node_modules/three/package.json ]]; then
    npm ci --prefix apps/workspace-viewer
  fi
  npm test --prefix apps/workspace-viewer
  npm run format:check --prefix apps/workspace-viewer
fi

step "offline audio analysis tests"
python3 -m unittest discover -s tools/audio-analysis -p 'test_*.py'

step "no vendor artefacts tracked"
if git ls-files | grep -E -i '\.(sys|vxd|dll|exe|bin|iso|img)$|^vendor-original/|^disasm/' ; then
  echo "error: vendor artefact tracked (docs/clean-room/POLICY.md rule 5)" >&2
  exit 1
fi

printf '\nquality gate passed at %s\n' "$(git rev-parse HEAD)"
