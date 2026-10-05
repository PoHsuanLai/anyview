#!/usr/bin/env bash
# Install the viewer: the binary, the desktop entry, the D-Bus service file and the icons. Safe to
# run again: a file already in place and identical is left alone.
#
#   dist/install.sh [--dry-run] [--prefix DIR] [--no-build] [--set-default] [--with-plugin NAME] [--mpv PATH]
#
#   --dry-run      print every action, change nothing (does not build, does not use sudo)
#   --prefix DIR   where it goes: DIR/bin, DIR/share/... (default: /usr/local as root, else ~/.local)
#   --no-build     use the binary that is already built
#   --set-default  also make the viewer the default for images, plain text, Markdown and PDF (xdg-mime default).
#                  Off unless asked: installing never takes a file type from another program.
#   --with-plugin NAME  also install a plugin (ffmpeg: facts, pictures and exports of video and audio, through the
#                  person's own ffprobe and ffmpeg): its program under DIR/libexec/anyview and its manifest under
#                  DIR/share/anyview/plugins. Off unless asked; may be given once for each plugin.
#                  (mpv: playback, through the person's own mpv run as a child process with mpv-wgpu's C
#                  plugin loaded into it: the plugin is built from an mpv-wgpu checkout and installed as
#                  DIR/libexec/anyview/mpv-wgpu-cplugin.so, and the manifest names the mpv found on the
#                  search path.) Without these plugins the viewer shows recordings as facts only.
#                  (heif: HEIC, HEIF and AVIF pictures, through the person's own libheif tools; raw: camera
#                  raw files developed in full, through the person's own LibRaw dcraw_emu or dcraw.)
#   --mpv PATH     the mpv the mpv plugin's manifest names (default: the first `mpv` on the search path)
#
# Environment:
#   DESTDIR        stage under this directory instead of the real root (nothing is registered)
#   ANYVIEW_BIN    the binary to install (default: release build under $CARGO_TARGET_DIR or target/)
#   ANYVIEW_FFMPEG_PLUGIN_BIN  the FFmpeg plugin's program to install (same default place, anyview-ffmpeg)
#   ANYVIEW_HEIF_PLUGIN_BIN, ANYVIEW_RAW_PLUGIN_BIN  the same for the HEIF and RAW plugins
#   MPV_WGPU_DIR   an mpv-wgpu checkout to build the mpv plugin's C plugin from (default: ../mpv)
#   ANYVIEW_MPV_CPLUGIN  the C plugin (libmpv_wgpu_cplugin.so) to install instead of building it
#   QUIRE_DIR      a quire checkout with assets/icons/apps/viewer/<px>.png (default: ../quire)
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=dist/lib.sh
source "$HERE/dist/lib.sh"

PREFIX=""
BUILD=yes
SET_DEFAULT=no
WITH_FFMPEG=no
WITH_MPV=no
WITH_HEIF=no
WITH_RAW=no
MPV_PATH=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) DRY_RUN=yes ;;
    --prefix) PREFIX="${2:?--prefix needs a directory}"; shift ;;
    --prefix=*) PREFIX="${1#--prefix=}" ;;
    --no-build) BUILD=no ;;
    --set-default) SET_DEFAULT=yes ;;
    --with-plugin) plugin="${2:?--with-plugin needs a name}"; shift
      case "$plugin" in
        ffmpeg) WITH_FFMPEG=yes ;;
        mpv) WITH_MPV=yes ;;
        heif) WITH_HEIF=yes ;;
        raw) WITH_RAW=yes ;;
        *) say "install.sh: unknown plugin $plugin (the plugins are: ffmpeg, heif, mpv, raw)"; exit 2 ;;
      esac ;;
    --mpv) MPV_PATH="${2:?--mpv needs a path}"; shift ;;
    --mpv=*) MPV_PATH="${1#--mpv=}" ;;
    -h|--help) sed -n '2,30p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) say "install.sh: unknown argument $1 (try --help)"; exit 2 ;;
  esac
  shift
done

[[ -n "$PREFIX" ]] || PREFIX="$(default_prefix)"
PREFIX="${PREFIX%/}"
[[ "$PREFIX" == /* ]] || { say "install.sh: --prefix must be an absolute path"; exit 2; }
choose_sudo "$PREFIX"

BIN="${ANYVIEW_BIN:-${CARGO_TARGET_DIR:-$HERE/target}/release/anyview}"
FFMPEG_BIN="${ANYVIEW_FFMPEG_PLUGIN_BIN:-${CARGO_TARGET_DIR:-$HERE/target}/release/anyview-ffmpeg}"
HEIF_BIN="${ANYVIEW_HEIF_PLUGIN_BIN:-${CARGO_TARGET_DIR:-$HERE/target}/release/anyview-heif}"
RAW_BIN="${ANYVIEW_RAW_PLUGIN_BIN:-${CARGO_TARGET_DIR:-$HERE/target}/release/anyview-raw}"
MPV_WGPU="${MPV_WGPU_DIR:-$HERE/../mpv}"
MPV_CPLUGIN="${ANYVIEW_MPV_CPLUGIN:-${MPV_WGPU_TARGET_DIR:-$MPV_WGPU/target}/release/libmpv_wgpu_cplugin.so}"
QUIRE="${QUIRE_DIR:-$HERE/../quire}"
say "anyview install to $PREFIX$([[ -n "$DESTDIR" ]] && echo " (staged in $DESTDIR)") ($([[ $DRY_RUN == yes ]] && echo 'dry run: nothing is changed' || echo 'for real'))"

say "1. build"
if [[ "$BUILD" == no || -n "${ANYVIEW_BIN:-}" ]]; then
  say "  skipped: using $BIN"
elif [[ "$DRY_RUN" == yes ]]; then
  say "  would run: cargo build --release -p anyview (in $HERE)"
else
  (cd "$HERE" && cargo build --release -p anyview)
fi
if [[ "$DRY_RUN" == no && ! -x "$BIN" ]]; then
  say "install.sh: no binary at $BIN (build it, or set ANYVIEW_BIN)"
  exit 1
fi
if [[ "$WITH_FFMPEG" == yes ]]; then
  if [[ "$BUILD" == no || -n "${ANYVIEW_FFMPEG_PLUGIN_BIN:-}" ]]; then
    say "  skipped: using $FFMPEG_BIN for the FFmpeg plugin"
  elif [[ "$DRY_RUN" == yes ]]; then
    say "  would run: cargo build --release -p anyview-ffmpeg (in $HERE)"
  else
    (cd "$HERE" && cargo build --release -p anyview-ffmpeg)
  fi
  if [[ "$DRY_RUN" == no && ! -x "$FFMPEG_BIN" ]]; then
    say "install.sh: no FFmpeg plugin at $FFMPEG_BIN (build it, or set ANYVIEW_FFMPEG_PLUGIN_BIN)"
    exit 1
  fi
fi

# build_plugin FLAG ENVVAR PACKAGE BIN LABEL: build a plugin's program unless it was named or --no-build.
build_plugin() {
  local named="$1" package="$2" bin="$3" label="$4" envvar="$5"
  if [[ "$BUILD" == no || -n "$named" ]]; then
    say "  skipped: using $bin for the $label plugin"
  elif [[ "$DRY_RUN" == yes ]]; then
    say "  would run: cargo build --release -p $package (in $HERE)"
  else
    (cd "$HERE" && cargo build --release -p "$package")
  fi
  if [[ "$DRY_RUN" == no && ! -x "$bin" ]]; then
    say "install.sh: no $label plugin at $bin (build it, or set $envvar)"
    exit 1
  fi
}
[[ "$WITH_HEIF" == no ]] || build_plugin "${ANYVIEW_HEIF_PLUGIN_BIN:-}" anyview-heif "$HEIF_BIN" HEIF ANYVIEW_HEIF_PLUGIN_BIN
[[ "$WITH_RAW" == no ]] || build_plugin "${ANYVIEW_RAW_PLUGIN_BIN:-}" anyview-raw "$RAW_BIN" RAW ANYVIEW_RAW_PLUGIN_BIN

if [[ "$WITH_MPV" == yes ]]; then
  # The mpv the manifest names is looked up now, on the search path, so the viewer never searches
  # the person's PATH when it runs; --mpv names another.
  if [[ -z "$MPV_PATH" ]]; then
    MPV_PATH="$(command -v mpv || true)"
  fi
  if [[ -z "$MPV_PATH" ]]; then
    if [[ "$DRY_RUN" == yes ]]; then
      MPV_PATH="/usr/bin/mpv"
      say "  no mpv on the search path (a real run would stop here; use --mpv PATH or install mpv)"
    else
      say "install.sh: no mpv on the search path (install mpv, or name one with --mpv PATH)"
      exit 1
    fi
  fi
  [[ "$MPV_PATH" == /* ]] || MPV_PATH="$(cd "$(dirname "$MPV_PATH")" 2>/dev/null && pwd)/$(basename "$MPV_PATH")"
  if [[ "$DRY_RUN" == no && ! -x "$MPV_PATH" ]]; then
    say "install.sh: $MPV_PATH is not an executable mpv (use --mpv PATH)"
    exit 1
  fi
  if [[ "$BUILD" == no || -n "${ANYVIEW_MPV_CPLUGIN:-}" ]]; then
    say "  skipped: using $MPV_CPLUGIN for the mpv plugin"
  elif [[ "$DRY_RUN" == yes ]]; then
    say "  would run: cargo build --release -p mpv-wgpu-cplugin (in $MPV_WGPU)"
  else
    (cd "$MPV_WGPU" && cargo build --release -p mpv-wgpu-cplugin)
  fi
  if [[ "$DRY_RUN" == no && ! -f "$MPV_CPLUGIN" ]]; then
    say "install.sh: no mpv-wgpu C plugin at $MPV_CPLUGIN (set MPV_WGPU_DIR to a checkout, or ANYVIEW_MPV_CPLUGIN)"
    exit 1
  fi
fi

# install_file MODE SRC DST: copy unless an identical file is there already.
install_file() {
  local mode="$1" src="$2" dst="$3" real
  real="$(dest "$dst")"
  if [[ -f "$real" && "$DRY_RUN" == no ]] && cmp -s "$src" "$real" \
    && [[ "$(stat -c %a "$real")" == "${mode#0}" ]]; then
    say "  unchanged: $dst"
    return 0
  fi
  step "install $dst" privileged install -Dm"$mode" "$src" "$real"
}

say "2. files"
install_file 755 "$BIN" "$PREFIX/bin/anyview"
install_file 644 "$HERE/dist/$APP_ID.desktop" "$PREFIX/share/applications/$APP_ID.desktop"

# The service file names the binary's path on this machine, so its Exec is rewritten to the
# installed one (never to a path inside DESTDIR: that is where it lands, not where it will run).
service="$PREFIX/share/dbus-1/services/$BUS_NAME.service"
if [[ "$DRY_RUN" == yes ]]; then
  say "  install $service"
  say "      (Exec=$PREFIX/bin/anyview, from dist/$BUS_NAME.service)"
else
  rendered="$(mktemp)"
  trap 'rm -f "$rendered"' EXIT
  sed "s|^Exec=.*|Exec=$PREFIX/bin/anyview|" "$HERE/dist/$BUS_NAME.service" >"$rendered"
  install_file 644 "$rendered" "$service"
fi

# A plugin is a program under libexec and a manifest next to the others: the manifest names the
# program by its installed path (never one inside DESTDIR, which is where it lands, not where it
# runs). The program finds ffmpeg and ffprobe itself when it runs, so none is needed here.
if [[ "$WITH_FFMPEG" == yes ]]; then
  say "2b. the FFmpeg plugin"
  install_file 755 "$FFMPEG_BIN" "$PREFIX/libexec/anyview/anyview-ffmpeg"
  manifest="$PREFIX/share/anyview/plugins/ffmpeg.toml"
  if [[ "$DRY_RUN" == yes ]]; then
    say "  install $manifest"
    say "      (path = $PREFIX/libexec/anyview/anyview-ffmpeg, from dist/plugins/anyview-ffmpeg.toml.in)"
  else
    rendered_plugin="$(mktemp)"
    trap 'rm -f "$rendered" "${rendered_plugin:-}"' EXIT
    sed "s|@PREFIX@|$PREFIX|g" "$HERE/dist/plugins/anyview-ffmpeg.toml.in" >"$rendered_plugin"
    install_file 644 "$rendered_plugin" "$manifest"
  fi
  command -v ffmpeg >/dev/null 2>&1 && command -v ffprobe >/dev/null 2>&1 \
    || warn "ffmpeg and ffprobe are not on the search path: the plugin offers nothing until they are installed"
fi

# The picture plugins, like the FFmpeg one: a program under libexec and a manifest. Each finds its
# own tools when it runs; a warning says when they are not there yet.
# install_picture_plugin ID LABEL BIN TOOLS...: TOOLS are alternatives, one of which should exist.
install_picture_plugin() {
  local id="$1" label="$2" bin="$3"; shift 3
  say "2d. the $label plugin"
  install_file 755 "$bin" "$PREFIX/libexec/anyview/anyview-$id"
  local manifest="$PREFIX/share/anyview/plugins/$id.toml"
  if [[ "$DRY_RUN" == yes ]]; then
    say "  install $manifest"
    say "      (path = $PREFIX/libexec/anyview/anyview-$id, from dist/plugins/anyview-$id.toml.in)"
  else
    local rendered_pic
    rendered_pic="$(mktemp)"
    sed "s|@PREFIX@|$PREFIX|g" "$HERE/dist/plugins/anyview-$id.toml.in" >"$rendered_pic"
    install_file 644 "$rendered_pic" "$manifest"
    rm -f "$rendered_pic"
  fi
  local tool
  for tool in "$@"; do command -v "$tool" >/dev/null 2>&1 && return 0; done
  warn "none of $* is on the search path: the $label plugin offers nothing until one is installed"
}
[[ "$WITH_HEIF" == no ]] || install_picture_plugin heif HEIF "$HEIF_BIN" heif-dec heif-convert
[[ "$WITH_RAW" == no ]] || install_picture_plugin raw RAW "$RAW_BIN" dcraw_emu dcraw

# The mpv plugin has no program of its own: its manifest names the person's mpv and the C plugin
# installed beside the other plugin programs.
if [[ "$WITH_MPV" == yes ]]; then
  say "2c. the mpv plugin"
  install_file 755 "$MPV_CPLUGIN" "$PREFIX/libexec/anyview/mpv-wgpu-cplugin.so"
  manifest="$PREFIX/share/anyview/plugins/mpv.toml"
  if [[ "$DRY_RUN" == yes ]]; then
    say "  install $manifest"
    say "      (mpv = $MPV_PATH, cplugin = $PREFIX/libexec/anyview/mpv-wgpu-cplugin.so, from dist/plugins/anyview-mpv.toml.in)"
  else
    rendered_mpv="$(mktemp)"
    trap 'rm -f "$rendered" "${rendered_plugin:-}" "$rendered_mpv"' EXIT
    sed -e "s|@PREFIX@|$PREFIX|g" -e "s|@MPV@|$MPV_PATH|g" "$HERE/dist/plugins/anyview-mpv.toml.in" >"$rendered_mpv"
    install_file 644 "$rendered_mpv" "$manifest"
  fi
fi

say "3. icons"
sizes="$(icon_sizes "$QUIRE")"
for px in $sizes; do
  install_file 644 "$QUIRE/assets/icons/apps/viewer/$px.png" \
    "$PREFIX/share/icons/hicolor/${px}x${px}/apps/$APP_ID.png"
done

say "4. desktop databases"
if [[ -n "$DESTDIR" ]]; then
  say "  skipped: staging into $DESTDIR registers nothing"
else
  if command -v update-desktop-database >/dev/null 2>&1; then
    step "refresh the desktop entry cache" privileged update-desktop-database "$PREFIX/share/applications"
  else
    say "  update-desktop-database not found: skipped"
  fi
  if [[ -n "$sizes" ]] && command -v gtk-update-icon-cache >/dev/null 2>&1; then
    step "refresh the icon cache" privileged gtk-update-icon-cache -q -t -f "$PREFIX/share/icons/hicolor"
  else
    say "  gtk-update-icon-cache not found or no icons: skipped"
  fi
fi

say "5. default application"
if [[ "$SET_DEFAULT" == no ]]; then
  say "  not asked (--set-default): other programs keep their file types"
else
  # Images, plain text, Markdown and PDF (never HTML, tables, audio or video), taken from the
  # entry's own MimeType line so the list cannot drift from it.
  types="$(sed -n 's/^MimeType=//p' "$HERE/dist/$APP_ID.desktop" | tr ';' '\n' \
    | grep -E '^(image/|text/plain$|text/markdown$|application/pdf$)' || true)"
  for type in $types; do
    if [[ -n "$DESTDIR" ]]; then
      say "  skipped under DESTDIR: xdg-mime default $APP_ID.desktop $type"
    elif command -v xdg-mime >/dev/null 2>&1; then
      step "open $type with the viewer" xdg-mime default "$APP_ID.desktop" "$type"
    else
      say "  xdg-mime not found: $type left alone"
    fi
  done
fi
say "done."
