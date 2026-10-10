#!/usr/bin/env bash
# The portable build (quire's check-portable.sh convention, design/36 section 4, in this repo's terms):
#   (a) the whole workspace compiles with every default feature off, so without `quire-desktop`, the
#       Linux desktop's services; anyview-platform's own tree has no zbus. quire still brings zbus
#       (ds-settings, ds-helpers) and wayland-client (ds-blitz) into the dependency graph until its
#       own portable lane lands; this checks anyview's code, not the final graph.
#   (b) no module outside a `desktop/` directory or a file named `desktop.rs` names `crate::desktop`
#       or zbus: the core never reaches the desktop modules or a D-Bus crate. Tests and the platform crate's `testing` support are exempt.
#       ds-desktop is not a leak: it is quire's probe, and without its `dbus` feature it has no bus.
#   (c) anyview-peek compiles on its own, headless and with `media` (see below).
#   (d) anyview-pane, the viewer as a pane another app hosts, compiles on its own and reaches no bus
#       and no docket or porter: it asks the OS for nothing (abilities arrive as `PlatformAbilities`).
#       The same with its `player` feature, which adds the player host over the person's own mpv.
#
#   scripts/check-portable.sh
set -euo pipefail
cd "$(dirname "$0")/.."
cargo check --workspace --no-default-features --locked
# (c) anyview-peek alone, as sill and other embedders take it: a workspace build unifies features, so
#     a peek that needs a feature only the viewer turns on (anyview-image's `encode`, 2026-10-10)
#     compiles here but not in a launcher. Built headless, and with `media`, the launcher's set.
cargo check -p anyview-peek --no-default-features --locked
cargo check -p anyview-peek --no-default-features --features media --locked
# (d) The pane alone, as temor takes it, and its tree holds none of the desktop's crates.
cargo check -p anyview-pane --no-default-features --locked
for dep in zbus ashpd docket-client docket-core porter-core; do
  if cargo tree -p anyview-pane -e normal,build -i "$dep" 2>/dev/null | grep -q .; then
    echo "check-portable: anyview-pane reaches $dep" >&2
    exit 1
  fi
done
# With its `player` feature the pane plays recordings through anyview-media-host (the person's own mpv, run
# as a child process): it compiles alone and still reaches no bus, no docket or porter.
cargo check -p anyview-pane --features player --locked
for dep in zbus ashpd docket-client docket-core porter-core; do
  if cargo tree -p anyview-pane --features player -e normal,build -i "$dep" 2>/dev/null | grep -q .; then
    echo "check-portable: anyview-pane --features player reaches $dep" >&2
    exit 1
  fi
done
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
echo "check-portable: the workspace builds without quire-desktop, anyview-peek builds alone (headless and with media), anyview-pane builds alone and reaches no bus, anyview-platform has no zbus and no core module names crate::desktop or zbus"
