# anyview

anyview is a file viewer for Linux desktops: images, PDF, text and code, Markdown, tables, JSON, audio and
video, fonts, archives and books (EPUB and CBZ) in one window, and the same format library as the launcher's
preview pane. It is a Rust workspace written in layers, from the pure vocabulary (`anyview-core`) up to the
app, and builds on the quire design system. `ARCHITECTURE.md` is the map, `CONVENTIONS.md` the rules,
`FINDINGS.md` the open items, `CHANGELOG.md` what changed. This is a beta: version 0.1.0-beta.1.

anyview links no codec library. Video, HEIC and camera raw files are handled by your own tools through
small plugins (see Plugins), so the formats you get are the ones your distribution gives you; the common
audio formats are decoded in pure Rust and play with nothing installed.

## Build from source

You need `rustup` (the toolchain in `rust-toolchain.toml` is installed on first use), `git`, `pkg-config`
and the development packages of the system libraries the build links: fontconfig, FreeType, HarfBuzz,
libxml2, libpng, zlib, bzip2, xz, GLib and ALSA (`libasound2-dev`, `alsa-lib-devel`: the sound card).

```sh
git clone https://github.com/PoHsuanLai/anyview.git
cd anyview
cargo build --release --locked
target/release/anyview --version
```

quire (the design system), blitz-kit, the Blitz, vello and anyrender forks and pdfrum are git dependencies
pinned by revision in `Cargo.toml` and `Cargo.lock`: cargo fetches them, and nothing needs to sit next to this
checkout. The pins live in `[workspace.dependencies]`; the quire crates (`ds`, `ds-core`, `ds-blitz`,
`ds-settings`, `ds-harness`, `ds-lint`) are the lines that name quire's rev.

To work on quire or blitz-kit at the same time, check them out beside this repository and add a
`[patch."https://github.com/PoHsuanLai/quire.git"]` (or blitz-kit) table pointing at the local path to
your own copy of `Cargo.toml`; do not commit it.

Checks: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
`cargo test --workspace`, `scripts/check-boundary.sh`, `dev/no-linked-codecs.sh`, `scripts/check-deny.sh`
(needs `cargo-deny`) and `dev/install-test.sh`.

### Without the Linux desktop's services

The Linux desktop's services (D-Bus single instance with bus activation, the now-playing entry and media
keys, the print and file-chooser portals, mail sharing, and Show in Folder through the file
manager) are the cargo feature `quire-desktop` of the `anyview` binary, on by default. Everything else is
portable. To build the portable viewer:

```sh
cargo build --release --locked -p anyview --no-default-features --features audio
```

(`--no-default-features` drops `audio` too; add it back as above for the built-in audio player.) Such a build
uses a per-user socket for single instance, opens links and shows files in their folder through the platform's
opener (`xdg-open`, `open`, `explorer`), and has no Share, Print, desktop file chooser or
now-playing entry: those actions are not offered (the palette, the menus, the buttons and their keys leave them out). Until quire's own portable build lands
the dependency tree still holds quire's `zbus` and Wayland crates; this is about the viewer's own code.
Check the portable build with `scripts/check-portable.sh` (quire's two rules: the workspace builds without the feature and no core module names `crate::desktop` or zbus, plus the platform crate's own zbus check).

## Install and uninstall

```sh
dist/install.sh                      # builds, then installs the viewer and its plugins under ~/.local (or /usr/local as root)
dist/install.sh --prefix /usr        # elsewhere; uses sudo when the prefix is not yours
dist/install.sh --dry-run            # print what it would do
dist/install.sh --without-plugin mpv # leave a plugin out (--no-plugins leaves them all out)
dist/install.sh --set-default        # also make it the default for images, text, Markdown and PDF
dist/uninstall.sh                    # removes what install.sh wrote, the plugins included
```

One command installs everything: the viewer and the four plugins (see Plugins), with nothing to remember. It
installs the binary, the desktop entry, the AppStream metainfo, the D-Bus service file, the icons
(`assets/icons`) and the licences, with the notices for the crates built in
(`share/doc/anyview/THIRD-PARTY-NOTICES.md`). `DESTDIR` stages a tree for packaging. Nothing takes a file
type from another program unless you pass `--set-default`. `scripts/third-party-notices.py` regenerates the
notices from `cargo metadata`; run it when `Cargo.lock` changes.

## Plugins

Each plugin is a small separate program that runs your own distribution's tool; `dist/install.sh` installs all
four with the viewer. A plugin whose tool is not on the machine yet is installed anyway: it tells the viewer
it has nothing to offer, the file opens as its facts with a line naming what to install, and the plugin
starts working as soon as you install the package, with no second install.

Viewer offers to install the missing tool, the way Totem offers a codec. On that line an Install… button
asks "Viewer needs libheif tools to open HEIC photos."; if you agree, your system's package service
(PackageKit) installs it and asks for your password itself, and the file opens again in the same window when
it is done. Not Now asks nothing more until you press Install… again. A tool you install in a terminal
instead is noticed too, and the file opens with it. Viewer never runs a package manager or `sudo` itself,
and where PackageKit is not available (a Flatpak sandbox, an image-based system) the sheet says which
package to install. `dist/helpers/anyview.toml` lists the packages for each distribution; it installs to
`share/quire/helpers/anyview.toml`.

| Plugin | Gives you | Fedora | Debian and Ubuntu | Arch |
|---|---|---|---|---|
| `mpv` | playing video and Opus audio, and everything else with mpv's own controls (the common audio formats play without it) | `mpv` | `mpv` | `mpv` |
| `ffmpeg` | facts, pictures and conversion of recordings (`ffprobe` comes with it) | `ffmpeg-free`, or RPM Fusion's `ffmpeg` for more codecs | `ffmpeg` | `ffmpeg` |
| `heif` | HEIC, HEIF and AVIF pictures | `libheif-tools` | `libheif-examples` | `libheif` (check) |
| `raw` | camera raw files developed in full | `LibRaw-samples` | `libraw-bin` | `libraw` (check) |

The package names the Install… button uses are in `dist/helpers/anyview.toml`, with a note on each name that
has not been checked against the distribution's package list.

The `mpv` plugin loads [mpv-wgpu](https://github.com/PoHsuanLai/mpv-wgpu)'s C plugin into your mpv, so the
installer builds that too: from `--with-mpv-from DIR`, `MPV_WGPU_DIR` or a checkout at `../mpv` if there is
one, and otherwise from the revision pinned at the top of `dist/install.sh`, fetched with `git` into
`~/.cache/anyview/build` (it needs git, the network and a Rust toolchain, like the viewer's own build). If
there is no `mpv` on the search path the plugin still installs, naming `/usr/bin/mpv`, and works once mpv
is there. If the fetch or the build fails, the installer says so in one line, skips only the mpv plugin and
finishes the rest; run it again once the cause is fixed. `--mpv PATH` names
another mpv, and `--without-plugin NAME` (`ffmpeg`, `heif`, `mpv` or `raw`) leaves one out.
`--with-plugin NAME`, from earlier releases, is accepted and ignored.

A camera raw file shows its embedded preview with no plugin at all. `dist/install.sh --help` lists every option.

## Known limitations

- Audio (MP3, AAC, M4A, FLAC, WAV, AIFF and Ogg Vorbis) plays with nothing installed. Video and Opus audio need your own `mpv` and the `mpv` plugin, and the viewer says so; HEIC, HEIF and AVIF need the `heif` plugin.
- Audio played without mpv has no speed control, cover picture or chapters; with the `mpv` plugin installed, mpv plays everything.
- HTML opens as source, not rendered. Office documents show their facts and can be opened in another
  program; spreadsheets (XLSX, ODS) open as tables. RAR archives are not supported.
- Tables show the first 200,000 rows; JSON over 64 MB does not open; Markdown over 2 MB shows as source.
  A very large text file is usable at once and finishes indexing a moment later.
- Left and Right walk the files in the folder even inside a PDF, book or comic; PageUp and PageDown turn pages.
- Edits are saved in place and the original is kept under `$XDG_STATE_HOME/anyview` for a while.
- There are no settings yet. Light, dark, accent colour and reduced motion follow the desktop.
- After the last window closes the program keeps running for ten minutes so the next open is quick.
- Ctrl and Command are one key, and shortcuts are drawn with Mac symbols.
- Built and run on Linux (Wayland, the freedesktop portals and D-Bus); the portable build (`--no-default-features`, above) compiles the viewer without its D-Bus parts but has not run on macOS or Windows. Single-instance, Open With, Share, Print and Trash have been exercised against fakes, not yet on a real desktop.
- It makes no network connection and sends nothing anywhere.

## Reporting a problem

If anyview panics it prints a short report and keeps the last ten under
`$XDG_STATE_HOME/anyview/crash/` (by default `~/.local/state/anyview/crash/`). Attach the newest one, the
output of `anyview --version` and the file type involved to an issue at
<https://github.com/PoHsuanLai/anyview/issues>.

## Licence

MIT OR Apache-2.0, at your option: `LICENSE-MIT` and `LICENSE-APACHE`. Third-party licences are in
`THIRD-PARTY-NOTICES.md`. The viewer links your system's libasound (ALSA) dynamically, to reach the sound
card: it is LGPL-2.1, loaded from your distribution and not bundled, and it is not a codec library.
