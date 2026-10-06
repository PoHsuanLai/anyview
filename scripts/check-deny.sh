#!/usr/bin/env bash
# The dependency gate: every cargo-deny check (advisories, bans, licenses, sources) against
# deny.toml. Duplicate versions warn and never fail. The advisory database is fetched when
# cargo-deny can reach the network and read from its cache when it cannot.
#
#   scripts/check-deny.sh
set -euo pipefail
cd "$(dirname "$0")/.."
command -v cargo-deny >/dev/null 2>&1 || { echo "check-deny: cargo-deny is not installed (cargo install cargo-deny)" >&2; exit 2; }
cargo deny check advisories bans licenses sources
