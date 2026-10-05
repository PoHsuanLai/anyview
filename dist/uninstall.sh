#!/usr/bin/env bash
# Remove what install.sh wrote, and nothing else; safe to run again.
#
#   dist/uninstall.sh [--dry-run] [--prefix DIR]
#
#   --dry-run     print every action, change nothing
#   --prefix DIR  the prefix it was installed under (default: /usr/local as root, else ~/.local)
#
# It removes what install.sh recorded in its receipt (<prefix>/share/anyview/install-receipt): the
# files it wrote, and the directories it made, only when they are empty. Nothing else is touched.
# Without a receipt (an install from before there was one) it removes the files install.sh writes
# by their names, the plugins' programs and manifests included, and the folders it left empty.
#
# DESTDIR names the staging root it was installed into. A default set with install.sh
# --set-default is a line in the person's mimeapps.list; it is left there (a default that names
# a missing entry is ignored by the desktop).
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=dist/lib.sh
source "$HERE/dist/lib.sh"

PREFIX=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) DRY_RUN=yes ;;
    --prefix) PREFIX="${2:?--prefix needs a directory}"; shift ;;
    --prefix=*) PREFIX="${1#--prefix=}" ;;
    -h|--help) sed -n '2,14p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) say "uninstall.sh: unknown argument $1 (try --help)"; exit 2 ;;
  esac
  shift
done

[[ -n "$PREFIX" ]] || PREFIX="$(default_prefix)"
PREFIX="${PREFIX%/}"
[[ "$PREFIX" == /* ]] || { say "uninstall.sh: --prefix must be an absolute path"; exit 2; }
choose_sudo "$PREFIX"
say "anyview uninstall from $PREFIX$([[ -n "$DESTDIR" ]] && echo " (staged in $DESTDIR)") ($([[ $DRY_RUN == yes ]] && echo 'dry run: nothing is changed' || echo 'for real'))"

# remove_file DST: remove when present.
remove_file() {
  local real
  real="$(dest "$1")"
  if [[ -e "$real" ]]; then
    step "remove $1" privileged rm -f "$real"
    CHANGED=yes
  else
    say "  absent: $1"
  fi
}
CHANGED=no

# remove_recorded RECEIPT: the files and then the empty directories the receipt names, each only
# when it lies under the prefix.
remove_recorded() {
  local receipt="$1" line path files=() dirs=() i
  while IFS= read -r line; do
    case "$line" in
      "file "*) path="${line#file }"; kind=file ;;
      "dir "*) path="${line#dir }"; kind=dir ;;
      *) continue ;;
    esac
    # A file lies under the prefix; a directory also may be the prefix or one above it, which
    # install made when the prefix itself was new.
    if [[ "$path" == *"/../"* || "$path" == */.. ]]; then
      warn "the receipt names $path: left alone"
      continue
    fi
    if [[ "$path" != "$PREFIX"/* && ! ( "$kind" == dir && ( "$path" == "$PREFIX" || "$PREFIX" == "$path"/* ) ) ]]; then
      warn "the receipt names $path, which is not under $PREFIX: left alone"
      continue
    fi
    if [[ "$kind" == file ]]; then files+=("$path"); else dirs+=("$path"); fi
  done <"$(dest "$receipt")"
  for path in ${files[@]+"${files[@]}"}; do remove_file "$path"; done
  if [[ "$DRY_RUN" == no ]]; then
    for ((i = ${#dirs[@]} - 1; i >= 0; i--)); do
      [[ -d "$(dest "${dirs[i]}")" ]] && privileged rmdir --ignore-fail-on-non-empty "$(dest "${dirs[i]}")"
    done
  fi
  return 0
}

RECEIPT="$PREFIX/$RECEIPT_REL"
if [[ -f "$(dest "$RECEIPT")" ]]; then
  say "1. files (from the install receipt)"
  remove_recorded "$RECEIPT"
else
  say "1. files (no install receipt: by name)"
  remove_file "$PREFIX/bin/anyview"
  remove_file "$PREFIX/share/applications/$APP_ID.desktop"
  remove_file "$PREFIX/share/dbus-1/services/$BUS_NAME.service"
  # The plugins install.sh --with-plugin put in: their programs and manifests, and the folders that
  # held them when nothing else is in them.
  remove_file "$PREFIX/libexec/anyview/anyview-ffmpeg"
  remove_file "$PREFIX/share/anyview/plugins/ffmpeg.toml"
  remove_file "$PREFIX/libexec/anyview/anyview-heif"
  remove_file "$PREFIX/share/anyview/plugins/heif.toml"
  remove_file "$PREFIX/libexec/anyview/anyview-raw"
  remove_file "$PREFIX/share/anyview/plugins/raw.toml"
  remove_file "$PREFIX/libexec/anyview/mpv-wgpu-cplugin.so"
  remove_file "$PREFIX/share/anyview/plugins/mpv.toml"
  if [[ "$DRY_RUN" == no ]]; then
    for folder in libexec/anyview share/anyview/plugins share/anyview; do
      [[ -d "$(dest "$PREFIX/$folder")" ]] && privileged rmdir --ignore-fail-on-non-empty "$(dest "$PREFIX/$folder")"
    done
  fi
  # The sizes install wrote are the folders that hold the viewer's icon; a size another program
  # also fills keeps its folder.
  icons_left=no
  for icon in "$(dest "$PREFIX")"/share/icons/hicolor/*/apps/"$APP_ID.png"; do
    [[ -e "$icon" ]] || continue
    remove_file "${icon#"$DESTDIR"}"
    icons_left=yes
    if [[ "$DRY_RUN" == no ]]; then
      apps="$(dirname "$icon")"
      privileged rmdir --ignore-fail-on-non-empty "$apps" "$(dirname "$apps")"
    fi
  done
  [[ "$icons_left" == yes ]] || say "  absent: the icons"
fi

say "2. desktop databases"
if [[ -n "$DESTDIR" ]]; then
  say "  skipped: staging registers nothing"
elif [[ "$CHANGED" == yes ]]; then
  if command -v update-desktop-database >/dev/null 2>&1; then
    step "refresh the desktop entry cache" privileged update-desktop-database "$PREFIX/share/applications"
  fi
  if command -v gtk-update-icon-cache >/dev/null 2>&1 && [[ -d "$PREFIX/share/icons/hicolor" ]]; then
    step "refresh the icon cache" privileged gtk-update-icon-cache -q -t -f "$PREFIX/share/icons/hicolor"
  fi
else
  say "  nothing changed"
fi
say "done."
