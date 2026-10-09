#!/usr/bin/env bash
# Rebuilds crates/anyview-text/src/code/syntaxes.packdump from the sources in
# crates/anyview-text/syntaxes. Run it after adding or changing a syntax, and commit the dump.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$here/../.."
# Run from the repo root with relative paths: syntect records each file's path in the dump, so
# absolute paths would leak the builder's folders and make the dump differ between checkouts.
cd "$root"
cargo run --locked --quiet --release --manifest-path dev/syntax-dump/Cargo.toml -- \
  crates/anyview-text/syntaxes crates/anyview-text/src/code/syntaxes.packdump
