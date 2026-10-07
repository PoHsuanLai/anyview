#!/usr/bin/env bash
# The portable build (quire's check-portable.sh convention, design/36 section 4, in this repo's terms):
#   (a) the whole workspace compiles with every default feature off, so without `quire-desktop`, the
#       Linux desktop's services; anyview-platform's own tree has no zbus. quire still brings zbus
#       (ds-settings, ds-helpers) and wayland-client (ds-blitz) into the dependency graph until its
#       own portable lane lands; this checks anyview's code, not the final graph.
#   (b) no module outside a `desktop/` directory or a file named `desktop.rs` names `crate::desktop`
#       or zbus: the core never reaches the desktop modules or a D-Bus crate. Tests and the platform crate's `testing` support are exempt.
#       ds-desktop is not a leak: it is quire's probe, and without its `dbus` feature it has no bus.
#
#   scripts/check-portable.sh
set -euo pipefail
cd "$(dirname "$0")/.."
cargo check --workspace --no-default-features --locked
if cargo tree -p anyview-platform --no-default-features -e normal,build -i zbus 2>/dev/null | grep -q .; then
  echo "check-portable: anyview-platform reaches zbus without quire-desktop" >&2
  exit 1
fi
leaks=$(find . -path ./target -prune -o -name '*.rs' -print \
  | grep -vE '(^|/)(target|desktop|\.git)/|(^|/)desktop\.rs$|/tests/|/testing/|/benches/' \
  | xargs -r grep -nE 'crate::desktop\b|\bzbus::|\buse zbus\b|\bextern crate zbus' 2>/dev/null \
  | grep -vE '^[^:]+:[0-9]+:\s*//' || true)
if [ -n "$leaks" ]; then
  echo "check-portable: core modules name crate::desktop or zbus:" >&2
  echo "$leaks" | head -50 >&2
  exit 1
fi
echo "check-portable: the workspace builds without quire-desktop, anyview-platform has no zbus and no core module names crate::desktop or zbus"
