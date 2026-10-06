# anyview

anyview is a file viewer for Linux desktops: images, PDF, text and code, Markdown, tables, JSON, audio and
video, fonts, archives and books (EPUB and CBZ) in one window, and the same format library as the launcher's
preview pane. It is a Rust workspace written in layers, from the pure vocabulary (`anyview-core`) up to the
app, and builds on the quire design system. `ARCHITECTURE.md` is the map, `CONVENTIONS.md` the rules,
`FINDINGS.md` the open items, `CHANGELOG.md` what changed. This is a beta: version 0.1.0-beta.1.

anyview links no codec. Video, audio, HEIC and camera raw files are handled by your own tools through
small plugins (see Plugins), so the formats you get are the ones your distribution gives you.

## Build from source

You need `rustup` (the toolchain in `rust-toolchain.toml` is installed on first use), `git`, `pkg-config`
and the development packages of the system libraries the build links: fontconfig, FreeType, HarfBuzz,
libxml2, libpng, zlib, bzip2, xz and GLib.

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

## Install and uninstall

```sh
dist/install.sh                      # builds, then installs under ~/.local (or /usr/local as root)
dist/install.sh --prefix /usr        # elsewhere; uses sudo when the prefix is not yours
dist/install.sh --dry-run            # print what it would do
dist/install.sh --set-default        # also make it the default for images, text, Markdown and PDF
dist/uninstall.sh                    # removes what install.sh wrote, the plugins included
```

It installs the binary, the desktop entry, the AppStream metainfo, the D-Bus service file, the icons
(`assets/icons`) and the licences, with the notices for the crates built in
(`share/doc/anyview/THIRD-PARTY-NOTICES.md`). `DESTDIR` stages a tree for packaging. Nothing takes a file
type from another program unless you pass `--set-default`. `scripts/third-party-notices.py` regenerates the
notices from `cargo metadata`; run it when `Cargo.lock` changes.

## Plugins

Each plugin is a separate program that runs your own tool; install the tool, then the plugin:

| Plugin | Gives you | Your tools (Fedora / Debian and Ubuntu) |
|---|---|---|
| `mpv` | playing video and audio | `mpv` |
| `ffmpeg` | facts, pictures and conversion of recordings | `ffmpeg` (`ffprobe` comes with it; Fedora's `ffmpeg-free` has fewer codecs than RPM Fusion's `ffmpeg`) |
| `heif` | HEIC, HEIF and AVIF pictures | `libheif-tools` / `libheif-examples` |
| `raw` | camera raw files developed in full | `LibRaw-samples` / `libraw-bin` |

```sh
dist/install.sh --with-plugin ffmpeg --with-plugin heif --with-plugin raw
```

The mpv plugin is optional and is the one that needs a second repository: it loads mpv-wgpu's C plugin
into your mpv, so it is built from a checkout of
[mpv-wgpu](https://github.com/PoHsuanLai/mpv-wgpu) (the revision in `Cargo.lock` is the one tested):

```sh
git clone https://github.com/PoHsuanLai/mpv-wgpu.git ../mpv
dist/install.sh --with-plugin mpv    # MPV_WGPU_DIR names another checkout, --mpv PATH another mpv
```

Without a plugin the file still opens, as its facts with the package that would handle it named. A camera
raw file shows its embedded preview with no plugin at all. `dist/install.sh --help` lists every option.

## Known limitations

- Video and audio need your own `mpv` and the `mpv` plugin; HEIC, HEIF and AVIF need the `heif` plugin.
- HTML opens as source, not rendered. Office documents show their facts and can be opened in another
  program; spreadsheets (XLSX, ODS) open as tables. RAR archives are not supported.
- Tables show the first 200,000 rows; JSON over 64 MB does not open; Markdown over 2 MB shows as source.
  A very large text file is usable at once and finishes indexing a moment later.
- Left and Right walk the files in the folder even inside a PDF, book or comic; PageUp and PageDown turn pages.
- Edits are saved in place and the original is kept under `$XDG_STATE_HOME/anyview` for a while.
- There are no settings yet. Light, dark, accent colour and reduced motion follow the desktop.
- After the last window closes the program keeps running for ten minutes so the next open is quick.
- Ctrl and Command are one key, and shortcuts are drawn with Mac symbols.
- Linux only (Wayland, the freedesktop portals and D-Bus). Single-instance, Open With, Share, Print and Trash have been exercised against fakes, not yet on a real desktop.
- It makes no network connection and sends nothing anywhere.

## Reporting a problem

If anyview panics it prints a short report and keeps the last ten under
`$XDG_STATE_HOME/anyview/crash/` (by default `~/.local/state/anyview/crash/`). Attach the newest one, the
output of `anyview --version` and the file type involved to an issue at
<https://github.com/PoHsuanLai/anyview/issues>.

## Licence

MIT OR Apache-2.0, at your option: `LICENSE-MIT` and `LICENSE-APACHE`. Third-party licences are in
`THIRD-PARTY-NOTICES.md`.
