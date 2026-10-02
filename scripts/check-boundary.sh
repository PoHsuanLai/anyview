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
# anyview-peek is the light tier the launcher links: it draws with quire's `ds` and `ds-blitz` (so
# Blitz, the renderer and, through `ds-blitz`, `wgpu` and pdfrum are in its tree) but never the media
# player or D-Bus. What it may not name itself is the DIRECT table below.
RULES=(
  "anyview-core: dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv image syntect blitz-dom anyrender serde_json"
  "anyview-store: dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv image blitz-dom blitz-paint anyrender"
  "anyview-ui: tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv image"
  "anyview-image: dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv blitz-dom blitz-paint blitz-traits blitz-html blitz-shell blitz-kit anyrender syntect pulldown-cmark"
  "anyview-text: dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv blitz-dom blitz-paint blitz-traits blitz-html blitz-shell blitz-kit anyrender image resvg jxl-oxide"
  "anyview-peek: mpv-wgpu-player rsmpv zbus ashpd"
)

# Dependencies a crate may reach only THROUGH another, never name in its own manifest. anyview-peek
# links `wgpu`, pdfrum and tokio because `ds-blitz` does; it must not depend on them itself, so a
# texture is made through `ds-blitz`'s `TextureLayer`, a page through its `PdfFileThumb` cache, and
# nothing here spawns.
DIRECT=(
  "anyview-peek: wgpu pdfrum pdfrum-anyrender pdfrum-edit tokio anyrender anyrender_vello_hybrid vello_hybrid blitz-dom blitz-paint blitz-html blitz-shell dioxus-native"
)

# The most distinct packages (name and version) `cargo tree -p <crate>` may list, normal and build
# dependencies only. The launcher links anyview-peek, so growth here is growth of its binary: raise a
# budget in the change that adds the dependency, with the reason (FINDINGS). anyview-peek is 532 today,
# almost all of it `ds` and `ds-blitz`, which the launcher already links.
BUDGETS=(
  "anyview-peek: 560"
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

for rule in "${DIRECT[@]}"; do
  crate="${rule%%:*}"
  read -r -a forbidden <<<"${rule#*:}"
  direct=$(cargo tree -p "$crate" --depth 1 -e normal,build --prefix none --all-features 2>/dev/null \
    | awk '{print $1}' | sort -u)
  named=0
  for dep in "${forbidden[@]}"; do
    if grep -qx "$dep" <<<"$direct"; then
      echo "DIRECT: $crate names $dep itself; reach it through ds-blitz"
      named=1
      fail=1
    fi
  done
  if [ "$named" -eq 0 ]; then
    echo "direct deps hold: $crate names none of ${forbidden[*]}"
  fi
done

for budget in "${BUDGETS[@]}"; do
  crate="${budget%%:*}"
  limit="${budget#*: }"
  count=$(cargo tree -p "$crate" -e normal,build --prefix none --format '{p}' 2>/dev/null \
    | sed 's/ (\*)$//' | sort -u | grep -c .)
  if [ "$count" -gt "$limit" ]; then
    echo "BUDGET: $crate has $count packages, the budget is $limit"
    fail=1
  else
    echo "budget holds: $crate has $count packages of $limit"
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
  "anyview-ui: anyview-core ds-core"
  "anyview-image: anyview-core ds-core"
  "anyview-text: anyview-core ds-core"
  "anyview-peek: anyview-core anyview-image anyview-text ds ds-blitz"
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

exit "$fail"
