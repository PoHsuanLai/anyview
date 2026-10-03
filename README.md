# anyview

anyview is a cross-platform, all-in-one file viewer for a macOS-style desktop: images, PDF, text
and code, Markdown, tables, audio and video, fonts, archives and books in one window, and the same
format library as the launcher's preview pane. It is a Rust workspace written in layers, from the
pure vocabulary (`anyview-core`: what a file is, how it is sniffed, the units, actions, edits,
exports and view memory) up to the app, and builds on the design system in the sibling quire
checkout. `ARCHITECTURE.md` is the map, `CONVENTIONS.md` the rules, `FINDINGS.md` the open items.
