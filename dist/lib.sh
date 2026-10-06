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

# The icon sizes the repo ships, from the quire checkout: <QUIRE_DIR>/assets/icons/apps/viewer/<px>.png.
# Prints one pixel size per line; prints nothing (and warns) when the directory is absent.
icon_sizes() {
  local dir="$1/assets/icons/apps/viewer" file
  if [[ ! -d "$dir" ]]; then
    warn "no icons at $dir (set QUIRE_DIR to a quire checkout that has them): skipping the icons"
    return 0
  fi
  for file in "$dir"/*.png; do
    [[ -e "$file" ]] || continue
    file="$(basename "$file" .png)"
    if [[ "$file" =~ ^[0-9]+$ ]]; then printf '%s\n' "$file"; fi
  done | sort -n
}

# What install.sh did, so uninstall.sh removes that and nothing else. install.sh records each file it
# wrote and each directory it had to make in a receipt, <prefix>/share/anyview/install-receipt, one
# `file PATH` or `dir PATH` line each (paths as the running system sees them, never under DESTDIR).
# A file that was already there, identical, is not recorded: it is not ours. Directories are listed
# outermost first, so uninstall removes them in reverse, and only when empty.
RECEIPT_REL="share/anyview/install-receipt"
RECEIPT_NEW=()

# note_install_dirs DIR: remember the directories above (and including) DIR that do not exist yet,
# so they are the ones this run makes.
note_install_dirs() {
  local dir="$1" missing=()
  while [[ -n "$dir" && "$dir" != / && ! -d "$(dest "$dir")" ]]; do
    missing=("$dir" ${missing[@]+"${missing[@]}"})
    dir="$(dirname "$dir")"
  done
  local made
  for made in ${missing[@]+"${missing[@]}"}; do RECEIPT_NEW+=("dir $made"); done
}

# note_install_file DST: remember that this run wrote DST.
note_install_file() { RECEIPT_NEW+=("file $1"); }

# write_receipt PREFIX: the receipt, with what an earlier install recorded kept. A dry run only says so.
write_receipt() {
  local prefix="$1" receipt old tmp
  receipt="$prefix/$RECEIPT_REL"
  note_install_dirs "$(dirname "$receipt")"
  note_install_file "$receipt"
  if [[ "$DRY_RUN" == yes ]]; then
    say "  install $receipt"
    say "      (records what this install wrote, for uninstall.sh)"
    return 0
  fi
  old=""
  [[ -f "$(dest "$receipt")" ]] && old="$(cat "$(dest "$receipt")")"
  tmp="$(mktemp)"
  { [[ -z "$old" ]] || printf '%s\n' "$old"; printf '%s\n' "${RECEIPT_NEW[@]}"; } | awk '!seen[$0]++' >"$tmp"
  step "install $receipt" privileged install -Dm644 "$tmp" "$(dest "$receipt")"
  rm -f "$tmp"
}
