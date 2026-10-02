# Findings

This file holds two kinds of entry: **open items**, things still unresolved with the condition
that ends each, and **standing facts**, things true at the current pins that code and tests rely
on. It is a reference, not a log: how each was found lives in git history.

## Open items

- **No `[patch]` sections yet.** sill patches `blitz-kit` to a sibling checkout and the vello and
  anyrender crates to forks; `anyview-core`, `anyview-store` and the machines of `anyview-ui` name
  none of them and cargo warns about every unused patch, so the root `Cargo.toml` carries none.
  Ends when the first crate that names Blitz, vello or anyrender lands (the views of
  `anyview-ui`): copy the sections from sill's `Cargo.toml` in that change.
- **`NoExportKind` is a hand-written `Word`.** `#[derive(Word)]` refuses an enum with no variant,
  and `ExportChoice::Kind` must be a `Word` even for a format with no export. The impl lists no
  variants. Ends when quire's derive accepts an empty enum.
- **Two `Percent` types.** `anyview_core::Percent` is an unclamped whole percentage (a volume goes
  to 150); `ds_core::vocab::Percent` clamps to 100. They share no definition because their ranges
  differ. Ends if quire's grows an unclamped sibling.
- **Export defaults and peek budgets are not settings yet.** The JPEG quality (90), the AVIF
  quality (65) and the default paper (A4, portrait) are constants in `export/`; `PeekBudget` has no
  default at all, so the caller reads it from settings. Ends when quire's `22-SETTINGS` has
  `viewer.export.*` and `viewer.peek.*` keys that supply them.
- **The pinned block has no image codecs beyond png and jpeg, and none of the back-end crates.**
  `anyview-image` adds `gif webp bmp tiff ico tga qoi` to the pinned `image` line from its own
  manifest, and `jxl-oxide`, `resvg`, `kamadak-exif`, `img-parts`, `ravif`, `syntect`,
  `pulldown-cmark`, `csv` and `encoding_rs` sit below the pinned block next to `infer`. Ends at the next
  change to quire's `docs/workspace-deps.toml`: add those features to its `image` line and those crates
  to it (CONVENTIONS section 10), then copy the block verbatim here.
- **AVIF decoding is not built or tested here.** The `avif` feature of `anyview-image` needs the dav1d
  C library with its headers and a `dav1d.pc` for pkg-config; this build image has the runtime library
  (`libdav1d7`) but not `libdav1d-dev`, so `cargo clippy --all-features` and `cargo test --all-features`
  stop in `dav1d-sys`. The gate runs without `--all-features`, and an AVIF file is
  `ImageError::NotCompiledIn`. AVIF encoding (`ravif`) is always built and tested. Ends when the image
  or CI has `libdav1d-dev` and `pkg-config`: run the gate with `--all-features`.
- **SVG text is not drawn.** `resvg` is built without its `text` feature and with no font database, and
  external files an SVG names are not read, so only what the SVG itself holds appears. Ends when
  `anyview-platform` can hand the decoder a font database (the `text` feature and `fontdb`).
- **A legacy-encoded text file is not text to the sniffer.** `anyview_core::sniff` calls a head that is
  not valid UTF-8 and has no byte-order mark `Other`, so a Windows-1252 or Shift_JIS `.txt` never reaches
  `anyview-text`; the Windows-1252 fallback of `detect` serves files whose first 4 KiB is ASCII and whose
  accents come later. Ends when `sniff` classifies a head with no NUL and mostly printable bytes as plain
  text under a text extension; no change is needed in `anyview-text`.
- **The only fallback encoding is Windows-1252.** A file in another legacy encoding (Shift_JIS, GBK,
  KOI8-R) is shown as Windows-1252 with mojibake. Ends if that matters: guess with `chardetng`.
- **The back ends' limits are constants.** `PEEK_LINES` (40), `DETECT_BYTES` (1 MiB), the largest image
  to decode (16384 by 16384), the largest animation in memory (512 MiB), the SVG long edge (1024 to 4096),
  the AVIF encoder speed (6), the longest line highlighted (4096 bytes), the largest inlined Markdown
  image (8 MiB) and the top-level keys a fact names (8) are not settings. Ends when quire's `22-SETTINGS`
  has `viewer.peek.*` and `viewer.image.*` keys; the callers then pass them in.
- **The peeks' time budget is not enforced.** `PeekBudget::time` is for the caller: no crate here reads a
  clock, so a slow decode or AVIF encode is abandoned by dropping its worker. Ends if a deadline is
  injected (a `Stamp` source) and the decoders are polled.
- **`CodePeek` builds the highlighter's syntax set on every call.** `Peek::peek` takes no state to keep it
  in, and a global is not allowed. `syntect` loads syntaxes lazily, so a call costs the one syntax it
  uses. Ends if `Peek` gains an environment argument.
- **syntect's default syntaxes lack TypeScript, Kotlin, Swift, TOML, INI, SCSS and Dockerfile.**
  TypeScript is highlighted as JavaScript, Kotlin as Java, SCSS as CSS, and the rest are plain. Ends when
  a bundled syntax pack (Sublime syntax files) is added to `Highlighter::new`.
- **Fenced code in rendered Markdown is highlighted only when the caller passes a `Highlighter`.** The
  `tok-<class>` classes need a stylesheet in the sealed frame that maps them to colour tokens. Ends when
  the Markdown stage's stylesheet lands in `anyview-ui`.
- **JSON is parsed whole and shown as parsed.** A JSON file larger than the peek's byte budget is refused
  with `TextError::JsonOverBudget` (a document cannot be read in part), a JSON Lines file is held in
  memory whole when opened, and a number prints as its value (`1.0` reads `1`). Ends if big JSON matters:
  scan the top level without parsing, and keep numbers as written (`arbitrary_precision`).
- **`TextLines::lines` reads a line whole.** A file that is one enormous line is read into memory when its
  window is asked for. Ends if lines get a byte cap with a marker for the cut.
- **A table is held in memory.** `Table::parse` keeps every row; a very large CSV costs its size several
  times over. Ends if that matters: index record offsets like `TextLines` does and parse windows.
- **Animations loop forever and a peek decodes every frame to count them.** The container's loop count is
  not read, a JPEG XL animation shows its first frame, and the peek's frame count costs a full decode of
  the animation. Ends if any of those hurts: read the GIF and WebP loop counts, and count frames from the
  container headers.
- **`RasterTarget` has no BMP.** `anyview_image::encode_bmp` exists outside it, for callers that need
  the format. Ends if the export sheet offers BMP (the core target and `RasterExportKind` gain it).
- **The history cap and the pruning rule are not settings yet.** `HistoryCap::DEFAULT` is 200
  files, and `record_view` prunes view memory of vanished, replaced and no-longer-listed files
  with no switch. Ends when quire's `22-SETTINGS` has `viewer.history.*` keys; the caller then
  passes the cap to `StoreWriter::new`.
- **History rows carry a `FormatKind`, not a MIME type.** The launcher's file rows are keyed by
  MIME. Ends when `sill-launcher` maps a kind to its MIME (or `anyview-core` exposes the mapping)
  and merges history into file ranking.
- **A touched file forgets its view memory.** The fingerprint is exact length plus modification
  time, so a `touch` or a restore from backup drops the stored position. Ends if that proves
  annoying: compare length only, or add a content hash of the head.
- **`Zoom::Scale` is not clamped by its variant.** Only `Zoom::scaled` clamps to 1% to 6400%; a
  stored or hand-built `Scale(Permille(0))` loads as written. The raster and PDF stages clamp what
  they restore and what they set, so no state of theirs holds an unclamped scale. Ends when `Scale`
  takes a validated scale type.
- **Proposed values of the machines are not settings yet.** The chrome's idle delay
  (`ChromeParams::HIDE_AFTER`, 2000 ms, quire design/20 section 2.6) should read
  `viewer.chrome.hide_after`; the zoom step (x1.25, `viewer.zoom.step`) and the seek step (5 s,
  `viewer.media.seek_step`) likewise. The reveal and hide fades are the motion token `--t-quick`
  (150 ms) and stay tokens. Ends when quire's `22-SETTINGS` has the `viewer.chrome.*`,
  `viewer.zoom.*` and `viewer.media.*` keys; the view then builds `ViewerParams` from them.
- **The viewer's chords bypass `Shortcut::custom`.** ⌘I is the info panel here, and
  `StandardAction::Italic` reserves it, so `Shortcut::custom` would refuse it. `route` matches
  raw `ShortcutKey`s instead. The viewer edits no rich text, so the clash is harmless in the
  window; it matters if a text field in the viewer ever takes ⌘I. Ends when design/27 names ⌘I
  for the viewer or the panel moves to another chord.
- **Which tabs and which stage a kind has is not decided yet.** `PanelParams::tabs` and the
  `StageFamily` of a probe are handed to the machines; the one match on `FormatKind` that
  produces them belongs in `anyview-core`'s `profile` (`panel_tabs`, `stage_family`), next to
  `stage_support`. Ends when `profile/table.rs` carries both columns.
- **Stage keys are a fixed table.** `StageCommand::from_key` binds `+ = - 0 1 v w Space ⇧← ⇧→
  PageUp PageDown ⌘F ⌘G ⇧⌘G`. Ends when the viewer has a keymap setting; the palette shows the
  same keys.
- **Mini stays when the file stops being media.** Walking the sequence from a video in the mini
  window to an image leaves the presentation `Mini`; `Presentation` only leaves `Mini` on
  `ToWindow`. Ends when the root promotes the window as the stage family changes.
- **Rotating a zoomed image refits it.** A centre in the old orientation has no meaning in the
  new one, so `RasterStage` returns to `Fitted` with the new turn. Ends if a rotated view should
  keep its zoom: the centre then needs rotating about the content's middle.
- **Switching a text file between rendered and source keeps the line number.** Rendered lines and
  source lines are not the same lines. Ends when `anyview-text` exposes the mapping.
- **Find hits are addressed by index.** The stages hold the hit count and the current index; the
  edge keeps the hits and maps an index to a place. A document whose hits change while a find is
  open (a reload) must send `Find` again. Ends if live re-search is wanted.
- **`stage_support` is the viewer's current truth.** Books (EPUB, CBZ), office documents,
  folders and unknown files are `PeekOnly`; every other kind has a stage. Each row changes with
  the stage that lands (`profile/table.rs`).
- **Legacy and unusual types fall to `Other`.** RAR, JPEG 2000, DjVu, JPEG XR, executables and
  the other types `infer` knows but no family holds are `Other` with `infer`'s media type, shown as
  facts and Open With…. Ends per type when a family holds it.
- **`anyview-platform`'s dependencies sit below the pinned block.** `md-5`, `percent-encoding`, `png` and
  `futures-util` are in no pinned list; `zbus` (with `tokio`), `dirs`, `memfd` and `freedesktop-desktop-entry`
  are. Ends at the next change to quire's `docs/workspace-deps.toml`: add the four and copy the block here.
  `ashpd` is not used: the print portal is called through `zbus` as `ds-blitz`'s `print` does, so the portal
  code exists twice (quire's blocking one and this async one). Ends if `ds-blitz` exposes its portal call or
  the viewer prints through it.
- **No clipboard in `Share`.** The plan's "copy" is the window's clipboard, which must outlive the call and
  belongs to the UI crate; `ShareTarget` has only `Mail` (`xdg-email --attach`). Ends when a freedesktop share
  portal exists, or when the window's clipboard is reached through the platform edge.
- **`WindowStacking` has no Linux protocol.** `NoStacking` answers `Unsupported` for `KeepAbove`: Wayland
  clients cannot place themselves, and COSMIC's protocol is not bound. Ends when `anyview-platform` binds a
  compositor protocol (the shell's layer or a COSMIC toplevel-management request) and the binary passes a
  window handle to `request`.
- **Open With is simple.** `apps_for` matches the exact MIME type: no `mime` subclass or alias (so
  `text/x-rust` is not offered the editors of `text/plain`), no `OnlyShowIn`, `TryExec` or `Terminal=true`,
  entries in subdirectories of `applications/` are found by scan but not by id (`kde-foo.desktop`), names are
  untranslated (`Env` carries no locales), and `%f` receives the path, not a URI. Ends when the shared
  MIME database is read (`shared-mime-info`'s `subclasses` and `aliases`) and `Env` gains locales.
- **A started program is not reaped.** `ProcessSpawn` drops the child handle, so an exited Open With or mail
  program stays a zombie until the viewer exits. Ends when the binary installs a `SIGCHLD` reaper or `Spawn`
  hands the child to a systemd scope.
- **MPRIS `Position` is not announced.** The spec says clients poll it, so `publish` emits no `Seeked`; a
  position jump is visible only on the next read. Ends if a client (sill's now-playing) needs `Seeked`:
  `MediaState` then carries a position-jump marker.
- **The thumbnail cache has no `fail/` directory and no `xx-large`.** A file that cannot be thumbnailed is
  retried every time, and the 1024-pixel size of spec 1.0 is not offered. Ends when the launcher's pane
  measures that retry cost or asks for the size.
- **`file_uri` uses Unix path bytes.** `uri.rs` reads the path as raw bytes (`OsStrExt`), so it does not
  build on Windows. Ends when `windows/` is added: `uri.rs` then encodes through `to_string_lossy` for that
  target.
- **The bus tests need `dbus-daemon`.** The bus tests start one with a private configuration and skip
  with a message on stderr when the program is missing, so a machine without it passes without running them.
  Ends when CI is known to have `dbus-daemon`: make a missing one a failure.

## Standing facts

- **`redb` cannot be shared between the viewer and the launcher.** It takes `flock(LOCK_EX)` on
  open and has no shared-reader mode, so the launcher could not read while the viewer ran. The
  store is JSON files replaced by rename instead.
- **A path that is not valid UTF-8 is not remembered.** `StoreWriter::record_view` returns
  `StoreError::PathNotUtf8` before writing anything, because `FilePath` stores through `PathBuf`'s
  serde, which refuses such a path.
- **One writer per store root.** The history is read, changed and rewritten; two writers would
  lose an update and share the `.tmp` name. The viewer is a single instance.

- **`infer` has no signature for** TTC fonts, MPEG transport streams, ICNS, TGA, QOI and HDR; the
  extension names them, and only when the head is binary. A text head is never reclassified by a
  binary extension, so an empty `.png` is plain text.
- **`infer`'s `webm` matcher accepts any EBML header**, and its `mkv` matcher (checked first)
  needs `matroska` in the doctype, so Matroska is recognised by magic and everything else EBML is
  WebM.
- **`infer` with default features pulls `cfb`** (legacy Office OLE containers), pure Rust and MIT,
  which is how `.doc`, `.xls` and `.ppt` are recognised by magic.
- **An iWork package and a plain zip look alike by entry names**; the application (Pages, Numbers,
  Keynote) comes from the file's extension, carried in `ZipProbe`. An iWork layout under a `.zip`
  name is an archive.
- **WebP export is lossless only** (`image` has no lossy encoder) and there is no HEIC or JPEG XL
  export: no pure-Rust encoder exists.
- **`Machine::wake` takes no params.** A machine whose timer length is a setting therefore stores
  the instant it waits for (`Shown { hide_at }`), computed with the setting current when the state
  was entered. A changed setting applies from the next step, which is what `Machine::Params`
  documents.
- **`Machine::In: From<Elapsed>` forces an `Elapsed` input on every machine.** The ones with no
  timer list it among their ignored inputs and return `None` from `wake`.
- **`PageCount::new` is not `const`**, so the PDF stage's tests build their params in a function
  rather than a `const`; every other machine's `CASES` is a `const`.

- **`kamadak-exif` reads EXIF but cannot patch it** (its writer builds a block from scratch and would
  drop the maker notes), so `anyview-image` writes the orientation entry itself: in place when the block
  has one, otherwise by copying IFD0 to the end of the block with the entry added, which moves no other
  offset.
- **`img-parts`' JPEG type stops at the end of image and moves the EXIF segment**, so it would drop the
  bytes some cameras append after the image. `rotate_jpeg` therefore scans the file's own markers and
  replaces one APP1 segment; `img-parts` is used only to splice metadata into a freshly encoded file.
- **Decoded pixels are upright, straight alpha.** Orientation is applied by the decoder (the JPEG XL
  decoder applies its own), so nothing downstream reads the tag, and an encode that carries EXIF resets
  its orientation to upright.
- **JPEG has no alpha.** An export to JPEG composites the picture onto white.
- **The image stack matches quire and sill's locks.** `image` 0.25.10, `png`, `gif`, `zune-jpeg`,
  `weezl`, `image-webp`, `resvg` and `usvg` 0.48.1 and `tiny-skia` 0.12.0 resolve to the same versions;
  `thiserror` 2.0.21, `encoding_rs` 0.8.42 and a few others are patch releases newer in this lock. The
  crates added here introduce four duplicates inside this tree: `miniz_oxide` 0.8.9 (`img-parts`) beside
  0.9.1 (`flate2`), `hashbrown` 0.13.2 (`mp4parse`, reached only through the `avif` feature),
  `getrandom` 0.3 and 0.4 and `r-efi` 5 and 6 (`rand` under `rav1e` beside `tempfile`).
- **JPEG XL fixtures are made with ffmpeg's `libjxl`** (`-distance 0`, lossless); `cjxl` is not
  installed. The AVIF fixture exists but is decoded only under the `avif` feature.
