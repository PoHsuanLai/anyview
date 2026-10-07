#!/usr/bin/env bash
# The portable build: the whole workspace compiles with every default feature off (so without
# `quire-desktop`, the Linux desktop's services), and anyview-platform's own tree has no zbus. quire still
# brings zbus (ds-settings) and wayland-client (ds-blitz) into the dependency graph until its own
# portable lane lands; this checks anyview's code, not the final graph.
#
#   scripts/check-portable.sh
set -euo pipefail
cd "$(dirname "$0")/.."
cargo check --workspace --no-default-features --locked
if cargo tree -p anyview-platform --no-default-features -e normal,build -i zbus 2>/dev/null | grep -q .; then
  echo "check-portable: anyview-platform reaches zbus without quire-desktop" >&2
  exit 1
fi
echo "check-portable: the workspace builds without quire-desktop and anyview-platform has no zbus"
