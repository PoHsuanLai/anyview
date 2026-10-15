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

# privileged CMD...: a command that writes under a system prefix runs as root unless we are root,
# stage into DESTDIR, or the prefix is the person's own. What makes it root is, in order: the
# person's own $SUDO, then sudo, doas (Alpine, some Arch and BSD-minded setups) and pkexec
# (polkit, on any desktop). With none of them the run turns into a dry run and says so: every
# command is printed for the person to run as root, and nothing half-installs.
ELEVATE_FROM_ENV="${SUDO:-}"
SUDO=""
privileged() { if [[ -n "$SUDO" ]]; then "$SUDO" "$@"; else "$@"; fi; }
choose_sudo() {
  local prefix="$1" tool
  if [[ -n "$DESTDIR" || "$(id -u)" == 0 || "$prefix" == "${HOME:-/nonexistent}"/* ]]; then
    SUDO=""
    return 0
  fi
  for tool in "$ELEVATE_FROM_ENV" sudo doas pkexec; do
    if [[ -n "$tool" ]] && command -v "$tool" >/dev/null 2>&1; then
      SUDO="$tool"
      return 0
    fi
  done
  SUDO=""
  if [[ "$DRY_RUN" == no ]]; then
    warn "writing under $prefix needs root, and there is no sudo, doas or pkexec here (or set SUDO=<tool>)"
    warn "nothing is changed: the commands below are what to run as root, or run again with --prefix \"\$HOME/.local\""
    DRY_RUN=yes
  fi
}

# One scratch directory for the run, made by make_scratch and removed when the script ends: the
# rendered files, and the tree staged for the one elevated step.
SCRATCH=""
make_scratch() {
  SCRATCH="$(mktemp -d)"
  trap 'rm -rf -- "${SCRATCH:?}"' EXIT
}

# plain_path WHAT PATH: succeed for a path that can be written, as it is, into a sed program, a D-Bus or
# desktop Exec= line and a TOML string. Others (a space, a quote, a backslash, a | or & or %) are
# refused, as autotools refuses a prefix it cannot quote, rather than escaped for each place.
plain_path() {
  case "$2" in
    ''|*[!A-Za-z0-9_./+@:,=-]*)
      say "install.sh: $1 ($2) has a character that does not go safely into the files written from it; use letters, digits and _ . / + @ : , = -"
      return 1 ;;
    *) return 0 ;;
  esac
}

# A system prefix is written in one elevated step, so a prompt that is declined leaves nothing
# half installed and a prompt that is kept (pkexec, doas without persist) is asked once. Everything is
# first put, as the person, into $STAGE/tree/<destination>; $STAGE/first and $STAGE/list name each file
# with its mode, the receipt in "first" so a copy that stops half way is still one uninstall.sh can undo.
# STAGE stays empty (files are written one by one, as the person or as root) for a person's own prefix,
# a DESTDIR, and a dry run.
STAGE=""
stage_begin() { # after choose_sudo and make_scratch
  [[ -n "$SUDO" && "$DRY_RUN" == no ]] || return 0
  STAGE="$SCRATCH/stage"
  mkdir -p -- "$STAGE/tree"
  : >"$STAGE/first"
  : >"$STAGE/list"
}

# put_file MODE SRC DST [LIST]: write SRC as DST with MODE, or stage it for commit_staged.
put_file() {
  local mode="$1" src="$2" dst="$3" list="${4:-list}"
  if [[ -n "$STAGE" ]]; then
    say "  stage $dst"
    install -Dm"$mode" "$src" "$STAGE/tree$dst"
    printf '%s %s\n' "$mode" "$dst" >>"$STAGE/$list"
  else
    step "install $dst" privileged install -Dm"$mode" "$src" "$(dest "$dst")"
  fi
}

# What the elevated step runs, given the stage and the prefix (plain sh: it is root's).
APPLY_STAGED='set -eu
stage=$1 prefix=$2
for list in first list; do
  while read -r mode dst; do
    install -Dm"$mode" "$stage/tree$dst" "$dst"
  done <"$stage/$list"
done
if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$prefix/share/applications"
fi
if [ -d "$prefix/share/icons/hicolor" ] && command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -q -t -f "$prefix/share/icons/hicolor"
fi'

# commit_staged PREFIX: the one elevated call, which copies what was staged and refreshes the caches.
commit_staged() {
  [[ -n "$STAGE" ]] || return 0
  step "copy the staged files into $1 and refresh the desktop's caches" \
    privileged sh -c "$APPLY_STAGED" sh "$STAGE" "$1"
}

# The icon sizes the repo ships: <ICON_DIR>/<px>.png (assets/icons, Viewer's own icon; the 1024 px master
# and the Windows .ico are in assets/icon-source, as hicolor has no 1024 size).
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
  put_file 644 "$tmp" "$receipt" first
  rm -f -- "$tmp"
}
