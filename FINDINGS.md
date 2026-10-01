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
- **Only `PlainText` records its encoding.** `FormatDetail::Text` carries the `TextEncoding` the
  head had; Markdown, code and tables do not, so a UTF-16 source file is detected as text but its
  loader must re-detect the encoding. Ends when `anyview-text` decodes and needs the sniffed one.
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
