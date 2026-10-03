#!/usr/bin/env bash
# Runs the real binary on a private session bus with scratch XDG directories and the sound off, and
# checks what the desktop sees of a recording that plays with no window: the MPRIS player appears and
# says Playing, Pause and Stop from the bus take effect, and a Play forwarded over D-Bus to the running
# viewer starts another. Nothing here touches the person's session, files or speakers.
#
#   dev/media-acceptance.sh [path/to/anyview]     (default: the debug build in $CARGO_TARGET_DIR or target/)
#
# Needs: dbus-run-session, busctl, a Wayland or X display is NOT needed for the playing session (it has no
# window), but the binary starts its event loop, which needs one: run it where the viewer runs.
set -uo pipefail
cd "$(dirname "$0")/.."

bin="${1:-${CARGO_TARGET_DIR:-target}/debug/anyview}"
tone="crates/anyview-media/tests/fixtures/tone.flac"
clip="crates/anyview-media/tests/fixtures/clip.mkv"
[ -x "$bin" ] || { echo "no binary at $bin (cargo build -p anyview)"; exit 2; }

scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
export HOME="$scratch/home" XDG_CONFIG_HOME="$scratch/config" XDG_DATA_HOME="$scratch/data" \
  XDG_CACHE_HOME="$scratch/cache" ANYVIEW_AUDIO_OUTPUT=null
mkdir -p "$HOME" "$XDG_CONFIG_HOME" "$XDG_DATA_HOME" "$XDG_CACHE_HOME"

session() {
  local name=org.mpris.MediaPlayer2.anyview path=/org/mpris/MediaPlayer2 player=org.mpris.MediaPlayer2.Player
  status() { busctl --user get-property "$name" "$path" "$player" PlaybackStatus 2>/dev/null | cut -d'"' -f2; }
  wait_for() { # wait_for <want> <what>
    for _ in $(seq 1 200); do [ "$(status)" = "$1" ] && return 0; sleep 0.1; done
    echo "FAIL: the player never said $1 ($2); it says '$(status)'"; return 1
  }

  "$bin" --play "$tone" &
  viewer=$!
  wait_for Playing "after --play with no window" || return 1
  echo "ok: --play published an MPRIS player that says Playing"

  busctl --user call "$name" "$path" "$player" Pause
  wait_for Paused "after Pause over the bus" || return 1
  echo "ok: Pause from the bus paused it"

  busctl --user call "$name" "$path" "$player" Play
  wait_for Playing "after Play over the bus" || return 1
  echo "ok: Play from the bus resumed it"

  busctl --user call "$name" "$path" "$player" Stop
  wait_for Stopped "after Stop over the bus" || return 1
  echo "ok: Stop from the bus ended the session"
  kill -0 "$viewer" 2>/dev/null || { echo "FAIL: the viewer exited at once; it stays warm for its linger"; return 1; }
  echo "ok: the process stays warm with no window and nothing playing"

  # A second launch forwards its request to the running viewer (single instance) and leaves.
  "$bin" --play "$clip" || { echo "FAIL: the forwarded launch failed"; return 1; }
  wait_for Playing "after a forwarded --play" || return 1
  echo "ok: a Play forwarded over D-Bus started another session"
  busctl --user call "$name" "$path" "$player" Stop
  kill "$viewer" 2>/dev/null
  wait "$viewer" 2>/dev/null
  echo "all media checks passed"
}

export -f session 2>/dev/null
export bin tone clip
dbus-run-session -- bash -c "$(declare -f session); bin='$bin'; tone='$tone'; clip='$clip'; session"
