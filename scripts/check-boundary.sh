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
# Plain text from cargo whatever the caller asks for: a coloured `(*)` slips past the dedup below and
# CI (CARGO_TERM_COLOR=always) counted 800 packages where there are 590.
export CARGO_TERM_COLOR=never
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
# D-Bus, and never libav or libmpv: its `media` feature (default on) reads a recording's facts and
# cover art with pure-Rust parsers (symphonia, mp4parse, matroska-demuxer), and a video's frame comes
# from a thumbnail source the host injects. `cargo tree -p` below runs with the default features, so
# those parsers are in the tree it checks and `ffmpeg-next`, `ffmpeg-sys-next`, `rsmpv` and
# `rsmpv-sys` must not be. It does not depend on `anyview-media` at all. What it may not name itself is the DIRECT table below.
# anyview-book reads EPUB and comic zips through anyview-archive (the one crate that names the container
# codecs) and parses the package with roxmltree: the same blocking, effect-free back end as the archive
# crate, with no image decoder (a cover is bytes, decoded by the light tier) and no highlighter or Markdown
# parser.
# anyview-pdf is the same kind of blocking back end, and the one crate that may name pdfrum. It draws
# to CPU pixels and never encodes them: page images are encoded by anyview-image, so `image` and the
# other codecs stay out (as does the GPU rasterizer, which would bring wgpu), and `rayon` stays out
# because the binary owns every thread.
# anyview (the binary) owns every thread and joins the crates: the window (anyview-ui, ds, ds-blitz),
# the platform edge, the plugin registry, the media crate, the light tier's readers, the store and the
# core. Everything they bring comes along (the renderer, `wgpu`, the decoders, D-Bus), so it has no rule
# of what it may reach; what it may not NAME in its own manifest is the DIRECT table below: the binary
# asks for the renderer, a decoder, the player, a codec library, the bus or a PDF library only through
# the crate that owns it. It does name `dioxus`, for the root component every window shares.
# No crate links libmpv or libav (CONVENTIONS section 15, "run, never link"): `rsmpv`, `rsmpv-sys`,
# `ffmpeg-next` and `ffmpeg-sys-next` are forbidden in every crate's tree by FORBIDDEN_EVERYWHERE
# below, and `dev/ldd-test.sh` checks that the built binary has no libmpv and no libav*. Playing is the
# person's own mpv run as a child process (anyview-media with its `player` feature, through
# `mpv-wgpu-player`'s `subprocess` host, which links no mpv), and probing and writing recordings is the
# FFmpeg plugin. The media crate itself spawns nothing, reads no clock and draws nothing: no runtime,
# no bus, no UI.
# anyview-plugin-protocol is what a plugin author depends on, so it is pure and small: bayonet's wire
# (bayonet without its `host` feature: `serde`, `serde_json` and `thiserror`), `serde` and `thiserror`
# and nothing of the viewer's (no anyview-core, no ds-core, no `toml`), and no runtime, bus, GPU,
# decoder or UI. anyview-plugin is the viewer's capabilities and the registry as values: it names
# anyview-core, the protocol and bayonet (whose host half parses the manifest's TOML), and reaches no
# runtime, bus, GPU, decoder, player or UI. anyview-plugin-fake is a plugin like any other: the protocol crate and nothing else
# (its dev-dependencies, which the checks below do not look at, are the host's crates).
# anyview-ffmpeg (under plugins/, since it is a program shipped as its own package and no layer of
# the viewer) is a plugin like the fake one: the protocol crate, `serde`, `serde_json` and `thiserror`,
# and nothing of the viewer's. It runs the person's ffprobe and ffmpeg, so it names no libav binding, no
# player, and not anyview-media, anyview-platform, the core or the UI; `cargo tree` for it must show
# no `ffmpeg-next` and no `rsmpv` (its dev-dependencies, the host's crates for the tests, are not looked at).
# anyview-heif and anyview-raw are plugins too, over the shared `anyview-tool-kit` (the protocol crate,
# `image` to read the PNG, TIFF or PPM a tool wrote, `tempfile` and `thiserror`): they run libheif's and
# LibRaw's programs and name nothing of the viewer's.
# Forbidden in every crate's tree, whatever its row says: the bindings of libmpv and of libav, and so the
# libraries themselves (CONVENTIONS section 15). Codec and copyleft code lives in separate-process plugins
# that use the person's own distro tools.
# The same for the picture codecs: libheif's bindings, LibRaw's bindings and the raw decoders that are
# LGPL or link C (`rawloader`, `rawler`). Pure-Rust parsing of a container (the viewer reads a raw
# file's embedded JPEG itself) is fine; decoding HEIC or developing a raw file is a plugin that runs the
# person's own libheif or LibRaw tools (anyview-heif, anyview-raw).
FORBIDDEN_EVERYWHERE=(rsmpv rsmpv-sys ffmpeg-next ffmpeg-sys-next libheif-rs libheif-sys libheif-rs-sys libraw-rs libraw-sys rsraw rsraw-sys rawloader rawler)

RULES=(
  "anyview-ffmpeg: anyview-core anyview-media anyview-platform anyview-plugin anyview-ui anyview-peek ds-core ds ds-blitz toml dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next image blitz-dom anyrender syntect"
  "anyview-tool-kit: anyview-core anyview-media anyview-platform anyview-plugin anyview-ui anyview-peek ds-core ds ds-blitz toml dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next blitz-dom anyrender syntect"
  "anyview-heif: anyview-core anyview-media anyview-platform anyview-plugin anyview-ui anyview-peek ds-core ds ds-blitz toml dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next blitz-dom anyrender syntect"
  "anyview-raw: anyview-core anyview-media anyview-platform anyview-plugin anyview-ui anyview-peek ds-core ds ds-blitz toml dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next blitz-dom anyrender syntect"
  "anyview-plugin-protocol: anyview-core ds-core toml dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next image blitz-dom anyrender syntect"
  "anyview-plugin: dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next image blitz-dom blitz-paint anyrender syntect"
  "anyview-plugin-fake: anyview-core ds-core toml dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next image blitz-dom anyrender syntect"
  "anyview-core: dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next image syntect blitz-dom anyrender serde_json"
  "anyview-store: dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next image blitz-dom blitz-paint anyrender"
  "anyview-ui: zbus mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next"
  "anyview-export: zbus mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next"
  "anyview-image: dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next blitz-dom blitz-paint blitz-traits blitz-html blitz-shell blitz-kit anyrender syntect pulldown-cmark"
  "anyview-platform: dioxus wgpu pdfrum mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next image blitz-dom blitz-paint blitz-traits blitz-html blitz-shell blitz-kit anyrender syntect pulldown-cmark resvg jxl-oxide"
  "anyview-text: dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next blitz-dom blitz-paint blitz-traits blitz-html blitz-shell blitz-kit anyrender image resvg jxl-oxide"
  "anyview-archive: dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next blitz-dom blitz-paint blitz-traits blitz-html blitz-shell blitz-kit anyrender image resvg jxl-oxide syntect pulldown-cmark skrifa"
  "anyview-book: dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next blitz-dom blitz-paint blitz-traits blitz-html blitz-shell blitz-kit anyrender image resvg jxl-oxide syntect pulldown-cmark skrifa"
  "anyview-font: dioxus tokio zbus wgpu pdfrum mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next blitz-dom blitz-paint blitz-traits blitz-html blitz-shell blitz-kit anyrender image resvg jxl-oxide syntect pulldown-cmark zip tar sevenz-rust2 flate2 bzip2 ruzstd lzma-rs lzma-rust2"
  "anyview-peek: mpv-wgpu-player rsmpv rsmpv-sys ffmpeg-next ffmpeg-sys-next zbus ashpd cpal alsa alsa-sys"
  "anyview-pdf: dioxus tokio zbus wgpu mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next blitz-dom blitz-paint blitz-traits blitz-html blitz-shell blitz-kit anyrender image resvg jxl-oxide syntect pulldown-cmark rayon"
  "anyview-media: dioxus tokio zbus pdfrum image syntect pulldown-cmark resvg jxl-oxide blitz-dom blitz-paint blitz-traits blitz-html blitz-shell blitz-kit anyrender"
)

# Dependencies a crate may reach only THROUGH another, never name in its own manifest. anyview-peek
# links `wgpu`, pdfrum and tokio because `ds-blitz` does; it must not depend on them itself, so a
# texture is made through `ds-blitz`'s `TextureLayer`, a page through its `PdfFileThumb` cache, and
# nothing here spawns.
DIRECT=(
  "anyview: cpal symphonia zbus ashpd freedesktop-desktop-entry latchkey interprocess wgpu pdfrum pdfrum-edit mpv-wgpu-player rsmpv rsmpv-sys ffmpeg-next ffmpeg-sys-next image anyrender anyrender_vello_hybrid vello_hybrid blitz-dom blitz-paint blitz-html blitz-shell dioxus-native"
  "anyview-peek: wgpu pdfrum pdfrum-anyrender pdfrum-edit tokio mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next anyrender anyrender_vello_hybrid vello_hybrid blitz-dom blitz-paint blitz-html blitz-shell dioxus-native"
  "anyview-ui: pdfrum pdfrum-anyrender pdfrum-edit"
  "anyview-export: wgpu pdfrum pdfrum-anyrender pdfrum-edit image tokio mpv-wgpu-player rsmpv ffmpeg-next ffmpeg-sys-next anyrender anyrender_vello_hybrid vello_hybrid blitz-dom blitz-paint blitz-html blitz-shell dioxus-native"
)

# The most distinct packages (name and version) `cargo tree -p <crate>` may list, normal and build
# dependencies only. The launcher links anyview-peek, so growth here is growth of its binary: raise a
# budget in the change that adds the dependency, with the reason (FINDINGS). anyview-peek is 564 today (591
# before anyview-image's AVIF encoder, ravif with rav1e and their 27 packages, moved behind its `encode` feature,
# which the viewer and its exports turn on and the peek does not):
# about 530 are `ds` and `ds-blitz`, which the launcher already links, and the rest the container codecs
# of anyview-archive, skrifa and the pure-Rust media parsers (symphonia and its format and codec
# crates, mp4parse, matroska-demuxer) and the Photoshop, ICNS and OpenEXR readers of anyview-image
# (psd, icns, exr and its inflate and SIMD helpers; the budgets rose by nine for them), and the
# EPUB package reader of anyview-book (roxmltree, one more), and the spreadsheet and
# office readers of anyview-text and anyview-archive (calamine and quick-xml for XLSX and ODS, with what
# they pull in, five more). The viewer (anyview) is 670: the peek's
# tree and the window, the platform edge and the plugin registry, with no libmpv or libav binding in
# it, and the built-in audio player's sound card: cpal, with alsa and alsa-sys under it and dasp_sample
# (four packages; the decoders are the symphonia crates the peek already links, and libasound is an audio
# device library, not a codec). The missing-tool prompt adds two: ds-shell (the install sheet, drawn by the views) and
# ds-helpers (the catalog and PackageKit install, named by the binary); everything else they use was already in the tree.
# Single instance without D-Bus adds three: latchkey, interprocess (its Unix socket and named pipe
# transport) and doctest-file (interprocess's proc macro), which the portable build needs and the Linux
# desktop build links as well. Hiding what the platform cannot do adds one: ds-desktop (quire's capability
# probe: whether PackageKit answers, so an Install... is offered only where it can work; it has no
# dependency of its own without its `dbus` feature, which `quire-desktop` turns on, and then only the
# zbus already in the tree). anyview-peek is 591 since quire v0.2.22: `ds` itself now depends on `ds-desktop` (the capsule and menus ask which desktop services answer), so the
# launcher links it too (one package, no dependency of its own without `dbus`); it never reaches cpal (its rule above).
# Offering the viewer to docket (the agent layer, `quire-desktop` only) adds twelve to the viewer and nothing to
# the peek: docket-client, docket-core and docket-dbus; porter-core, porter-dbus and prov (docket's names and
# labels); almanac-core, cua-action, model-provider, genai-names and vision-prep (pure vocabulary crates
# docket-core names in its signatures: episodes, computer-use actions, model shapes); and base64 0.22.1 (the
# older one docket-core's wire uses beside the 0.23 already in the tree). The viewer is 682.
# The plugin machinery moved out to bayonet (a library of its own, shared with other apps): the viewer is 683,
# one package more, since bayonet is a package where the same code was three workspace crates' own, and its
# dependencies (toml, rustix, serde, serde_json, thiserror) were all in the tree already.
# quire v0.2.31 builds arboard with `wayland-data-control`, so copied text reaches a Wayland desktop and outlives
# the call that set it: the viewer is 687, four packages more (wl-clipboard-rs, which serves the selection, and its
# petgraph, os_pipe and tree_magic_mini). The pane links ds-blitz too, so anyview-peek is 569: the same four, and
# fixedbitset, which the viewer already had. The headless peek links no ds-blitz and stays 248.
# Both ratchet down when a change drops a dependency and are never raised without the reason.
#
# The headless peek (`--no-default-features`: no `pane`, no `media`) is what a mail client or a terminal
# links to look at a file: 248 packages, with no renderer, no window system and no encoder. It reaches
# none of HEADLESS_FORBIDDEN, and CI builds it on macOS and Windows.
BUDGETS=(
  "anyview-peek: 569"
  "anyview: 687"
)
HEADLESS_BUDGET=248
HEADLESS_FORBIDDEN=(ds-blitz wgpu pdfrum blitz-dom anyrender rav1e ravif img-parts zbus wayland-client)
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

# The headless peek reaches no renderer and no encoder, and stays inside its package budget.
for dep in "${HEADLESS_FORBIDDEN[@]}"; do
  if cargo tree -p anyview-peek --no-default-features -i "$dep" -e normal,build 2>/dev/null | grep -q .; then
    echo "LEAK: anyview-peek --no-default-features depends on $dep"
    fail=1
  fi
done
headless=$(cargo tree -p anyview-peek --no-default-features -e normal,build --prefix none --format '{p}' 2>/dev/null \
  | sed 's/ (\*)$//' | sort -u | grep -c .)
if [ "$headless" -gt "$HEADLESS_BUDGET" ]; then
  echo "BUDGET: anyview-peek --no-default-features has $headless packages, the budget is $HEADLESS_BUDGET"
  fail=1
else
  echo "budget holds: anyview-peek --no-default-features has $headless packages of $HEADLESS_BUDGET"
fi

# No crate of the workspace reaches the bindings of libmpv or libav, and the lockfile does not hold
# them at all, so a dev-dependency cannot bring them back either.
for dir in crates/*/ plugins/*/; do
  crate="$(basename "$dir")"
  if ! cargo tree -p "$crate" --depth 0 >/dev/null 2>&1; then
    echo "ERROR: cargo tree cannot resolve $crate; the codec boundary was not checked"
    fail=1
    continue
  fi
  leaked=0
  for dep in "${FORBIDDEN_EVERYWHERE[@]}"; do
    if cargo tree -p "$crate" -i "$dep" -e normal,build 2>/dev/null | grep -q .; then
      echo "LEAK: $crate depends on $dep: codecs are run as plugins, never linked"
      leaked=1
      fail=1
    fi
  done
  if [ "$leaked" -eq 0 ]; then
    echo "codec boundary holds: $crate reaches none of ${FORBIDDEN_EVERYWHERE[*]}"
  fi
done
for dep in "${FORBIDDEN_EVERYWHERE[@]}"; do
  if grep -qx "name = \"$dep\"" Cargo.lock; then
    echo "LEAK: Cargo.lock holds $dep"
    fail=1
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
  "anyview: anyview-core anyview-export anyview-image anyview-media anyview-pdf anyview-peek anyview-platform anyview-plugin anyview-plugin-protocol anyview-store anyview-ui ds ds-blitz ds-desktop ds-helpers ds-settings"
  "anyview-core: ds-core"
  "anyview-store: anyview-core"
  "anyview-ui: anyview-archive anyview-book anyview-core anyview-image anyview-pdf anyview-text ds ds-blitz ds-core ds-shell"
  "anyview-image: anyview-core ds-core"
  "anyview-text: anyview-core ds-core"
  "anyview-platform: anyview-core anyview-plugin anyview-plugin-protocol bayonet ds-core docket-client docket-core porter-core prov"
  "anyview-peek: anyview-archive anyview-book anyview-core anyview-font anyview-image anyview-text ds ds-blitz"
  "anyview-media: anyview-core ds-core"
  "anyview-archive: anyview-core ds-core"
  "anyview-book: anyview-archive anyview-core ds-core"
  "anyview-font: anyview-core"
  "anyview-pdf: anyview-core"
  "anyview-export: anyview-core anyview-image anyview-pdf anyview-store anyview-text ds-blitz ds-core"
  "anyview-plugin: anyview-core anyview-plugin-protocol bayonet"
  "anyview-plugin-protocol: bayonet"
  "anyview-plugin-fake: anyview-plugin-protocol"
  "anyview-ffmpeg: anyview-plugin-protocol"
  "anyview-heif: anyview-plugin-protocol anyview-tool-kit"
  "anyview-raw: anyview-plugin-protocol anyview-tool-kit"
  "anyview-tool-kit: anyview-plugin-protocol"
)
  # quire, docket, porter and bayonet crates are git dependencies, so they show with their url where a
  # workspace crate shows a path. (bayonet is a path while it sits beside this checkout, and then shows
  # with a path like the workspace's own.)
for edge in "${EDGES[@]}"; do
  crate="${edge%%:*}"
  read -r -a allowed <<<"${edge#*:}"
  found=$(cargo tree -p "$crate" --depth 1 -e normal,build --prefix none --all-features 2>/dev/null \
    | grep -E '\((/|https://github.com/PoHsuanLai/(quire|docket|porter|bayonet))' | awk '{print $1}' | grep -vx "$crate" | sort -u | tr '\n' ' ')
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
for dir in crates/*/ plugins/*/; do
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
# anyview-platform reaches the bus and the freedesktop entry readers only through its `quire-desktop`
# feature (ARCHITECTURE.md section 2e): with it off, none of them is in its tree. This is the
# crate's own tree; quire's `ds-settings` still brings zbus into the binary's until quire's portable
# build lands, and that is not the platform crate's doing.
for dep in zbus ashpd freedesktop-desktop-entry memfd; do
  if cargo tree -p anyview-platform --no-default-features -e normal,build -i "$dep" 2>/dev/null | grep -q .; then
    echo "PORTABLE: anyview-platform reaches $dep without quire-desktop"
    fail=1
  fi
done
echo "portable build holds: anyview-platform --no-default-features reaches none of zbus ashpd freedesktop-desktop-entry memfd"

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
