# Findings

This file holds two kinds of entry: **open items**, things still unresolved with the condition
that ends each, and **standing facts**, things true at the current pins that code and tests rely
on. It is a reference, not a log: how each was found lives in git history.

## Open items

- **No `[patch]` sections yet.** sill patches `blitz-kit` to a sibling checkout and the vello and
  anyrender crates to forks; `anyview-core` names none of them and cargo warns about every unused
  patch, so the root `Cargo.toml` carries none. Ends when the first crate that names Blitz, vello
  or anyrender (`anyview-ui`) lands: copy the sections from sill's `Cargo.toml` in that change.
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
- **`FilePath` cannot be written to JSON when it is not valid UTF-8.** It is stored in view
  history through `PathBuf`'s serde, which refuses such a path. Ends when `anyview-store` fixes
  its path encoding.
- **`Zoom::Scale` is not clamped by its variant.** Only `Zoom::scaled` clamps to 1% to 6400%; a
  stored or hand-built `Scale(Permille(0))` loads as written. Ends when the raster stage clamps
  what it draws, or `Scale` takes a validated scale type.
- **`stage_support` is the viewer's current truth.** Books (EPUB, CBZ), office documents,
  folders and unknown files are `PeekOnly`; every other kind has a stage. Each row changes with
  the stage that lands (`profile/table.rs`).
- **Legacy and unusual types fall to `Other`.** RAR, JPEG 2000, DjVu, JPEG XR, executables and
  the other types `infer` knows but no family holds are `Other` with `infer`'s media type, shown as
  facts and Open With…. Ends per type when a family holds it.

## Standing facts

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
