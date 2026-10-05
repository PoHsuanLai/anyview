# Shared by install.sh and uninstall.sh. Sourced, never run.
#
# Every change goes through `step`: it prints the command and runs it unless --dry-run. Under
# DESTDIR (a staging root, as for packagers and for the tests) files land under that directory and
# nothing talks to the desktop's databases, so a staging run cannot reach the real system.

DRY_RUN=no
DESTDIR="${DESTDIR:-}"
APP_ID=org.quire.Anyview
BUS_NAME=org.quire.Anyview1

say() { printf '%s\n' "$*"; }
warn() { printf 'warning: %s\n' "$*" >&2; }

# step DESCRIPTION CMD...: print, then run unless this is a dry run.
step() {
  local what="$1"; shift
  say "  $what"
  say "      \$ $*"
  [[ "$DRY_RUN" == yes ]] || "$@"
}

# The prefix when none is given: /usr/local for root, the person's own ~/.local otherwise (the
# way keycap installs system-wide and a user install stays in $HOME).
default_prefix() {
  if [[ "$(id -u)" == 0 ]]; then
    printf '%s' /usr/local
  else
    printf '%s' "${HOME:?HOME is not set}/.local"
  fi
}

# dest PATH: where an absolute path really goes, under DESTDIR when there is one.
dest() { printf '%s%s' "$DESTDIR" "$1"; }

# privileged CMD...: a command that writes under a system prefix runs through sudo unless we are
# root, stage into DESTDIR, or the prefix is the person's own.
SUDO=""
privileged() { if [[ -n "$SUDO" ]]; then "$SUDO" "$@"; else "$@"; fi; }
choose_sudo() {
  local prefix="$1"
  if [[ -n "$DESTDIR" || "$(id -u)" == 0 || "$prefix" == "${HOME:-/nonexistent}"/* ]]; then
    SUDO=""
  else
    SUDO=sudo
  fi
}

# The icon sizes the repo ships: <ICON_DIR>/<px>.png (assets/icons, copied from quire's viewer icons).
# Prints one pixel size per line; prints nothing (and warns) when the directory is absent.
icon_sizes() {
  local dir="$1" file
  if [[ ! -d "$dir" ]]; then
    warn "no icons at $dir (set ICON_DIR to a folder of <px>.png files): skipping the icons"
    return 0
  fi
  for file in "$dir"/*.png; do
    [[ -e "$file" ]] || continue
    file="$(basename "$file" .png)"
    if [[ "$file" =~ ^[0-9]+$ ]]; then printf '%s\n' "$file"; fi
  done | sort -n
}

# The directories a staging run may leave empty after an uninstall are removed, deepest first. A
# real system keeps its own share/ tree: only the icon folders this program's files sat in go.
prune_empty() {
  if [[ -n "$DESTDIR" ]]; then
    [[ -d "$DESTDIR" ]] && find "$DESTDIR" -mindepth 1 -depth -type d -empty -delete
  fi
}
