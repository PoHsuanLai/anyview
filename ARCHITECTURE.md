# Architecture

anyview is the viewer: one program that opens every common file type (images, PDF, text and
code, Markdown, tables, audio and video, fonts, archives, books, office documents) and the format
library the launcher shares, so a file looks the same in the launcher's pane and in a window. This
file is the map: which crate owns what, what may depend on what, where each concept lives, the
exact shape of each trait, and the commands that gate a change. `CONVENTIONS.md` holds the rules
every repo shares; where it and this file disagree, this file wins for anyview.

anyview is a sibling checkout of quire (`../quire`) and takes its design-system crates by path,
the way sill does (quire `CONSUMING.md` section 1). The pinned block of `[workspace.dependencies]`
in `Cargo.toml` is copied verbatim from quire's `docs/workspace-deps.toml`; a new dependency is a
change to that file first.

## 1. Crates and allowed edges

Layers go lowest first. A crate names only crates in a lower layer, and
`scripts/check-boundary.sh` enforces the edges and the external boundaries below. A crate marked
planned has no directory yet; its row is the rule it will carry.

| Layer | Crate | Status | Purpose |
| --- | --- | --- | --- |
| L0 | `anyview-core` | exists | pure vocabulary: kinds, sniffing, units, sequence, actions, edits, exports, view memory, the `Peek` trait |
| L1 | `anyview-store` | planned | view memory and history file: one format, a read API (sill reads it) and a write API |
| L1 | `anyview-image` | planned | decode, encode, EXIF, orientation |
| L1 | `anyview-pdf` | planned | pdfrum: open, tile scheduling, search, outline, page edits, exports |
| L1 | `anyview-media` | planned | the media player session: tracks, chapters, typed state |
| L1 | `anyview-text` | planned | text, code highlighting, Markdown to HTML, CSV, JSON tree |
| L1 | `anyview-archive` | planned | zip, tar and 7z listings and single-entry extraction |
| L1 | `anyview-font` | planned | font facts and the specimen's font face |
| L2 | `anyview-platform` | planned | the edge: traits, their Linux implementations and fakes |
| L3 | `anyview-peek` | planned | the light tier: `Peek` implementations and the pane view (what the launcher links) |
| L4 | `anyview-ui` | planned | the viewer's pure machines and its views: stages, chrome, palette, panel, sheets |
| L5 | `anyview` | planned | the binary: launch, single instance, CLI, wiring the platform |

### Allowed edges (workspace crates and quire; everything else is forbidden)

| Crate | May depend on |
| --- | --- |
| `anyview-core` | `ds-core` (its `#[derive(Word)]` is re-exported by `ds-core`, so `ds-core-derive` is not an edge) |

Dev-dependencies follow the same table, plus `serde_json` for round-trip tests and `ds-core` with
its `testing` feature for `word_matches_serde`.

### External boundaries (`scripts/check-boundary.sh`)

| Crate | Never reaches |
| --- | --- |
| `anyview-core` | `dioxus`, `tokio`, `zbus`, `wgpu`, `pdfrum`, `mpv-wgpu-player`, `rsmpv`, `image`, `syntect`, `blitz-dom`, `anyrender`; `serde_json` outside tests |
| `anyview-peek` (planned) | `mpv-wgpu-player`, `rsmpv`, `wgpu`, `zbus`: libmpv and D-Bus stay out of the launcher's process; pdfrum only through `ds-blitz`'s `pdf` feature |
| `anyview-ui` (planned) | `zbus`: platform code arrives through `anyview-platform` traits |
| every crate but `anyview-platform` (planned) | `zbus`, `ashpd`, `freedesktop-*`, and the macOS and Windows bindings |

`anyview-core` depends on `serde`, `thiserror`, `infer` and `ds-core` and nothing else. It does no
I/O and reads no clock: a function that needs bytes takes them (`FileHead`, `ZipEntries`), and one
that needs the time takes it.

## 2. Modules inside `anyview-core`

`lib.rs` declares private modules and re-exports each public item once at the crate root; there is
no other public path. A module names only modules above it in this list.

| Module | Holds |
| --- | --- |
| `error` | `CoreError`, the crate's one error |
| `units` | page, media, ratio, zoom, turn, content-space and pixel newtypes; all integer |
| `source` | `FilePath`, `FileName`, `FileStamp`, `Source` |
| `kind` | `FormatKind`, `Mime`, `FormatDetail`, `SyntaxName` and the format families |
| `sniff` | `sniff`, `sniff_zip`, `Sniffed` and the head and entries they read |
| `sequence` | `NonEmpty`, `Sequence`, `moved`, `neighbours` |
| `edit` | `Edit`, `EditKind` |
| `action` | `FileAction`, `Reach`, `reach`, `shortcut` |
| `export` | the per-format export enums, `ExportChoice`, `ExportJob` and its payloads |
| `resume` | `Resume`, `TrackChoice` |
| `facts` | `FactLabel`, `FactValue`, `Facts` |
| `peek` | `Peek`, `PeekBudget`, `StageSupport` |
| `profile` | the one match on `FormatKind`: `actions_for`, `edits_for`, `stage_support` |

## 3. Layer rules

1. **A lower layer never names a higher one.** If something needed lives above, move the shared
   piece down or pass it in.
2. **`anyview-core` is pure.** Effects (reading files, listing a zip, decoding, playing) belong to
   crates above it, which hand it values: the first 4 KiB of a file, a zip's entry names.
3. **Every kind of file is mapped from day one.** A kind without a viewer stage is `PeekOnly` (facts
   plus Open With…), never a stub.
4. **No `_` arm on our own enums.** The workspace turns on clippy's `wildcard_enum_match_arm` and
   `match_wildcard_for_single_variants` at `deny`. An input a state ignores is listed by name, so
   adding a variant is a compile error everywhere it matters.
5. **One exhaustive match per enum, in one file.** `FormatKind` is matched in `profile`;
   `FileAction` in `action`'s spec; each format family in its own file behind the `Family` trait.
   A second match elsewhere is a missing method.
6. **Typestate where a lifecycle is linear and owned in one place.** `Sniffed` can only be made by
   sniffing, so a loader that takes one is never handed a kind guessed from a name alone.

## 4. One home per concept

The single place a concept lives. Extend it; never write a second one.

| Concept | Home |
| --- | --- |
| Closed vocabulary (slug, label, ALL, parse) | `ds_core::word::Word` and `#[derive(Word)]` |
| Shortcuts and the reserved standard table | `ds_core::vocab::Shortcut`, `ds_core::standard_action::StandardAction` |
| What a file is (kind, MIME, detail) | `anyview_core::FormatKind`, `Mime`, `FormatDetail` |
| Sniffing, magic bytes first, extension as the fallback | `anyview_core::sniff`, `sniff_zip` |
| Which extensions and MIME types a format family has | `Family` in `kind/family.rs`, one file per family |
| Which actions, edits and stage a kind has | `anyview_core::actions_for`, `edits_for`, `stage_support` (`profile`) |
| Where an action may appear, and its keys | `anyview_core::reach`, `shortcut` (`action/spec.rs`) |
| Page, time, volume, zoom, turn, dpi, pixel units | `anyview_core` units (`units`) |
| The list the arrow keys walk | `anyview_core::Sequence`, `moved`, `neighbours` |
| Where a person left a file | `anyview_core::Resume` |
| What each format exports, and the sheet's contract | `anyview_core::ExportChoice` and the per-format enums (`export`) |
| The shared encoders an export becomes | `anyview_core::ExportJob` |
| Rows of facts a pane lists | `anyview_core::Facts` |
| The light tier of a format | `anyview_core::Peek` |
| The crate error | `anyview_core::CoreError` |

## 5. The canonical traits

A trait exists where two or more implementations swap or a generic consumer runs over many
(CONVENTIONS section 5). Closed sets stay enums.

```rust
/// The light tier of one format: cheap, no GPU, no libmpv. One implementation per kind.
pub trait Peek: 'static {
    const KIND: FormatKind;
    type Peeked: Clone + PartialEq + Send + 'static;
    type Error: std::error::Error + Send;
    /// Blocking; run on a worker. Bounded by `budget` (bytes read, pixels decoded, time).
    fn peek(src: &Source, sniffed: &Sniffed, budget: &PeekBudget) -> Result<Self::Peeked, Self::Error>;
    fn facts(peeked: &Self::Peeked) -> Facts;
}

/// One format's closed set of exports. The export sheet is generic over it.
pub trait ExportChoice: Clone + PartialEq + 'static {
    type Kind: Word;
    fn kinds() -> &'static [Self::Kind] { Self::Kind::ALL }   // the format pop-up's entries
    fn kind(&self) -> Self::Kind;                            // the pop-up's current entry
    fn default_for(kind: Self::Kind) -> Self;                // options pre-filled
    fn extension(&self) -> ExportExtension;
}
```

`anyview-core` holds no registry and no visitor over `Peek`: it declares the trait and cannot name
its implementations. The light tier's registry (one exhaustive match over `FormatKind`) lives in
`anyview-peek` and the stage registry in `anyview-ui`.

Planned with the crates that need them: `Stage` (anyview-ui; the viewer's full tier, extending
`Peek`) and `Machine` (quire's pure state machine trait; every viewer region implements it).

## 6. Recipes

**Add a kind of file.**
1. Add the variant to `FormatKind` (`kind/mod.rs`) and its row in `profile/table.rs`.
2. If it has formats, add a family enum implementing `Family` (`kind/<family>.rs`), a `FormatDetail`
   variant, and its lookup in `sniff/extension.rs`.
3. Add its cases to `sniff/tests.rs` and `profile/tests.rs`.

**Add a format to a family.** Add the variant with its extensions and MIME type in the family's
file; its `the_table_is_well_formed` test checks them.

**Add an action.** Add the variant to `FileAction` and its reach and binding in `action/spec.rs`;
list it in `profile/table.rs` for the kinds that offer it; extend the tables in `action/tests.rs`.
A binding that means what a standard action means uses `Binding::Standard`.

**Add a format's export.** Write its enum and its `Kind` enum in `export/<format>.rs`, implement
`ExportChoice`, and add it to the contract test in `export/tests.rs`. Name its output in
`ExportJob` payloads only if no existing job covers it.

**Add a peek.** Implement `Peek` in `anyview-peek` and add its arm to that crate's registry.

## 7. Tests

Pure functions get one `const CASES` table and one loop, each row named, so a failure names the
row. Stored types (`Resume`, `FileAction`, `Zoom`, `Edit`, `FilePath`, …) have a round-trip test
against the exact JSON, and a stored `Word` enum is checked against its serde form with
`ds_core::testing::word_matches_serde`. Fixtures are small byte literals of real file signatures,
in the test that uses them. A `Peek` fake in `peek/tests.rs` shows how a generic consumer drives the
trait.

## 8. Repo rules

- **Effect boundary** (`scripts/check-boundary.sh`): the tables in section 1. A pure crate that
  seems to need an effect returns a description of it to its caller.
- **Gate:**

  ```bash
  cargo fmt --all --check
  cargo clippy --workspace --all-targets --all-features -- -D warnings
  cargo test --workspace
  ./scripts/check-boundary.sh
  cargo deny check licenses
  ```

- **No `unsafe`** anywhere in the workspace; `unsafe_code = "deny"`.
- **No `unwrap`** outside tests: clippy's `unwrap_used` is `deny`, and `clippy.toml` allows it in
  tests only.
- **Stored forms:** a type with `Serialize` is stored or crosses a wire, is adjacently tagged when
  it has data, and has a round-trip test. `FilePath`, `Mime`, `SyntaxName`, `PageRange`, `Dpi`,
  `Quality` and `Volume` validate on load, so a stored value cannot hold what a constructor
  refuses.
- **Integer units only** in data: a float appears inside arithmetic and nowhere in a type.
