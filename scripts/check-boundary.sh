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
# anyview-ui holds the pure machines and the views that draw them. The crate itself may name the
# window (ds-blitz, which brings tokio and wgpu) and the two back ends (which bring image); it never
# names a bus, a PDF library or a player. The machines inside it stay pure, which is checked per
# source file below.
# anyview-image and anyview-text are blocking back ends the launcher links: no runtime, no bus, no
# GPU, no UI, no Blitz, no player, and neither reaches the other's codecs (the image crate has no
# highlighter or Markdown parser, the text crate no image decoder).
RULES=(
  "anyview-core: dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv image syntect blitz-dom anyrender serde_json"
  "anyview-store: dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv image blitz-dom blitz-paint anyrender"
  "anyview-ui: zbus pdfrum mpv-wgpu-player rsmpv"
  "anyview-image: dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv blitz-dom blitz-paint blitz-traits blitz-html blitz-shell blitz-kit anyrender syntect pulldown-cmark"
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
  "anyview-core: ds-core"
  "anyview-store: anyview-core"
  "anyview-ui: anyview-core anyview-image anyview-text ds ds-blitz ds-core"
  "anyview-image: anyview-core ds-core"
  "anyview-text: anyview-core ds-core"
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

# The machines of anyview-ui are pure: their source names no view, no quire component, no decoder,
# no disk, no thread and no clock. The effects are carried out by `io`, `families` and `views`.
MACHINES=(chrome command keys load navigate palette panel presentation sheet stage time typed viewer)
for machine in "${MACHINES[@]}"; do
  path="crates/anyview-ui/src/$machine"
  [ -d "$path" ] || path="$path.rs"
  hits=$(grep -rnE '\bdioxus\b|\bds::|\bds_blitz\b|\banyview_image\b|\banyview_text\b|std::fs|std::thread|std::time::(Instant|SystemTime)|futures_' "$path" || true)
  if [ -n "$hits" ]; then
    echo "IMPURE: the machine $machine names an effect or a view"
    echo "$hits" | head -10
    fail=1
  fi
done
echo "machines hold: ${MACHINES[*]} name no view, decoder, disk, thread or clock"

exit "$fail"
