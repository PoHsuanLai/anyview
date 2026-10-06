#!/usr/bin/env bash
# Install the viewer: the binary, the desktop entry, the D-Bus service file and the icons. Safe to
# run again: a file already in place and identical is left alone.
#
#   dist/install.sh [--dry-run] [--prefix DIR] [--no-build] [--set-default] [--without-plugin NAME]
#                   [--no-plugins] [--with-mpv-from DIR] [--mpv PATH]
#
#   --dry-run      print every action, change nothing (does not build, fetch or use sudo)
#   --prefix DIR   where it goes: DIR/bin, DIR/share/... (default: /usr/local as root, else ~/.local)
#   --no-build     use what is already built (a plugin that is not built is skipped, with a warning)
#   --set-default  also make the viewer the default for images, plain text, Markdown and PDF (xdg-mime default).
#                  Off unless asked: installing never takes a file type from another program.
#   --without-plugin NAME  leave a plugin out (ffmpeg, heif, mpv or raw); may be given more than once.
#   --no-plugins   install the viewer alone.
#
# The plugins install with the viewer, each as a program under DIR/libexec/anyview and a manifest under
# DIR/share/anyview/plugins. Each one runs your own distribution's tool and links no codec, so a plugin
# whose tool is not installed yet is still installed: it says so when it is used, and starts working once
# you install the package (see the README's table).
#   ffmpeg  facts, pictures and conversion of video and audio (ffmpeg, ffprobe)
#   heif    HEIC, HEIF and AVIF pictures (heif-dec or heif-convert)
#   raw     camera raw files developed in full (dcraw_emu or dcraw)
#   mpv     playback, through your own mpv with mpv-wgpu's C plugin loaded into it. The C plugin is built
#           from an mpv-wgpu checkout: --with-mpv-from DIR, MPV_WGPU_DIR, or ../mpv when it exists; else
#           the revision named by MPV_WGPU_REV below is fetched with git into the cache
#           (${XDG_CACHE_HOME:-~/.cache}/anyview/build) and built there. When there is no mpv on the search
#           path, git, the network or the build is missing, the mpv plugin is skipped with a one-line
#           warning and everything else installs; run it again to retry.
#   --with-mpv-from DIR  build the mpv plugin's C plugin from this mpv-wgpu checkout
#   --mpv PATH     the mpv the mpv plugin's manifest names (default: the first `mpv` on the search path)
#   --with-plugin NAME  accepted and ignored: plugins install by default now
#
# Environment:
#   DESTDIR        stage under this directory instead of the real root (nothing is registered)
#   ANYVIEW_BIN    the binary to install (default: release build under $CARGO_TARGET_DIR or target/)
#   ANYVIEW_FFMPEG_PLUGIN_BIN  the FFmpeg plugin's program to install (same default place, anyview-ffmpeg)
#   ANYVIEW_HEIF_PLUGIN_BIN, ANYVIEW_RAW_PLUGIN_BIN  the same for the HEIF and RAW plugins
#   MPV_WGPU_DIR   an mpv-wgpu checkout to build the mpv plugin's C plugin from (default: ../mpv, if there)
#   MPV_WGPU_URL   where the mpv-wgpu revision is fetched from (default: its GitHub repository)
#   ANYVIEW_MPV_CPLUGIN  the C plugin (libmpv_wgpu_cplugin.so) to install instead of building it
#   ICON_DIR       a folder of <px>.png icons (default: assets/icons in this repository)

# The mpv-wgpu revision fetched when no checkout is given: the one the viewer is tested against, kept in
# step with the desktop manifest's pin. Change it here and nowhere else.
MPV_WGPU_REV=7aba35b3c75786f0b02a356f1d34d1ce888d1f48
MPV_WGPU_URL="${MPV_WGPU_URL:-https://github.com/PoHsuanLai/mpv-wgpu.git}"
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=dist/lib.sh
source "$HERE/dist/lib.sh"

PREFIX=""
BUILD=yes
SET_DEFAULT=no
WITH_FFMPEG=yes
WITH_MPV=yes
WITH_HEIF=yes
WITH_RAW=yes
MPV_PATH=""
MPV_FROM=""
plugin_name() { # plugin_name NAME: succeed for a plugin's name
  case "$1" in ffmpeg|heif|mpv|raw) return 0 ;; *) return 1 ;; esac
}
while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) DRY_RUN=yes ;;
    --prefix) PREFIX="${2:?--prefix needs a directory}"; shift ;;
    --prefix=*) PREFIX="${1#--prefix=}" ;;
    --no-build) BUILD=no ;;
    --set-default) SET_DEFAULT=yes ;;
    --without-plugin) plugin="${2:?--without-plugin needs a name}"; shift
      case "$plugin" in
        ffmpeg) WITH_FFMPEG=no ;;
        mpv) WITH_MPV=no ;;
        heif) WITH_HEIF=no ;;
        raw) WITH_RAW=no ;;
        *) say "install.sh: unknown plugin $plugin (the plugins are: ffmpeg, heif, mpv, raw)"; exit 2 ;;
      esac ;;
    --no-plugins) WITH_FFMPEG=no WITH_MPV=no WITH_HEIF=no WITH_RAW=no ;;
    --with-plugin) plugin="${2:?--with-plugin needs a name}"; shift
      plugin_name "$plugin" || { say "install.sh: unknown plugin $plugin (the plugins are: ffmpeg, heif, mpv, raw)"; exit 2; }
      say "note: --with-plugin is not needed any more: plugins install by default (--without-plugin $plugin leaves one out)" ;;
    --with-mpv-from) MPV_FROM="${2:?--with-mpv-from needs a directory}"; shift ;;
    --with-mpv-from=*) MPV_FROM="${1#--with-mpv-from=}" ;;
    --mpv) MPV_PATH="${2:?--mpv needs a path}"; shift ;;
    --mpv=*) MPV_PATH="${1#--mpv=}" ;;
    -h|--help) sed -n '2,/^$/{/^$/!p}' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) say "install.sh: unknown argument $1 (try --help)"; exit 2 ;;
  esac
  shift
done

[[ -n "$PREFIX" ]] || PREFIX="$(default_prefix)"
PREFIX="${PREFIX%/}"
[[ "$PREFIX" == /* ]] || { say "install.sh: --prefix must be an absolute path"; exit 2; }
choose_sudo "$PREFIX"

TARGET="${CARGO_TARGET_DIR:-$HERE/target}"
BIN="${ANYVIEW_BIN:-$TARGET/release/anyview}"
FFMPEG_BIN="${ANYVIEW_FFMPEG_PLUGIN_BIN:-$TARGET/release/anyview-ffmpeg}"
HEIF_BIN="${ANYVIEW_HEIF_PLUGIN_BIN:-$TARGET/release/anyview-heif}"
RAW_BIN="${ANYVIEW_RAW_PLUGIN_BIN:-$TARGET/release/anyview-raw}"
ICONS="${ICON_DIR:-$HERE/assets/icons}"
say "anyview install to $PREFIX$([[ -n "$DESTDIR" ]] && echo " (staged in $DESTDIR)") ($([[ $DRY_RUN == yes ]] && echo 'dry run: nothing is changed' || echo 'for real'))"

say "1. build"
# The viewer and the three Rust plugins build in one cargo run; one that was named (ANYVIEW_*_BIN) or
# that --no-build leaves out is used as it is.
PACKAGES=()
if [[ "$BUILD" == no || -n "${ANYVIEW_BIN:-}" ]]; then
  say "  skipped: using $BIN"
else
  PACKAGES+=(-p anyview)
fi
# plugin_program FLAG NAMED PACKAGE BIN LABEL ENVVAR: queue a plugin's build; the flag is cleared (the
# plugin skipped, with a warning) when --no-build left it with no program.
plugin_program() {
  local flag="$1" named="$2" package="$3" bin="$4" label="$5" envvar="$6"
  [[ "${!flag}" == yes ]] || return 0
  if [[ "$BUILD" == no || -n "$named" ]]; then
    say "  skipped: using $bin for the $label plugin"
    if [[ "$DRY_RUN" == no && ! -x "$bin" ]]; then
      if [[ -z "$named" ]]; then
        warn "the $label plugin was skipped: no program at $bin (build it with cargo build --release -p $package)"
        printf -v "$flag" no
      else
        say "install.sh: no $label plugin at $bin (build it, or set $envvar)"
        exit 1
      fi
    fi
  else
    PACKAGES+=(-p "$package")
  fi
}
plugin_program WITH_FFMPEG "${ANYVIEW_FFMPEG_PLUGIN_BIN:-}" anyview-ffmpeg "$FFMPEG_BIN" FFmpeg ANYVIEW_FFMPEG_PLUGIN_BIN
plugin_program WITH_HEIF "${ANYVIEW_HEIF_PLUGIN_BIN:-}" anyview-heif "$HEIF_BIN" HEIF ANYVIEW_HEIF_PLUGIN_BIN
plugin_program WITH_RAW "${ANYVIEW_RAW_PLUGIN_BIN:-}" anyview-raw "$RAW_BIN" RAW ANYVIEW_RAW_PLUGIN_BIN
if [[ ${#PACKAGES[@]} -gt 0 ]]; then
  if [[ "$DRY_RUN" == yes ]]; then
    say "  would run: cargo build --release ${PACKAGES[*]} (in $HERE)"
  else
    (cd "$HERE" && cargo build --release "${PACKAGES[@]}")
  fi
fi
if [[ "$DRY_RUN" == no && ! -x "$BIN" ]]; then
  say "install.sh: no binary at $BIN (build it, or set ANYVIEW_BIN)"
  exit 1
fi
if [[ "$DRY_RUN" == no ]]; then
  for pair in "$WITH_FFMPEG:$FFMPEG_BIN:FFmpeg" "$WITH_HEIF:$HEIF_BIN:HEIF" "$WITH_RAW:$RAW_BIN:RAW"; do
    IFS=: read -r on bin label <<<"$pair"
    if [[ "$on" == yes && ! -x "$bin" ]]; then
      say "install.sh: the build left no $label plugin at $bin"
      exit 1
    fi
  done
fi

# The mpv plugin. Its pieces are looked for in turn, and the plugin is skipped, with one warning, when
# a piece is missing: the rest of the install never depends on it.
MPV_SKIP_NOTE="the mpv plugin was skipped; install the missing piece and run install.sh again, or name a checkout with --with-mpv-from DIR"
skip_mpv() { warn "$1: $MPV_SKIP_NOTE"; WITH_MPV=no; }

# cplugin_in TARGET: where a build under TARGET leaves the C plugin.
cplugin_in() { printf '%s/release/libmpv_wgpu_cplugin.so' "$1"; }

# build_cplugin SOURCE TARGET: build the C plugin from a checkout, into its own target directory.
build_cplugin() {
  local source="$1" target="$2"
  if [[ "$DRY_RUN" == yes ]]; then
    say "  would run: cargo build --release -p mpv-wgpu-cplugin (in $source, into $target)"
    return 0
  fi
  (cd "$source" && CARGO_TARGET_DIR="$target" cargo build --release -p mpv-wgpu-cplugin)
}

# fetch_mpv_wgpu DIR: a checkout of the pinned revision in DIR, fetched by that exact revision (a
# shallow fetch). One that is already there at the revision is reused.
fetch_mpv_wgpu() {
  local dir="$1"
  if [[ "$(git -C "$dir" rev-parse HEAD 2>/dev/null || true)" == "$MPV_WGPU_REV" ]]; then
    say "  reusing the mpv-wgpu checkout in $dir"
    return 0
  fi
  command -v git >/dev/null 2>&1 || return 1
  rm -rf "$dir"
  mkdir -p "$dir" \
    && git -C "$dir" init -q \
    && GIT_TERMINAL_PROMPT=0 git -C "$dir" fetch -q --depth 1 "$MPV_WGPU_URL" "$MPV_WGPU_REV" \
    && git -C "$dir" -c advice.detachedHead=false checkout -q FETCH_HEAD \
    && [[ "$(git -C "$dir" rev-parse HEAD)" == "$MPV_WGPU_REV" ]] || { rm -rf "$dir"; return 1; }
}

MPV_CPLUGIN=""
if [[ "$WITH_MPV" == yes ]]; then
  say "  the mpv plugin"
  # The mpv the manifest names is looked up now, on the search path, so the viewer never searches
  # the person's PATH when it runs; --mpv names another.
  if [[ -z "$MPV_PATH" ]]; then
    MPV_PATH="$(command -v mpv || true)"
    if [[ -z "$MPV_PATH" && "$DRY_RUN" == yes ]]; then
      MPV_PATH="/usr/bin/mpv"
      say "    no mpv on the search path (a real run would skip the mpv plugin; install mpv or use --mpv PATH)"
    elif [[ -z "$MPV_PATH" ]]; then
      skip_mpv "no mpv on the search path"
    fi
  fi
fi
if [[ "$WITH_MPV" == yes ]]; then
  [[ "$MPV_PATH" == /* ]] || MPV_PATH="$(cd "$(dirname "$MPV_PATH")" 2>/dev/null && pwd)/$(basename "$MPV_PATH")"
  if [[ "$DRY_RUN" == no && ! -x "$MPV_PATH" ]]; then
    say "install.sh: $MPV_PATH is not an executable mpv (use --mpv PATH)"
    exit 1
  fi
  checkout="${MPV_FROM:-${MPV_WGPU_DIR:-}}"
  # The sibling checkout is the developer's own (the test sets ANYVIEW_MPV_SIBLING empty to ignore it).
  sibling="${ANYVIEW_MPV_SIBLING-$HERE/../mpv}"
  [[ -n "$checkout" || -z "$sibling" || ! -d "$sibling" ]] || checkout="$sibling"
  if [[ -n "${ANYVIEW_MPV_CPLUGIN:-}" ]]; then
    MPV_CPLUGIN="$ANYVIEW_MPV_CPLUGIN"
    say "    skipped: using $MPV_CPLUGIN for the mpv plugin"
  elif [[ -n "$checkout" ]]; then
    if [[ ! -d "$checkout" ]]; then
      skip_mpv "no mpv-wgpu checkout at $checkout"
    else
      target="${MPV_WGPU_TARGET_DIR:-$checkout/target}"
      MPV_CPLUGIN="$(cplugin_in "$target")"
      if [[ "$BUILD" == no ]]; then
        say "    skipped: using $MPV_CPLUGIN for the mpv plugin"
      else
        build_cplugin "$checkout" "$target" || skip_mpv "building the mpv plugin from $checkout failed"
      fi
    fi
  else
    dir="${XDG_CACHE_HOME:-${HOME:?HOME is not set}/.cache}/anyview/build/mpv-wgpu-${MPV_WGPU_REV:0:12}"
    MPV_CPLUGIN="$(cplugin_in "$dir/target")"
    if [[ "$BUILD" == no ]]; then
      say "    skipped: using $MPV_CPLUGIN for the mpv plugin"
    elif [[ "$DRY_RUN" == yes ]]; then
      say "    would run: git fetch --depth 1 $MPV_WGPU_URL $MPV_WGPU_REV (into $dir)"
      build_cplugin "$dir" "$dir/target"
    elif ! fetch_mpv_wgpu "$dir"; then
      skip_mpv "could not fetch mpv-wgpu from $MPV_WGPU_URL"
    else
      build_cplugin "$dir" "$dir/target" || skip_mpv "building the mpv plugin failed"
    fi
  fi
  if [[ "$WITH_MPV" == yes && "$DRY_RUN" == no && ! -f "$MPV_CPLUGIN" ]]; then
    skip_mpv "no mpv-wgpu C plugin at $MPV_CPLUGIN"
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
  # Written below, so this run owns it (an identical file already there is not recorded).
  note_install_dirs "$(dirname "$dst")"
  note_install_file "$dst"
  step "install $dst" privileged install -Dm"$mode" "$src" "$real"
}

say "2. files"
install_file 755 "$BIN" "$PREFIX/bin/anyview"
install_file 644 "$HERE/dist/$APP_ID.desktop" "$PREFIX/share/applications/$APP_ID.desktop"
install_file 644 "$HERE/dist/$APP_ID.metainfo.xml" "$PREFIX/share/metainfo/$APP_ID.metainfo.xml"
# The licences: the viewer's own two and the notices for the crates built into it.
for doc in LICENSE-MIT LICENSE-APACHE THIRD-PARTY-NOTICES.md; do
  install_file 644 "$HERE/$doc" "$PREFIX/share/doc/anyview/$doc"
done

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
# runs). The program finds ffmpeg and ffprobe itself when it runs, so none is needed here; without
# them it greets the viewer with nothing on offer, and the plugin works once they are installed.
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
    || warn "ffmpeg and ffprobe are not on the search path: the ffmpeg plugin is installed and starts working once your distribution's ffmpeg package is (README, Plugins)"
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
  warn "none of $* is on the search path: the $id plugin is installed and starts working once your distribution's package for it is (README, Plugins)"
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
sizes="$(icon_sizes "$ICONS")"
for px in $sizes; do
  install_file 644 "$ICONS/$px.png" \
    "$PREFIX/share/icons/hicolor/${px}x${px}/apps/$APP_ID.png"
done

say "3b. receipt"
write_receipt "$PREFIX"

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
