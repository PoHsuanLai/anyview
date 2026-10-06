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
unset ANYVIEW_BIN ICON_DIR DESTDIR CARGO_TARGET_DIR

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

# A binary that is not the real one, and an icon folder with two sizes.
fake_bin="$scratch/anyview"
printf '#!/bin/sh\necho fake viewer\n' >"$fake_bin"
chmod +x "$fake_bin"
icons="$scratch/icons"
mkdir -p "$icons"
printf 'png16' >"$icons/16.png"
printf 'png256' >"$icons/256.png"
printf 'not a size' >"$icons/README.png"
export ANYVIEW_BIN="$fake_bin" ICON_DIR="$icons"

failures=0
check() { # check <what> <command...>
  local what="$1"; shift
  if "$@" >/dev/null 2>&1; then echo "ok: $what"; else echo "FAIL: $what"; failures=$((failures + 1)); fi
}
tree_of() { (cd "$1" 2>/dev/null && find . -mindepth 1 | LC_ALL=C sort); }
empty() { [ -z "$(tree_of "$1")" ]; }
installed="$stage$prefix"

# The plugins are on by default; sections 1 to 5 are about the viewer alone, so they leave them out.
install() { DESTDIR="$stage" bash "$repo/dist/install.sh" --prefix "$prefix" ${plugin_flags[@]+"${plugin_flags[@]}"} "$@"; }
plugin_flags=(--no-plugins)
uninstall() { DESTDIR="$stage" bash "$repo/dist/uninstall.sh" --prefix "$prefix" "$@"; }

# 1. A dry run prints every action and writes nothing.
out="$(install --dry-run 2>&1)"
check "dry run leaves no staging tree" test ! -e "$stage"
check "dry run says it is a dry run" grep -q "dry run: nothing is changed" <<<"$out"
check "dry run names the binary" grep -q "$prefix/bin/anyview" <<<"$out"
check "dry run names the entry" grep -q "$prefix/share/applications/org.quire.Anyview.desktop" <<<"$out"
check "dry run names the notices" grep -q "$prefix/share/doc/anyview/THIRD-PARTY-NOTICES.md" <<<"$out"
check "dry run names the metainfo" grep -q "$prefix/share/metainfo/org.quire.Anyview.metainfo.xml" <<<"$out"
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
check "the metainfo is installed" cmp -s "$repo/dist/org.quire.Anyview.metainfo.xml" "$installed/share/metainfo/org.quire.Anyview.metainfo.xml"
check "the notices are installed under doc" cmp -s "$repo/THIRD-PARTY-NOTICES.md" "$installed/share/doc/anyview/THIRD-PARTY-NOTICES.md"
check "the service names the bus" grep -qx "Name=org.quire.Anyview1" "$service"
check "the service runs the installed binary, without DESTDIR" grep -qx "Exec=$prefix/bin/anyview" "$service"
check "the 16 icon is in place" cmp -s "$icons/16.png" "$installed/share/icons/hicolor/16x16/apps/org.quire.Anyview.png"
check "the 256 icon is in place" cmp -s "$icons/256.png" "$installed/share/icons/hicolor/256x256/apps/org.quire.Anyview.png"
want="$(printf '%s\n' ./opt ./opt/av ./opt/av/bin ./opt/av/bin/anyview ./opt/av/share ./opt/av/share/anyview ./opt/av/share/anyview/install-receipt ./opt/av/share/applications \
  ./opt/av/share/applications/org.quire.Anyview.desktop ./opt/av/share/doc ./opt/av/share/doc/anyview \
  ./opt/av/share/doc/anyview/LICENSE-APACHE ./opt/av/share/doc/anyview/LICENSE-MIT \
  ./opt/av/share/doc/anyview/THIRD-PARTY-NOTICES.md ./opt/av/share/metainfo \
  ./opt/av/share/metainfo/org.quire.Anyview.metainfo.xml ./opt/av/share/dbus-1 ./opt/av/share/dbus-1/services \
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

# 4b. Uninstall removes only what install made: a file of the person's own beside the viewer's, and
# the folders that hold it, stay, even under a prefix that holds other things.
mkdir -p "$installed/share/mine" "$installed/bin" "$installed/share/applications"
printf 'mine' >"$installed/share/mine/note.txt"
printf 'other' >"$installed/bin/other-program"
install >/dev/null 2>&1
uninstall >/dev/null 2>&1
check "uninstall keeps a stranger's file" test "$(cat "$installed/share/mine/note.txt")" = mine
check "uninstall keeps another program in bin" test "$(cat "$installed/bin/other-program")" = other
check "uninstall removes the viewer from bin" test ! -e "$installed/bin/anyview"
check "uninstall removes the receipt" test ! -e "$installed/share/anyview"
check "uninstall keeps a folder that was there before, though empty now" test -d "$installed/share/applications"
rm -rf "$stage"
# A binary that was there before is not the viewer's to remove: an identical one is not recorded.
mkdir -p "$installed/bin"
cp "$fake_bin" "$installed/bin/anyview"
install >/dev/null 2>&1
uninstall >/dev/null 2>&1
check "uninstall keeps an identical file it did not install" test -x "$installed/bin/anyview"
rm -rf "$stage"

# 5. Without the icons folder install warns and skips only the icons.
rm -rf "$icons"
install >"$scratch/noicons.out" 2>&1
check "missing icons warn" grep -q "skipping the icons" "$scratch/noicons.out"
check "missing icons still install the binary" test -x "$installed/bin/anyview"
check "missing icons install no icon" bash -c "! find $stage -name '*.png' | grep -q ."
uninstall >/dev/null 2>&1
check "uninstall empties that tree too" empty "$stage"

# 6. The plugins install by default: ffmpeg, heif and raw are built with the viewer (cargo is a shim
# here: it records its call and leaves a stand-in program where a build would) and installed with
# their manifests. mpv is looked up too; here no mpv-wgpu source is given, so its fetch is the thing
# under test, from a local repository and never from the network.
plugin_flags=()
install_plugins() { ANYVIEW_MPV_SIBLING= install "$@"; }
cargo_log="$scratch/cargo.log"
: >"$cargo_log"
cat >"$shims/cargo" <<SHIM
#!/bin/sh
echo "cargo \$* (in \$PWD, target \$CARGO_TARGET_DIR)" >> "$cargo_log"
[ -n "\$CARGO_TARGET_DIR" ] || exit 9
mkdir -p "\$CARGO_TARGET_DIR/release"
while [ \$# -gt 0 ]; do
  if [ "\$1" = -p ]; then
    case "\$2" in
      "\$FAKE_CARGO_FAIL") exit 1 ;;
      mpv-wgpu-cplugin) printf 'fake cplugin\n' >"\$CARGO_TARGET_DIR/release/libmpv_wgpu_cplugin.so" ;;
      anyview-*) printf '#!/bin/sh\necho fake plugin\n' >"\$CARGO_TARGET_DIR/release/\$2"; chmod +x "\$CARGO_TARGET_DIR/release/\$2" ;;
    esac
  fi
  shift
done
SHIM
chmod +x "$shims/cargo"
export CARGO_TARGET_DIR="$scratch/target"

mpv_manifest="$installed/share/anyview/plugins/mpv.toml"
mkdir -p "$scratch/pathbin"
printf '#!/bin/sh\necho fake mpv\n' >"$scratch/pathbin/mpv"
chmod +x "$scratch/pathbin/mpv"
with_mpv() { PATH="$scratch/pathbin:$PATH" "$@"; }

# A local mpv-wgpu stand-in: one commit, so a fetch of its exact revision needs no network.
src="$scratch/mpv-wgpu-src"
mkdir -p "$src/crates/mpv-wgpu-cplugin"
printf '[workspace]\n' >"$src/Cargo.toml"
gitq() { GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@t GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@t git "$@"; }
gitq -C "$src" init -q && gitq -C "$src" add -A && gitq -C "$src" commit -q -m stub
rev="$(git -C "$src" rev-parse HEAD)"
# The script's pin is read from the file, so a stand-in repository can be built for any revision.
pin="$(sed -n 's/^MPV_WGPU_REV=//p' "$repo/dist/install.sh")"
check "the mpv-wgpu pin is a full revision, set in one place" bash -c "[[ '$pin' =~ ^[0-9a-f]{40}\$ ]] && [ \$(grep -c '^MPV_WGPU_REV=' $repo/dist/install.sh) = 1 ]"
# The install script under test names the stand-in's revision instead (a copy with the pin changed,
# next to the real lib.sh, so the real script is never edited).
fake_repo="$scratch/fake-dist"
mkdir -p "$fake_repo/dist/plugins"
cp "$repo/dist/"*.sh "$fake_repo/dist/"
cp "$repo/dist/plugins/"* "$fake_repo/dist/plugins/"
cp "$repo/dist/"*.desktop "$repo/dist/"*.xml "$repo/dist/"*.service "$fake_repo/dist/"
cp "$repo/LICENSE-MIT" "$repo/LICENSE-APACHE" "$repo/THIRD-PARTY-NOTICES.md" "$fake_repo/"
sed -i "s/^MPV_WGPU_REV=.*/MPV_WGPU_REV=$rev/" "$fake_repo/dist/install.sh"
fetching_install() { DESTDIR="$stage" MPV_WGPU_URL="$src" ANYVIEW_MPV_SIBLING= bash "$fake_repo/dist/install.sh" --prefix "$prefix" "$@"; }

# 6a. The default dry run names every plugin, and the fetch it would make, and does nothing.
out="$(with_mpv env MPV_WGPU_URL=https://example.invalid/mpv-wgpu.git ANYVIEW_MPV_SIBLING= DESTDIR="$stage" bash "$repo/dist/install.sh" --prefix "$prefix" --dry-run 2>&1)"
for id in ffmpeg heif raw; do
  check "default dry run names the $id program" grep -q "$prefix/libexec/anyview/anyview-$id" <<<"$out"
  check "default dry run names the $id manifest" grep -q "$prefix/share/anyview/plugins/$id.toml" <<<"$out"
done
check "default dry run builds the three plugins in one cargo run" grep -q "cargo build --release -p anyview-ffmpeg -p anyview-heif -p anyview-raw" <<<"$out"
check "default dry run says what it would fetch" grep -q "git fetch --depth 1 https://example.invalid/mpv-wgpu.git $pin" <<<"$out"
check "default dry run names the mpv manifest" grep -q "$prefix/share/anyview/plugins/mpv.toml" <<<"$out"
check "the default dry run installs nothing, fetches nothing, builds nothing" bash -c "[ -z \"\$(find '$stage' -mindepth 1 2>/dev/null)\" ] && [ ! -s '$cargo_log' ] && [ -z \"\$(find '$XDG_CACHE_HOME' -mindepth 1)\" ]"

# 6b. The real default install, with the mpv-wgpu source fetched from the local stand-in.
with_mpv fetching_install >"$scratch/default.out" 2>&1
check "the default install succeeds" grep -q '^done\.' "$scratch/default.out"
for id in ffmpeg heif raw; do
  manifest="$installed/share/anyview/plugins/$id.toml"
  check "the $id program is installed and executable" test -x "$installed/libexec/anyview/anyview-$id"
  check "the $id manifest is the id's file" grep -qx "id = \"$id\"" "$manifest"
  check "the $id manifest names the installed program, without DESTDIR" grep -qx "path = \"$prefix/libexec/anyview/anyview-$id\"" "$manifest"
  check "no placeholder is left in the $id manifest" bash -c "! grep -q '@' $manifest"
  check "the receipt lists the $id program and manifest" bash -c "grep -qx 'file $prefix/libexec/anyview/anyview-$id' $installed/share/anyview/install-receipt && grep -qx 'file $prefix/share/anyview/plugins/$id.toml' $installed/share/anyview/install-receipt"
done
check "the ffmpeg manifest lists the export targets" grep -q 'targets = \["trim", "audio-copy", "m4a", "mp3", "flac", "wav", "opus"\]' "$installed/share/anyview/plugins/ffmpeg.toml"
check "the heif manifest provides decode and thumbnail" bash -c "grep -q 'capability = \"decode\"' $installed/share/anyview/plugins/heif.toml && grep -q 'capability = \"thumbnail\"' $installed/share/anyview/plugins/heif.toml"
check "the plugins were built in one release cargo run" grep -q "cargo build --release -p anyview-ffmpeg -p anyview-heif -p anyview-raw " "$cargo_log"
cache="$XDG_CACHE_HOME/anyview/build/mpv-wgpu-${rev:0:12}"
check "mpv-wgpu was fetched into the cache at its revision" test "$(git -C "$cache" rev-parse HEAD)" = "$rev"
check "the C plugin was built in the cache with its own target dir" grep -q "cargo build --release -p mpv-wgpu-cplugin (in $cache, target $cache/target)" "$cargo_log"
check "the C plugin is installed" test -x "$installed/libexec/anyview/mpv-wgpu-cplugin.so"
check "the mpv manifest names the mpv found on the search path" grep -qx "mpv = \"$scratch/pathbin/mpv\"" "$mpv_manifest"
check "the mpv manifest names the installed C plugin, without DESTDIR" grep -qx "cplugin = \"$prefix/libexec/anyview/mpv-wgpu-cplugin.so\"" "$mpv_manifest"
check "the mpv manifest provides playback" grep -q 'capability = "play"' "$mpv_manifest"
check "no placeholder is left in the mpv manifest" bash -c "! grep -q '@' $mpv_manifest"
check "the receipt lists the C plugin and the mpv manifest" bash -c "grep -qx 'file $prefix/libexec/anyview/mpv-wgpu-cplugin.so' $installed/share/anyview/install-receipt && grep -qx 'file $prefix/share/anyview/plugins/mpv.toml' $installed/share/anyview/install-receipt"
check "staging the plugins registered nothing" test ! -s "$calls"
check "HOME and XDG config stay empty" bash -c "[ -z \"\$(find $HOME $XDG_CONFIG_HOME $XDG_DATA_HOME -mindepth 1)\" ]"
# A second run reuses the checkout (no second fetch) and leaves the files alone.
: >"$cargo_log"
with_mpv fetching_install >"$scratch/default-again.out" 2>&1
check "a second install reuses the fetched checkout" grep -q "reusing the mpv-wgpu checkout" "$scratch/default-again.out"
check "a second install leaves the plugin files alone" grep -q "unchanged: $prefix/libexec/anyview/anyview-ffmpeg" "$scratch/default-again.out"
uninstall >"$scratch/default-uninstall.out" 2>&1
check "uninstall removes every plugin through the receipt and leaves the staging tree empty" empty "$stage"
check "uninstall names the mpv C plugin it removed" grep -q "remove $prefix/libexec/anyview/mpv-wgpu-cplugin.so" "$scratch/default-uninstall.out"
check "uninstall names the raw program it removed" grep -q "remove $prefix/libexec/anyview/anyview-raw" "$scratch/default-uninstall.out"
rm -rf "$XDG_CACHE_HOME/anyview"

# 6c. --without-plugin leaves one out, --no-plugins all; --with-plugin is a noted no-op.
with_mpv fetching_install --without-plugin heif --without-plugin mpv >"$scratch/without.out" 2>&1
check "--without-plugin keeps the others" test -x "$installed/libexec/anyview/anyview-ffmpeg" -a -x "$installed/libexec/anyview/anyview-raw"
check "--without-plugin heif leaves heif out" test ! -e "$installed/libexec/anyview/anyview-heif" -a ! -e "$installed/share/anyview/plugins/heif.toml"
check "--without-plugin mpv leaves mpv out and fetches nothing" bash -c "[ ! -e $mpv_manifest ] && [ ! -e '$XDG_CACHE_HOME/anyview' ]"
check "the receipt covers the plugins that were installed" grep -qx "file $prefix/share/anyview/plugins/raw.toml" "$installed/share/anyview/install-receipt"
check "the receipt names no plugin that was left out" bash -c "! grep -q 'heif\|mpv' $installed/share/anyview/install-receipt"
uninstall >/dev/null 2>&1
check "uninstall after --without-plugin leaves the staging tree empty" empty "$stage"
install --no-plugins >/dev/null 2>&1
check "--no-plugins installs the viewer alone" test ! -e "$installed/libexec" -a ! -e "$installed/share/anyview/plugins"
uninstall >/dev/null 2>&1
out="$(with_mpv fetching_install --without-plugin mpv --with-plugin ffmpeg 2>&1)"
check "--with-plugin prints a one-line deprecation note" grep -q "^note: --with-plugin is not needed any more" <<<"$out"
check "--with-plugin still installs the plugin it names" test -x "$installed/libexec/anyview/anyview-ffmpeg"
uninstall >/dev/null 2>&1
check "an unknown plugin is refused by --with-plugin" bash -c "! DESTDIR=$stage bash $repo/dist/install.sh --prefix $prefix --with-plugin nope >/dev/null 2>&1"
check "an unknown plugin is refused by --without-plugin" bash -c "! DESTDIR=$stage bash $repo/dist/install.sh --prefix $prefix --without-plugin nope >/dev/null 2>&1"
check "those refusals wrote nothing" empty "$stage"

# 6d. A named program is installed as it is, and --no-build builds nothing (a plugin with no program is
# skipped with a warning; the rest installs).
: >"$cargo_log"
printf '#!/bin/sh\necho named\n' >"$scratch/named-ffmpeg"
chmod +x "$scratch/named-ffmpeg"
ANYVIEW_FFMPEG_PLUGIN_BIN="$scratch/named-ffmpeg" fetching_install --without-plugin mpv >/dev/null 2>&1
check "a named FFmpeg program is the one installed" cmp -s "$scratch/named-ffmpeg" "$installed/libexec/anyview/anyview-ffmpeg"
check "a named program is not built" bash -c "! grep -q 'anyview-ffmpeg' '$cargo_log' || [ \"\$(grep -c 'p anyview-ffmpeg' '$cargo_log')\" = 0 ]"
uninstall >/dev/null 2>&1
: >"$cargo_log"
rm -rf "$CARGO_TARGET_DIR"
with_mpv fetching_install --no-build >"$scratch/nobuild.out" 2>&1
check "--no-build with nothing built still succeeds" grep -q '^done\.' "$scratch/nobuild.out"
check "--no-build runs no cargo and fetches nothing" bash -c "[ ! -s '$cargo_log' ] && [ ! -e '$XDG_CACHE_HOME/anyview' ]"
check "--no-build warns about each plugin it skipped" grep -q "the ffmpeg plugin was skipped\|the FFmpeg plugin was skipped" "$scratch/nobuild.out"
check "--no-build installs the viewer" test -x "$installed/bin/anyview"
check "--no-build installs no plugin that has no program" test ! -e "$installed/libexec"
uninstall >/dev/null 2>&1
check "uninstall empties that tree too" empty "$stage"

# 6e. When the mpv plugin cannot be made, the rest installs, one warning says so, and the exit is 0.
bad="$scratch/not-a-repo"
mpv_fails() { # mpv_fails WHAT URL CARGO_FAIL: an install where the mpv plugin cannot be made
  local what="$1" url="$2" fail="$3" status
  rm -rf "$XDG_CACHE_HOME/anyview"
  with_mpv env FAKE_CARGO_FAIL="$fail" DESTDIR="$stage" MPV_WGPU_URL="$url" ANYVIEW_MPV_SIBLING= \
    bash "$fake_repo/dist/install.sh" --prefix "$prefix" >"$scratch/mpvfail.out" 2>&1
  status=$?
  check "mpv: $what: the exit is 0" test "$status" = 0
  check "mpv: $what: one warning says the plugin was skipped and how to retry" test "$(grep -c "warning: .*the mpv plugin was skipped.*run install.sh again.*--with-mpv-from" "$scratch/mpvfail.out")" = 1
  check "mpv: $what: the other plugins and the viewer are installed" test -x "$installed/libexec/anyview/anyview-ffmpeg" -a -x "$installed/libexec/anyview/anyview-raw" -a -x "$installed/bin/anyview"
  check "mpv: $what: no mpv plugin is installed" test ! -e "$mpv_manifest" -a ! -e "$installed/libexec/anyview/mpv-wgpu-cplugin.so"
  uninstall >/dev/null 2>&1
  check "mpv: $what: uninstall leaves the staging tree empty" empty "$stage"
}
mkdir -p "$bad"
mpv_fails "the fetch fails (a URL that is no repository)" "$bad" ""
mpv_fails "the build fails" "$src" mpv-wgpu-cplugin
# An unreachable URL, as a machine with no network sees it, fails the same way and quickly.
rm -rf "$XDG_CACHE_HOME/anyview"
with_mpv env DESTDIR="$stage" MPV_WGPU_URL="file://$scratch/no/such/repo.git" ANYVIEW_MPV_SIBLING= bash "$fake_repo/dist/install.sh" --prefix "$prefix" >"$scratch/mpvfail2.out" 2>&1
status=$?
check "mpv: an unreachable URL: the exit is 0" test "$status" = 0
check "mpv: an unreachable URL: the plugin is skipped" grep -q 'the mpv plugin was skipped' "$scratch/mpvfail2.out"
uninstall >/dev/null 2>&1
check "no failed fetch leaves a half checkout behind" test ! -e "$cache"
# No mpv on the search path: the plugin is skipped before any fetch, and the rest installs.
empty_path="$scratch/emptybin"
mkdir -p "$empty_path"
for tool in bash env sed cat dirname basename mktemp id install rm rmdir mkdir cmp stat find git awk sort; do
  ln -s "$(type -P "$tool")" "$empty_path/$tool" 2>/dev/null
done
rm -rf "$XDG_CACHE_HOME/anyview"
PATH="$empty_path" DESTDIR="$stage" MPV_WGPU_URL="$src" ANYVIEW_MPV_SIBLING= bash "$fake_repo/dist/install.sh" --prefix "$prefix" --no-build --without-plugin ffmpeg --without-plugin heif --without-plugin raw >"$scratch/nompv.out" 2>&1
status=$?
check "with no mpv on the search path install succeeds" test "$status" = 0
check "and warns that the mpv plugin was skipped" grep -q "warning: no mpv on the search path: the mpv plugin was skipped" "$scratch/nompv.out"
check "that skip fetched nothing and installed the viewer" bash -c "[ ! -e '$XDG_CACHE_HOME/anyview' ] && [ -x $installed/bin/anyview ] && [ ! -e $mpv_manifest ]"
uninstall >/dev/null 2>&1

# 6f. mpv from a checkout: --with-mpv-from and MPV_WGPU_DIR build there, into the checkout's own target;
# a prebuilt C plugin (ANYVIEW_MPV_CPLUGIN) is installed without any build; --mpv names the mpv.
: >"$cargo_log"
with_mpv install_plugins --with-mpv-from "$src" --without-plugin ffmpeg --without-plugin heif --without-plugin raw >"$scratch/from.out" 2>&1
check "--with-mpv-from builds in that checkout" grep -q "cargo build --release -p mpv-wgpu-cplugin (in $src, target $src/target)" "$cargo_log"
check "--with-mpv-from fetches nothing" test ! -e "$XDG_CACHE_HOME/anyview"
check "--with-mpv-from installs the C plugin" test -x "$installed/libexec/anyview/mpv-wgpu-cplugin.so"
uninstall >/dev/null 2>&1
rm -rf "$src/target"
: >"$cargo_log"
with_mpv env MPV_WGPU_DIR="$src" ANYVIEW_MPV_SIBLING= DESTDIR="$stage" bash "$repo/dist/install.sh" --prefix "$prefix" --without-plugin ffmpeg --without-plugin heif --without-plugin raw >/dev/null 2>&1
check "MPV_WGPU_DIR builds in that checkout" grep -q "(in $src, target $src/target)" "$cargo_log"
uninstall >/dev/null 2>&1
rm -rf "$src/target"
printf 'prebuilt\n' >"$scratch/libmpv_wgpu_cplugin.so"
: >"$cargo_log"
ANYVIEW_MPV_CPLUGIN="$scratch/libmpv_wgpu_cplugin.so" with_mpv install_plugins --without-plugin ffmpeg --without-plugin heif --without-plugin raw >/dev/null 2>&1
check "ANYVIEW_MPV_CPLUGIN installs that file and runs no cargo" bash -c "cmp -s '$scratch/libmpv_wgpu_cplugin.so' $installed/libexec/anyview/mpv-wgpu-cplugin.so && [ ! -s '$cargo_log' ]"
uninstall >/dev/null 2>&1
ANYVIEW_MPV_CPLUGIN="$scratch/libmpv_wgpu_cplugin.so" install_plugins --without-plugin ffmpeg --without-plugin heif --without-plugin raw --mpv "$scratch/pathbin/mpv" >/dev/null 2>&1
check "--mpv names the mpv the manifest uses" grep -qx "mpv = \"$scratch/pathbin/mpv\"" "$mpv_manifest"
uninstall >/dev/null 2>&1
check "uninstall empties the tree after the mpv-only installs" empty "$stage"

# 6g. Without a receipt, uninstall still removes the plugins by name.
with_mpv fetching_install >/dev/null 2>&1
rm -f "$installed/share/anyview/install-receipt"
uninstall >/dev/null 2>&1
check "uninstall without a receipt removes the plugins by name" test ! -e "$installed/libexec"
rm -rf "$stage" "$XDG_CACHE_HOME/anyview"

# 7. A relative prefix is refused, and nothing was written for it.
check "a relative prefix is refused" bash -c "! DESTDIR=$stage bash $repo/dist/install.sh --prefix rel >/dev/null 2>&1"
check "the refusal wrote nothing" empty "$stage"

check "no tool was called throughout" test ! -s "$calls"
if [ "$failures" -ne 0 ]; then echo "$failures check(s) failed"; exit 1; fi
echo "install and uninstall hold"
