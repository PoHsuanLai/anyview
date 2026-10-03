#!/usr/bin/env bash
# Remove what install.sh wrote, and nothing else; safe to run again.
#
#   dist/uninstall.sh [--dry-run] [--prefix DIR]
#
#   --dry-run     print every action, change nothing
#   --prefix DIR  the prefix it was installed under (default: /usr/local as root, else ~/.local)
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
    -h|--help) sed -n '2,11p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
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

say "1. files"
remove_file "$PREFIX/bin/anyview"
remove_file "$PREFIX/share/applications/$APP_ID.desktop"
remove_file "$PREFIX/share/dbus-1/services/$BUS_NAME.service"
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
[[ "$DRY_RUN" == yes ]] || prune_empty

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
