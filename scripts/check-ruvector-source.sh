#!/usr/bin/env bash
# Refuse an absent, wrong-revision or modified consumer dependency.
set -euo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
sibling="${repo}/../ruvector"
expected="677b2475409c50cb964be8a3b848da2952390535"
actual="$(git -C "$sibling" rev-parse HEAD 2>/dev/null)" || { echo 'Missing RuVector sibling; follow docs/development-source.md' >&2; exit 1; }
[ "$actual" = "$expected" ] || { echo "RuVector revision mismatch: expected $expected, got $actual" >&2; exit 1; }
git -C "$sibling" diff --exit-code HEAD -- Cargo.toml Cargo.lock crates/ruvector-core >/dev/null || { echo 'RuVector consumer source has uncommitted changes' >&2; exit 1; }
[ -z "$(git -C "$sibling" ls-files --others --exclude-standard -- crates/ruvector-core)" ] || { echo 'RuVector consumer source has untracked files' >&2; exit 1; }
echo "PASS RuVector consumer source $expected"
