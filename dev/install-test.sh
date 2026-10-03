#!/usr/bin/env bash
# Runs dist/install.sh and dist/uninstall.sh in a scratch directory and checks what they did: the
# staged tree, that --dry-run writes nothing, that --set-default is opt-in, a missing icon folder
# only warns, and that uninstall leaves the staging tree empty. HOME and every XDG directory are
# scratch, DESTDIR stages the files, and the tools that register files with a desktop are shims that
# only record a call, so nothing here can reach the real system.
#
#   dev/install-test.sh
set -uo pipefail
cd "$(dirname "$0")/.."
repo="$PWD"

scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
export HOME="$scratch/home" XDG_CONFIG_HOME="$scratch/config" XDG_DATA_HOME="$scratch/data" \
  XDG_CACHE_HOME="$scratch/cache" XDG_RUNTIME_DIR="$scratch/run"
mkdir -p "$HOME" "$XDG_CONFIG_HOME" "$XDG_DATA_HOME" "$XDG_CACHE_HOME" "$XDG_RUNTIME_DIR"
unset ANYVIEW_BIN QUIRE_DIR DESTDIR CARGO_TARGET_DIR

stage="$scratch/stage"
prefix=/opt/av
calls="$scratch/calls.log"
: >"$calls"

# Shims first on PATH: a registration tool that is called records it and does nothing.
shims="$scratch/shims"
mkdir -p "$shims"
for tool in update-desktop-database gtk-update-icon-cache xdg-mime sudo; do
  printf '#!/bin/sh\necho "%s $*" >> "%s"\n' "$tool" "$calls" >"$shims/$tool"
  chmod +x "$shims/$tool"
done
export PATH="$shims:$PATH"

# A binary that is not the real one, and a quire checkout with two icon sizes.
fake_bin="$scratch/anyview"
printf '#!/bin/sh\necho fake viewer\n' >"$fake_bin"
chmod +x "$fake_bin"
quire="$scratch/quire"
mkdir -p "$quire/assets/icons/apps/viewer"
printf 'png16' >"$quire/assets/icons/apps/viewer/16.png"
printf 'png256' >"$quire/assets/icons/apps/viewer/256.png"
printf 'not a size' >"$quire/assets/icons/apps/viewer/README.png"
export ANYVIEW_BIN="$fake_bin" QUIRE_DIR="$quire"

failures=0
check() { # check <what> <command...>
  local what="$1"; shift
  if "$@" >/dev/null 2>&1; then echo "ok: $what"; else echo "FAIL: $what"; failures=$((failures + 1)); fi
}
tree_of() { (cd "$1" 2>/dev/null && find . -mindepth 1 | LC_ALL=C sort); }
empty() { [ -z "$(tree_of "$1")" ]; }
installed="$stage$prefix"

install() { DESTDIR="$stage" bash "$repo/dist/install.sh" --prefix "$prefix" "$@"; }
uninstall() { DESTDIR="$stage" bash "$repo/dist/uninstall.sh" --prefix "$prefix" "$@"; }

# 1. A dry run prints every action and writes nothing.
out="$(install --dry-run 2>&1)"
check "dry run leaves no staging tree" test ! -e "$stage"
check "dry run says it is a dry run" grep -q "dry run: nothing is changed" <<<"$out"
check "dry run names the binary" grep -q "$prefix/bin/anyview" <<<"$out"
check "dry run names the entry" grep -q "$prefix/share/applications/org.quire.Anyview.desktop" <<<"$out"
check "dry run names the service" grep -q "$prefix/share/dbus-1/services/org.quire.Anyview1.service" <<<"$out"
check "dry run names both icons" grep -q "256x256/apps/org.quire.Anyview.png" <<<"$out"
check "dry run ignores a file that is not a size" bash -c '! grep -q README <<<"$0"' "$out"
check "dry run without --set-default names no xdg-mime" bash -c '! grep -q xdg-mime <<<"$0"' "$out"

# 2. --set-default names images, plain text, Markdown and PDF, and nothing else.
out="$(install --dry-run --set-default 2>&1)"
for type in image/png image/svg+xml text/plain text/markdown application/pdf; do
  check "--set-default says $type" grep -q "$type" <<<"$out"
done
for type in video/mp4 audio/flac text/html text/csv application/json; do
  check "--set-default leaves $type" bash -c '! grep -q "xdg-mime default org.quire.Anyview.desktop $0" <<<"$1"' "$type" "$out"
done
check "dry run wrote nothing in HOME or XDG" bash -c "empty() { [ -z \"\$(find \"\$1\" -mindepth 1)\" ]; }; empty $HOME && empty $XDG_CONFIG_HOME && empty $XDG_DATA_HOME"
check "dry run called no tool" test ! -s "$calls"

# 3. The real run, staged.
install >"$scratch/install.out" 2>&1
check "the binary is installed and executable" test -x "$installed/bin/anyview"
check "the entry is installed" cmp -s "$repo/dist/org.quire.Anyview.desktop" "$installed/share/applications/org.quire.Anyview.desktop"
service="$installed/share/dbus-1/services/org.quire.Anyview1.service"
check "the service names the bus" grep -qx "Name=org.quire.Anyview1" "$service"
check "the service runs the installed binary, without DESTDIR" grep -qx "Exec=$prefix/bin/anyview" "$service"
check "the 16 icon is in place" cmp -s "$quire/assets/icons/apps/viewer/16.png" "$installed/share/icons/hicolor/16x16/apps/org.quire.Anyview.png"
check "the 256 icon is in place" cmp -s "$quire/assets/icons/apps/viewer/256.png" "$installed/share/icons/hicolor/256x256/apps/org.quire.Anyview.png"
want="$(printf '%s\n' ./opt ./opt/av ./opt/av/bin ./opt/av/bin/anyview ./opt/av/share ./opt/av/share/applications \
  ./opt/av/share/applications/org.quire.Anyview.desktop ./opt/av/share/dbus-1 ./opt/av/share/dbus-1/services \
  ./opt/av/share/dbus-1/services/org.quire.Anyview1.service ./opt/av/share/icons ./opt/av/share/icons/hicolor \
  ./opt/av/share/icons/hicolor/16x16 ./opt/av/share/icons/hicolor/16x16/apps \
  ./opt/av/share/icons/hicolor/16x16/apps/org.quire.Anyview.png ./opt/av/share/icons/hicolor/256x256 \
  ./opt/av/share/icons/hicolor/256x256/apps ./opt/av/share/icons/hicolor/256x256/apps/org.quire.Anyview.png | LC_ALL=C sort)"
check "the staged tree is exactly the files" test "$(tree_of "$stage")" = "$want"
check "staging registered nothing" test ! -s "$calls"
check "HOME and XDG stay empty" bash -c "[ -z \"\$(find $HOME $XDG_CONFIG_HOME $XDG_DATA_HOME -mindepth 1)\" ]"
install >"$scratch/again.out" 2>&1
check "a second install leaves the files alone" grep -q "unchanged: $prefix/bin/anyview" "$scratch/again.out"

# 4. An uninstall dry run keeps everything; the real one empties the staging tree.
before="$(tree_of "$stage")"
uninstall --dry-run >/dev/null 2>&1
check "uninstall dry run removes nothing" test "$(tree_of "$stage")" = "$before"
uninstall >"$scratch/uninstall.out" 2>&1
check "uninstall leaves the staging tree empty" empty "$stage"
check "a second uninstall is quiet and succeeds" uninstall

# 5. Without the icons folder install warns and skips only the icons.
rm -rf "$quire/assets"
install >"$scratch/noicons.out" 2>&1
check "missing icons warn" grep -q "skipping the icons" "$scratch/noicons.out"
check "missing icons still install the binary" test -x "$installed/bin/anyview"
check "missing icons install no icon" bash -c "! find $stage -name '*.png' | grep -q ."
uninstall >/dev/null 2>&1
check "uninstall empties that tree too" empty "$stage"

# 6. --with-plugin ffmpeg adds the plugin's program and its manifest, and only when asked.
printf '#!/bin/sh\necho fake plugin\n' >"$scratch/anyview-ffmpeg"
chmod +x "$scratch/anyview-ffmpeg"
export ANYVIEW_FFMPEG_PLUGIN_BIN="$scratch/anyview-ffmpeg"
manifest="$installed/share/anyview/plugins/ffmpeg.toml"
out="$(install --dry-run --with-plugin ffmpeg 2>&1)"
check "dry run names the plugin program" grep -q "$prefix/libexec/anyview/anyview-ffmpeg" <<<"$out"
check "dry run names the plugin manifest" grep -q "$prefix/share/anyview/plugins/ffmpeg.toml" <<<"$out"
check "dry run installs no plugin" empty "$stage"
install >/dev/null 2>&1
check "without --with-plugin no plugin is installed" test ! -e "$installed/libexec" -a ! -e "$manifest"
uninstall >/dev/null 2>&1
install --with-plugin ffmpeg >"$scratch/plugin.out" 2>&1
check "the plugin's program is installed and executable" test -x "$installed/libexec/anyview/anyview-ffmpeg"
check "the manifest is installed" test -f "$manifest"
check "the manifest is the id's file" grep -qx 'id = "ffmpeg"' "$manifest"
check "the manifest names the installed program, without DESTDIR" grep -qx "path = \"$prefix/libexec/anyview/anyview-ffmpeg\"" "$manifest"
check "no placeholder is left in the manifest" bash -c "! grep -q '@' $manifest"
check "the manifest lists the export targets" grep -q 'targets = \["trim", "audio-copy", "m4a", "mp3", "flac", "wav", "opus"\]' "$manifest"
check "the viewer is installed beside it" test -x "$installed/bin/anyview"
check "staging the plugin registered nothing" test ! -s "$calls"
install --with-plugin ffmpeg >"$scratch/plugin-again.out" 2>&1
check "a second plugin install leaves the files alone" grep -q "unchanged: $prefix/libexec/anyview/anyview-ffmpeg" "$scratch/plugin-again.out"
check "an unknown plugin is refused" bash -c "! DESTDIR=$stage bash $repo/dist/install.sh --prefix $prefix --with-plugin nope >/dev/null 2>&1"
uninstall >"$scratch/plugin-uninstall.out" 2>&1
check "uninstall removes the plugin and leaves the staging tree empty" empty "$stage"
check "uninstall names the plugin it removed" grep -q "remove $prefix/libexec/anyview/anyview-ffmpeg" "$scratch/plugin-uninstall.out"
unset ANYVIEW_FFMPEG_PLUGIN_BIN

# 7. A relative prefix is refused, and nothing was written for it.
check "a relative prefix is refused" bash -c "! DESTDIR=$stage bash $repo/dist/install.sh --prefix rel >/dev/null 2>&1"
check "the refusal wrote nothing" empty "$stage"

check "no tool was called throughout" test ! -s "$calls"
if [ "$failures" -ne 0 ]; then echo "$failures check(s) failed"; exit 1; fi
echo "install and uninstall hold"
