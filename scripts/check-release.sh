#!/usr/bin/env bash
# Release metadata kept by hand in several places must agree:
#   - the mpv-wgpu revision dist/install.sh fetches is the one Cargo.toml pins the player to (the CI
#     player job builds Cargo.toml's, the installer would otherwise build another);
#   - the version in Cargo.toml, the first <release> of the metainfo and the newest numbered
#     CHANGELOG.md heading (not "Unreleased") are one, and the metainfo and the changelog give it the
#     same date.
set -uo pipefail
cd "$(dirname "$0")/.."

failed=0
fail() { printf 'check-release: %s\n' "$*" >&2; failed=1; }

cargo_rev="$(sed -n 's/^mpv-wgpu-player = .*rev = "\([0-9a-f]*\)".*/\1/p' Cargo.toml)"
install_rev="$(sed -n 's/^MPV_WGPU_REV=\([0-9a-f]*\)$/\1/p' dist/install.sh)"
if [[ -z "$cargo_rev" || -z "$install_rev" ]]; then
  fail "could not read the mpv-wgpu rev (Cargo.toml: '$cargo_rev', dist/install.sh: '$install_rev')"
elif [[ "$cargo_rev" != "$install_rev" ]]; then
  fail "MPV_WGPU_REV in dist/install.sh ($install_rev) differs from the mpv-wgpu-player rev in Cargo.toml ($cargo_rev)"
fi

cargo_version="$(sed -n '/^\[workspace\.package\]/,/^\[/{s/^version = "\(.*\)"$/\1/p}' Cargo.toml)"
meta_version="$(sed -n 's/.*<release version="\([^"]*\)".*/\1/p' dist/org.quire.Anyview.metainfo.xml | head -n 1)"
meta_date="$(sed -n 's/.*<release version="[^"]*" date="\([^"]*\)".*/\1/p' dist/org.quire.Anyview.metainfo.xml | head -n 1)"
log_line="$(grep -m 1 -E '^## [0-9]' CHANGELOG.md || true)"
log_version="$(printf '%s' "$log_line" | sed -n 's/^## \([^ ]*\).*/\1/p')"
log_date="$(printf '%s' "$log_line" | sed -n 's/^## [^ ]* (\([^)]*\)).*/\1/p')"
if [[ -z "$cargo_version" || -z "$meta_version" || -z "$log_version" ]]; then
  fail "could not read all three versions (Cargo.toml: '$cargo_version', metainfo: '$meta_version', CHANGELOG.md: '$log_version')"
else
  [[ "$cargo_version" == "$meta_version" ]] \
    || fail "Cargo.toml says $cargo_version, the metainfo's newest release says $meta_version"
  [[ "$cargo_version" == "$log_version" ]] \
    || fail "Cargo.toml says $cargo_version, CHANGELOG.md's newest numbered entry says $log_version"
  [[ "$meta_date" == "$log_date" ]] \
    || fail "the metainfo dates $meta_version as $meta_date, CHANGELOG.md as $log_date"
fi

exit "$failed"
