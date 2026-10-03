#!/usr/bin/env bash
# Checks the built viewer binary links no libmpv and no libav*: playback is the person's own mpv run
# as a child process and probing and writing recordings is the FFmpeg plugin, so none of those
# libraries is loaded into the viewer (CONVENTIONS section 15, "run, never link"). It reads the
# binary's dynamic dependencies with `ldd` (a binary is never run) and fails when ldd says nothing,
# so a binary it could not read cannot pass.
#
#   dev/no-linked-codecs.sh [path/to/anyview]     (default: the debug build in $CARGO_TARGET_DIR or target/)
set -uo pipefail
cd "$(dirname "$0")/.."

bin="${1:-${CARGO_TARGET_DIR:-target}/debug/anyview}"
[ -x "$bin" ] || { echo "no binary at $bin (cargo build -p anyview)"; exit 2; }

linked="$(ldd "$bin" 2>&1)"
if ! grep -q "libc\.so" <<<"$linked"; then
  echo "FAIL: ldd could not read $bin:"
  echo "$linked"
  exit 2
fi
found="$(grep -iE 'libmpv|libav|libsw(scale|resample)|libpostproc' <<<"$linked" || true)"
if [ -n "$found" ]; then
  echo "FAIL: $bin links codec libraries:"
  echo "$found"
  exit 1
fi
echo "ok: $bin links no libmpv and no libav ($(grep -c '=>' <<<"$linked") libraries in all)"
