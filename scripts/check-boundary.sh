#!/usr/bin/env bash
# Crate boundaries, mechanically enforced.
#
# `cargo tree -i <dep>` exits 101 when the dependency is absent, which is precisely the state
# we want. Checking the exit status would therefore fail whenever the boundary holds, so we
# check for OUTPUT instead: any line naming the dependency is a leak.
#
# Each rule is "<crate>: <forbidden deps...>". Forbidden names are exact package names; a
# family such as blitz-* is spelled out member by member.
set -uo pipefail
cd "$(dirname "$0")/.."

# anyview-core is the pure vocabulary: no UI, no runtime, no bus, no GPU, no decoder, no media
# player, no highlighter. `serde_json` is a dev-dependency (round-trip tests) and never a normal
# one. The crates above it (anyview-peek and the back ends not yet written) are added to this table
# when they exist; ARCHITECTURE.md section 1 lists the rule each will carry. anyview-store does
# blocking file I/O and nothing else (the launcher links it): no runtime, no UI, no decoder.
# anyview-ui is the viewer's pure machines: no bus, no runtime, no GPU, no decoder, no player.
# anyview-image and anyview-text are blocking back ends the launcher links: no runtime, no bus, no
# GPU, no UI, no Blitz, no player, and neither reaches the other's codecs (the image crate has no
# highlighter or Markdown parser, the text crate no image decoder).
# anyview-platform is the edge: it alone names the bus and the freedesktop formats (checked for every
# other crate further down), and it reaches no UI, GPU, decoder, player or highlighter. It runs on
# the binary's tokio runtime (zbus's tokio feature) and spawns nothing itself.
# anyview (the binary) owns every thread: its runtime is generic over the back ends and names none
# of them, so no player, GPU, decoder, UI toolkit or bus reaches it. The media-thread spike links
# the player and wgpu as dev-dependencies, which `-e normal,build` does not see. The change that
# wires the viewer together amends this row and the edge below with what it links.
RULES=(
  "anyview: dioxus zbus wgpu pdfrum mpv-wgpu-player rsmpv image blitz-dom blitz-paint anyrender"
  "anyview-core: dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv image syntect blitz-dom anyrender serde_json"
  "anyview-store: dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv image blitz-dom blitz-paint anyrender"
  "anyview-ui: tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv image"
  "anyview-image: dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv blitz-dom blitz-paint blitz-traits blitz-html blitz-shell blitz-kit anyrender syntect pulldown-cmark"
  "anyview-platform: dioxus wgpu pdfrum mpv-wgpu-player rsmpv image blitz-dom blitz-paint blitz-traits blitz-html blitz-shell blitz-kit anyrender syntect pulldown-cmark resvg jxl-oxide"
  "anyview-text: dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv blitz-dom blitz-paint blitz-traits blitz-html blitz-shell blitz-kit anyrender image resvg jxl-oxide"
)
fail=0

for rule in "${RULES[@]}"; do
  crate="${rule%%:*}"
  read -r -a forbidden <<<"${rule#*:}"
  # A crate that cargo cannot find would make every check below pass vacuously.
  if ! cargo tree -p "$crate" --depth 0 >/dev/null 2>&1; then
    echo "ERROR: cargo tree cannot resolve $crate; the boundary was not checked"
    fail=1
    continue
  fi
  leaked=0
  for dep in "${forbidden[@]}"; do
    if cargo tree -p "$crate" -i "$dep" -e normal,build 2>/dev/null | grep -q .; then
      echo "LEAK: $crate depends on $dep"
      cargo tree -p "$crate" -i "$dep" -e normal,build 2>/dev/null | head -20
      leaked=1
      fail=1
    fi
  done
  if [ "$leaked" -eq 0 ]; then
    echo "boundary holds: $crate reaches none of ${forbidden[*]}"
  fi
done

# The allowed edges between our own crates (ARCHITECTURE.md section 1): each crate's direct normal
# and build dependencies that live in a path (this workspace and the sibling quire checkout), and
# nothing else. A dependency on a crate not listed here is a leak; so is one the crate no longer
# has, so the table stays exact. `ds-core`'s `#[derive(Word)]` is re-exported by `ds-core` itself,
# so `ds-core-derive` is not an edge.
EDGES=(
  "anyview: anyview-core"
  "anyview-core: ds-core"
  "anyview-store: anyview-core"
  "anyview-ui: anyview-core ds-core"
  "anyview-image: anyview-core ds-core"
  "anyview-text: anyview-core ds-core"
  "anyview-platform: anyview-core ds-core"
)
for edge in "${EDGES[@]}"; do
  crate="${edge%%:*}"
  read -r -a allowed <<<"${edge#*:}"
  found=$(cargo tree -p "$crate" --depth 1 -e normal,build --prefix none --all-features 2>/dev/null \
    | grep '(/' | awk '{print $1}' | grep -vx "$crate" | sort -u | tr '\n' ' ')
  want=$(printf '%s\n' "${allowed[@]}" | grep . | sort -u | tr '\n' ' ')
  if [ "$found" != "$want" ]; then
    echo "EDGE: $crate depends on [${found% }], the table allows [${want% }]"
    fail=1
  else
    echo "edges hold: $crate depends on [${found% }]"
  fi
done

# Only anyview-platform names the bus or the freedesktop formats (ARCHITECTURE.md section 1): every
# other crate in crates/ must reach none of them, however indirectly.
EDGE_ONLY=(zbus ashpd freedesktop-desktop-entry freedesktop-icons freedesktop-file-parser)
for dir in crates/*/; do
  crate="$(basename "$dir")"
  [ "$crate" = "anyview-platform" ] && continue
  if ! cargo tree -p "$crate" --depth 0 >/dev/null 2>&1; then
    echo "ERROR: cargo tree cannot resolve $crate; the platform-only names were not checked"
    fail=1
    continue
  fi
  leaked=0
  for dep in "${EDGE_ONLY[@]}"; do
    if cargo tree -p "$crate" -i "$dep" -e normal,build 2>/dev/null | grep -q .; then
      echo "LEAK: $crate reaches $dep, which only anyview-platform may name"
      leaked=1
      fail=1
    fi
  done
  if [ "$leaked" -eq 0 ]; then
    echo "platform-only names held: $crate reaches none of ${EDGE_ONLY[*]}"
  fi
done

exit "$fail"
