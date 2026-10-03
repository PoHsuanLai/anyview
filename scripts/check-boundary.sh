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
# window (ds-blitz, which brings tokio and wgpu) and the three back ends (which bring image and
# pdfrum); it never names a bus, a PDF library itself (the DIRECT table) or a player. The machines inside it stay pure, which is checked per
# source file below.
# anyview-image and anyview-text are blocking back ends the launcher links: no runtime, no bus, no
# GPU, no UI, no Blitz, no player, and neither reaches the other's codecs (the image crate has no
# highlighter or Markdown parser, the text crate no image decoder).
# anyview-archive and anyview-font are the same kind of blocking back end: no runtime, no bus, no GPU,
# no UI, no Blitz, no player, no image decoder, highlighter or Markdown parser. The archive crate is
# the one that names the container codecs (zip, tar, 7z, gzip, bzip2, xz, Zstandard), and the font
# crate the one that names skrifa for reading a face; neither reaches the other's.
# anyview-platform is the edge: it alone names the bus and the freedesktop formats (checked for every
# other crate further down), and it reaches no UI, GPU, decoder, player or highlighter. It runs on
# the binary's tokio runtime (zbus's tokio feature) and spawns nothing itself.
# anyview-peek is the light tier the launcher links: it draws with quire's `ds` and `ds-blitz` (so
# Blitz, the renderer and, through `ds-blitz`, `wgpu` and pdfrum are in its tree) but never libmpv or
# D-Bus. It links libav through `anyview-media`'s `ffmpeg` feature, to read a recording's facts and
# cover art for the pane. What it may not name itself is the DIRECT table below.
# anyview-pdf is the same kind of blocking back end, and the one crate that may name pdfrum. It draws
# to CPU pixels and never encodes them: page images are encoded by anyview-image, so `image` and the
# other codecs stay out (as does the GPU rasterizer, which would bring wgpu), and `rayon` stays out
# because the binary owns every thread.
# anyview (the binary) owns every thread and joins the crates: the window (anyview-ui, ds, ds-blitz),
# the platform edge, the media crate, the store and the core. Everything they bring comes along (the
# renderer, `wgpu`, the decoders, libmpv and libav through anyview-media, D-Bus), so it has no rule of
# what it may reach; what it may not NAME in its own manifest is the DIRECT table below: the binary
# asks for the renderer, a decoder, the player, libav, the bus or a PDF library only through the
# crate that owns it. It does name `dioxus`, for the root component every window shares.
# anyview-media is the only crate that names `ffmpeg-next` and `ffmpeg-sys-next` (libav) and, with its
# `player` feature, `mpv-wgpu-player` and `rsmpv` (libmpv): every other crate's row forbids libav, and
# anyview-peek's forbids both libraries, since the launcher links it with the `ffmpeg` feature alone
# (`cargo tree -p` resolves only that package's features, so the player's stay out of its tree). The
# media crate itself spawns nothing, reads no clock and draws nothing: no runtime, no bus, no UI.
RULES=(
  "anyview-core: dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next image syntect blitz-dom anyrender serde_json"
  "anyview-store: dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next image blitz-dom blitz-paint anyrender"
  "anyview-ui: zbus mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next"
  "anyview-image: dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next blitz-dom blitz-paint blitz-traits blitz-html blitz-shell blitz-kit anyrender syntect pulldown-cmark"
  "anyview-platform: dioxus wgpu pdfrum mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next image blitz-dom blitz-paint blitz-traits blitz-html blitz-shell blitz-kit anyrender syntect pulldown-cmark resvg jxl-oxide"
  "anyview-text: dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next blitz-dom blitz-paint blitz-traits blitz-html blitz-shell blitz-kit anyrender image resvg jxl-oxide"
  "anyview-archive: dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next blitz-dom blitz-paint blitz-traits blitz-html blitz-shell blitz-kit anyrender image resvg jxl-oxide syntect pulldown-cmark skrifa"
  "anyview-font: dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next blitz-dom blitz-paint blitz-traits blitz-html blitz-shell blitz-kit anyrender image resvg jxl-oxide syntect pulldown-cmark zip tar sevenz-rust flate2 bzip2 ruzstd lzma-rs"
  "anyview-peek: mpv-wgpu-player rsmpv zbus ashpd"
  "anyview-pdf: dioxus tokio zbus wgpu mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next blitz-dom blitz-paint blitz-traits blitz-html blitz-shell blitz-kit anyrender image resvg jxl-oxide syntect pulldown-cmark rayon"
  "anyview-media: dioxus tokio zbus pdfrum image syntect pulldown-cmark resvg jxl-oxide blitz-dom blitz-paint blitz-traits blitz-html blitz-shell blitz-kit anyrender"
)

# Dependencies a crate may reach only THROUGH another, never name in its own manifest. anyview-peek
# links `wgpu`, pdfrum and tokio because `ds-blitz` does; it must not depend on them itself, so a
# texture is made through `ds-blitz`'s `TextureLayer`, a page through its `PdfFileThumb` cache, and
# nothing here spawns.
DIRECT=(
  "anyview: zbus ashpd freedesktop-desktop-entry wgpu pdfrum pdfrum-edit mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next image anyrender anyrender_vello_hybrid vello_hybrid blitz-dom blitz-paint blitz-html blitz-shell dioxus-native"
  "anyview-peek: wgpu pdfrum pdfrum-anyrender pdfrum-edit tokio mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next anyrender anyrender_vello_hybrid vello_hybrid blitz-dom blitz-paint blitz-html blitz-shell dioxus-native"
  "anyview-ui: pdfrum pdfrum-anyrender pdfrum-edit"
)

# The most distinct packages (name and version) `cargo tree -p <crate>` may list, normal and build
# dependencies only. The launcher links anyview-peek, so growth here is growth of its binary: raise a
# budget in the change that adds the dependency, with the reason (FINDINGS). anyview-peek is 574 today:
# about 530 are `ds` and `ds-blitz`, which the launcher already links, and the rest the container codecs
# of anyview-archive, skrifa and libav's bindings (anyview-media, `ffmpeg` feature).
BUDGETS=(
  "anyview-peek: 580"
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
  "anyview: anyview-core anyview-image anyview-media anyview-platform anyview-store anyview-ui ds ds-blitz ds-settings"
  "anyview-core: ds-core"
  "anyview-store: anyview-core"
  "anyview-ui: anyview-core anyview-image anyview-pdf anyview-text ds ds-blitz ds-core"
  "anyview-image: anyview-core ds-core"
  "anyview-text: anyview-core ds-core"
  "anyview-platform: anyview-core ds-core"
  "anyview-peek: anyview-archive anyview-core anyview-font anyview-image anyview-media anyview-text ds ds-blitz"
  "anyview-media: anyview-core ds-core"
  "anyview-archive: anyview-core ds-core"
  "anyview-font: anyview-core"
  "anyview-pdf: anyview-core"
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
# other crate in crates/ must reach none of them, however indirectly. The binary links the platform
# crate, so it reaches them through it; its DIRECT row above holds it to never naming them.
EDGE_ONLY=(zbus ashpd freedesktop-desktop-entry freedesktop-icons freedesktop-file-parser)
for dir in crates/*/; do
  crate="$(basename "$dir")"
  [ "$crate" = "anyview-platform" ] && continue
  [ "$crate" = "anyview" ] && continue
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
# The machines of anyview-ui are pure: their source names no view, no quire component, no decoder,
# no disk, no thread and no clock. The effects are carried out by `io`, `families` and `views`.
MACHINES=(chrome command keys load navigate palette panel presentation sheet stage time typed viewer)
for machine in "${MACHINES[@]}"; do
  path="crates/anyview-ui/src/$machine"
  [ -d "$path" ] || path="$path.rs"
  hits=$(grep -rnE '\bdioxus\b|\bds::|\bds_blitz\b|\banyview_image\b|\banyview_pdf\b|\banyview_text\b|std::fs|std::thread|std::time::(Instant|SystemTime)|futures_' "$path" || true)
  if [ -n "$hits" ]; then
    echo "IMPURE: the machine $machine names an effect or a view"
    echo "$hits" | head -10
    fail=1
  fi
done
echo "machines hold: ${MACHINES[*]} name no view, decoder, disk, thread or clock"

exit "$fail"
