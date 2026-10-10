# Changelog

All notable changes to anyview. The format follows Keep a Changelog; versions follow semantic
versioning, with pre-releases while the program is in beta.

## Unreleased

### Resume

- Where you left off in a file is now safe when two programs that use Viewer run at once, such as Viewer and a terminal with a viewer pane in it: each remembers its own files without erasing the other's.

### Shortcuts

- Shortcuts follow your system's keyboard settings. Command on a Mac-style desktop and Ctrl on Windows and most Linux desktops now mean what that system says; change a shortcut in the system and Viewer follows it. Ctrl is no longer Command on a desktop where Command is its own key.
- Zoom uses ⌘+ and ⌘−, and the bare + and − keys still work. As in Preview, Actual Size is ⌘0 (or 0), Zoom to Fit is ⌘9 (or 9), and Fit Width is 2. Get Info is ⌘I.
- Get Info is ⌘I, as in Preview; the viewer never styles text, so it gives Italic's chord up. The next and previous sheet of a workbook are ⌘Page Down and ⌘Page Up, so ⌘[ and ⌘] only turn a picture.
- The command palette, Get Info, Copy Path, Duplicate, Move to Trash, Export, the page edits and the rest show the key the system gives them, in the palette and in the menus.

### Closing

- The window's close button, and the close the desktop asks for, now ask to save as Command W does when a text or a picture has changes you have not saved: Save, Don't Save or Cancel. A window with nothing to save closes at once.
- The buttons of the bottom bar show their key in the tooltip, after the name of the button.

### Text

- You can edit plain text, code and Markdown files. Press Return, or choose Edit from the command palette or the bottom bar, to type in the file where you are reading it; Done finishes. The usual things work: the caret and selection, arrows with Option or Control for words and Command for the line, Home and End, Select All, Cut, Copy and Paste, Undo and Redo, and typing with an input method (Chinese, Japanese, Korean). Typing a word, a run of Backspaces and a paste each undo as one step. A Markdown file is edited in its source.
- Command S saves. The original is kept first, so Revert To can bring it back; the file keeps its line endings and, if it had one, its byte-order mark. A file that is not UTF-8 text, or is too large, is not edited, and Viewer says why in words.
- The title bar shows a dot after the name while there are changes you have not saved. Closing with Command W, opening another file or finishing with Done asks "Do you want to save the changes made to “name”?" with Save, Don't Save and Cancel. If another program changes the file while you are editing, Viewer tells you; with no changes of yours it reads the file again, and with some, Save asks before replacing what the other program wrote.
- Find works on the text you are editing, and its matches follow as you type.
- Long lines wrap in plain text and Markdown and run on in code. The Toggle Wrap command in the palette (or the bottom bar's button) changes it, and Viewer remembers your choice for the next file of the same kind.
- A Markdown file has a Preview | Source control at the right end of the title bar, next to where a picture has Select | Pan, so the preview is easy to find; the V key still switches. Source is set in the code face with a line-number gutter and room at the sides, and the preview reads in the interface face at a comfortable width, with headings in the display face.

### Tables and JSON

- A CSV, TSV or spreadsheet sheet is a table as in Numbers: a row-number gutter, a header that stays put (a hairline appears under it once the rows scroll), hairline rules between the columns, numbers at the right edge of their columns, and columns you can drag wider or narrower. Click a header to sort by it, again to reverse, and once more to go back to the file's own order; sorting is only how you look at the sheet and never changes the file. A two-hundred-thousand-row file still costs a screenful.
- A JSON tree uses the same row height and face as a table: keys in the ordinary text colour, values tinted by what they are, and the counts of objects and arrays faint.
- A workbook's panel has a Sheets tab with a row for each sheet, beside Info. When a file is longer than what Viewer shows, the panel says "Showing the first 200000 rows" at its foot instead of putting a banner over the rows.

### Left panel

- The side panel is now a pane on the left of the window, as in Preview and Finder. It stays open from file to file on the tab it was on (or the first tab, when the next file has not got that one), you can drag its edge to make it wider or narrower or to fold it away, and opening it makes the picture smaller rather than the window bigger. Page thumbnails fill its width.
- Get Info is now a card: the file's icon or picture, its name, one line saying what it is, and a Details section that lists everything else with each caption above its value. A file Viewer cannot show uses the same card in the middle of the window.

- A PDF or a book opens with its pages beside it, as in Preview, until you open or close the panel yourself; after that it stays as you left it from file to file. A file Viewer can only describe, such as an archive or a font, has no panel to open, because the window already shows its card.

### Pictures

- A picture can be cut, resized, turned and mirrored, and the changes wait until you save them. Crop sits beside Select and Pan in the title bar (or press C): drag the corners and edges of the rectangle, choose Free, Original, Square, 4:3 or 16:9 (landscape or portrait), and press Return to cut or Esc to leave it. The title bar shows a dot while the picture has changes that are not saved.
- Adjust Size… (in the command palette) opens a dialog with the width and height in pixels or as a percent, kept in proportion unless you untick it.
- Rotate and flip no longer write the file at once: they turn what you see, and Command S saves the picture in its own format, keeping its details except the turn, which is now in the picture itself. Command Z and Shift-Command Z take back and redo the steps first. A picture Viewer cannot write offers Export… instead.
- Closing the window or opening another file asks whether to save the changes: Save, Don't Save or Cancel.

### Find

- Find is in the command palette. Command F opens it as a find, with the last find's text selected: what you type is searched for in the file, the matches are marked behind the palette, and the places are listed under "In This File" (with the line or page), followed by the commands that match. Moving through the list shows each match, Return goes to it and closes the palette, and Esc clears the marks. The bottom bar then reads "3 of 17" with Previous Match and Next Match buttons, and Command G and Shift-Command G step through the matches as before. The find bar is gone.

### Tooltips

- Tooltips are short: a name in title case, then the key, as in "Pointer Tool  H". The buttons on the bottom bar now begin each word with a capital ("Zoom In", "Next Page"), and a PDF's page reads "3 of 12".

### Design system

- Viewer is built on quire v0.3.1. The Select | Pan control is now part of the title bar, at its right end: clicking it never drags the window or zooms it, and it fades with the bar. A window that is snapped or tiled beside another is left at the size the desktop gave it when you open another file (this takes effect once the windowing layer reports tiling, which it does not yet on Linux). If the window system cannot start at all, Viewer now says so and exits with an error. Tooltips show as soon as the pointer is over a button. Shortcuts written with the command key now also answer to the Super key, as a window reports it. A right-click over a picture, a rendered Markdown page or a book now always opens the menu, and so does one where the hand moved slightly. Rotate Left and Rotate Right have a matching pair of icons. Copy Path reaches the clipboard on a Wayland desktop and stays there. The wheel speeds up when you spin it, the page moves in the same frame as the wheel, a menu or the palette taller than the window scrolls inside it, a long file name in the titlebar keeps its extension, and tooltips show in a real window.

### Menus

- The context menu, the command palette and the capsule list from one declaration. Each kind of file names the actions it has, each stage names what it can do to its file (a picture turns and flips, a PDF turns the page you are on, a recording plays), and a command is listed only where both say yes. Nothing is listed that would do nothing, and a read-only file loses its rotate buttons as well as its menu rows.
- Open With is gone from the menu, the palette, the keys and the screens of files that cannot be shown. Viewer is the viewer; a file it cannot show says so in words, with Show in Folder where there is a file manager.
- The Open… dialog starts on "Supported files", the types Viewer can show, with "All files" beside it. The desktop hides the other files rather than greying them out.
- The desktop entry names the types by the registry's own names (`audio/vnd.wave`, `image/heif`, `image/qoi`…), so file managers offer Viewer for them.

### Audio

- An audio file shows its cover art: ID3, FLAC, MP4 and Ogg pictures are all read. A file with no cover shows a plain music tile. Audio opens in a small window, and the side panel is never blank for it. Now Playing on the desktop shows the track with its cover, and the media keys skip between tracks.

### Books

- EPUB and CBZ open in the PDF view. A book's chapters are laid out on reading pages and bound into one PDF with the chapters as its outline, so a book scrolls, zooms, finds and prints as a PDF does. A book opens at reading width, as Books and Preview do, unless you left it somewhere else. Its pages cannot be edited, because they are not a file.

### Info panel

- The Info panel lists what a file says about itself, in sections: a picture's colour, density and (for a photo) camera, lens, exposure, flash, time and software, and where it was taken; a PDF's title, author, dates, producer, version, page size, protection and signatures; and for every file a General section, last: its kind in a person's words ("JPEG image", "Rust source"), its size with the exact bytes, when it was made and changed, the folder it is in, where it was downloaded from (host first, without the query) and its permissions. Where a photo was taken is shown here and nowhere else: the launcher's preview never lists it, and a camera's serial numbers are never read.

### Plugins

- The plugin machinery that has nothing to do with what a plugin offers is now bayonet (`github.com/PoHsuanLai/bayonet`, MIT OR Apache-2.0), a library of its own, so other apps can share it: the length-prefixed JSON frames, the manifest's shared fields, discovery in `anyview/plugins`, which plugin wins when manifests collide, the package suggestion, and starting a plugin with its timeouts, cancel and kill. anyview keeps its own capabilities (probe, peek, thumbnail, decode, export, play) and messages on top of it. Nothing changes for you: the manifests, the protocol, the `Needs:` lines and the way a crashed or silent plugin costs one request are as they were.

### Agents

- Viewer can be asked to open files by the desktop's agent layer (docket): `anyview.file.open` opens one or
  several files in the running window, or starts Viewer to show them. It only shows files; nothing an agent can
  ask edits or deletes one. A `files-viewer` skill tells the assistant how.

### Naming

- The program is called Viewer everywhere you see it: the welcome window, the helper sheet ("Viewer needs mpv to play videos."), the now-playing entry and the desktop entry. One constant, `anyview_core::APP_NAME`, holds the name; the binary, the app id and the bus names stay `anyview`, `org.quire.Anyview` and `org.quire.Anyview1`.

### Windows

- A window opens sized to its content, as Preview and QuickTime do: a picture with one image pixel to one
  screen pixel (on a 2x screen a 1200 by 800 picture opens in a 600 by 400 window), a video at its
  resolution, scaled down to fit when it is larger than 85% of the screen less the top bar (a 1920 by 1200
  screen is assumed before the screen is known) and never smaller than 480 by 320 (a small picture stays
  centred). The screen is the one the window is on, at its own scale, once the window is open. A PDF, and a picture the
  HEIC or RAW plugin decodes, take the window to their page or picture size as soon as they have loaded.
  Everything else, and a file that does not open, takes the usual size.
- Every different file starts fresh, as in Preview: the window takes the new file's size (a window you
  resized, or the last file's, does not hold; a maximized or fullscreen window is left as it is), and its zoom,
  pan, scroll, find, open menus and sheets are the file's own. Opening the same file again puts it back where you
  left it. The side panel stays open from file to file, on the same tab when the new file has it and on its first
  tab when not.
- Dragging a zoomed picture pans it, as in Preview, Photos and Loupe. Select | Pan, in the titlebar's trailing
  corner as in Preview, chooses between that and Select (H, or "Use Select" and "Use Pan" in the command palette);
  the choice stays as you go from file to file, and the picture's place does not. Holding Space pans for as
  long as it is down on a still picture (it plays an animation), and lets go when the window loses the keyboard.
- The mouse wheel scrolls PDFs, pictures and text smoothly: each notch eases over a fraction of a second
  instead of jumping, and notches in a burst add up.
- quire v0.2.20: a touchpad scrolls the PDF, pictures and text with the fingers and carries on with the glide
  after a fast flick, as a native scroll does; Control with the wheel zooms one step a click.
- The playback bar thins itself to the window: in a narrow window it drops the export, the speed, the length, the
  volume and then the clock, in that order, and keeps play, the seek buttons and the progress bar. A PDF's
  and a picture's bars drop their least needed buttons the same way. The window's least size is still 480 by
  320, and what a narrow bar hides stays in the command palette and the right-click menu.

### Look

- quire v0.2.23 to v0.2.30, with docket and porter moved to the revs built on them: a titled button also carries its title as an accessible description when it says more than the button's name; nothing else visible changes in Viewer (the Spaces kit and the consent alert's session offer are not used).
- quire v0.2.21: small icons are solid (filled) by default, a menu returns focus to what opened it, and the video controls' progress bar can be as narrow as 120 px, so the export button now shows on a window wide enough for the whole control bar (672 px and up).
- quire v0.2.19's Tahoe sizes: controls are 20, 24, 28, 32 and 40 tall (the capsule's buttons are the 32 Large,
  with an 18 px glyph), windows are rounder, and table and tree rows are quire's 32 px compact row.

### Platform

- What the platform cannot do is not offered. Open With, Share, Print, Open... and Show in Folder are left out
  of the palette, the right-click menu, the buttons on a card or a file that did not open, the welcome
  window and the keys (⌘O, ⌘P, ⌥⌘O, ⌘R) where the desktop service behind them is absent. The Install... of a
  missing tool is offered only where the system can install packages.
- The missing-tool sheet takes the package and program it names from the installer's own answer.

- The viewer's Linux desktop services are now an optional cargo feature, `quire-desktop` (on by default). A
  build with `--no-default-features` has no D-Bus code of its own: single instance goes through a per-user
  socket (latchkey), links and Show in Folder through the platform's opener, and Open With, Share, Print, the
  file chooser and the now-playing entry are absent and say so. Builds with the default features behave as
  before.

### Install

- `dist/install.sh` installs the `ffmpeg`, `heif` and `raw` plugins with the viewer, and builds the `mpv`
  plugin too: from `--with-mpv-from DIR`, `MPV_WGPU_DIR` or `../mpv`, else from mpv-wgpu's pinned revision,
  fetched with git into the cache. A plugin whose tool is not installed yet still installs and starts working
  once the package is there. If there is no mpv, git, network or build for mpv, only that plugin is skipped,
  with a one-line warning.
- With no mpv on the search path the mpv plugin still installs, naming `/usr/bin/mpv`, instead of being
  skipped: it works once mpv is installed (Install… offers it).
- `--without-plugin NAME` and `--no-plugins` replace `--with-plugin NAME`, which is accepted and ignored.

### Added

- Viewer offers to install a missing tool, as Totem offers a codec. A recording with no mpv, a HEIC with no
  libheif tools and a camera raw file with no LibRaw tools keep the "Needs" line and gain an Install… button
  on it; it asks "Viewer needs libheif tools to open HEIC photos.", and on Install the system's package
  service (PackageKit) installs the package and asks for the password itself. When it is done the file opens
  again in the same window with no restart, and a tool installed in a terminal is noticed the same way. Not
  Now closes the question and asks nothing more until the button is pressed again; where the system cannot
  install (a Flatpak sandbox, no PackageKit) or has no such package, the sheet says what to install.
  `dist/helpers/anyview.toml` names the packages for Fedora, Debian and Ubuntu, Arch and openSUSE and is
  installed to `share/quire/helpers/anyview.toml`.
- Audio plays without mpv. MP3, AAC (ADTS and M4A), ALAC, FLAC, WAV, AIFF and Ogg Vorbis are decoded in the
  viewer (pure Rust) and played through the sound card, with play and pause, seeking, volume, the resume
  position, the desktop's now-playing entry and media keys, and background playback. With the `mpv` plugin
  installed mpv still plays everything. Video and Opus audio still need mpv, and a machine with no sound
  output says so instead of playing.

### Changed

- The viewer's state machines (chrome, panel, palette, sheet, navigation, presentation, loading, the stages, key routing and the root) are a library of their own, `anyview-machines`, which depends on `anyview-core`, `ds-core` and `chordkit` and not on Dioxus or the window, so another app can drive them without compiling a window. `anyview-ui` re-exports every name, so nothing changes for you or for a caller. The viewer's package budget is 694 (one package more, the new crate); `anyview-machines` is 34.
- The package budget of the viewer is 666 (the sound card library adds four packages and the missing-tool
  prompt two, ds-shell and ds-helpers; the launcher is unchanged). Building from source needs the ALSA development files (`libasound2-dev`, `alsa-lib-devel`).

## 0.1.0-beta.1 (2026-10-06)

The first beta.

### What it opens

- Images: PNG, JPEG, GIF, WebP, BMP, TIFF, ICO, TGA, QOI, SVG, JPEG XL, OpenEXR, Radiance HDR, PSD and
  ICNS, with animated pictures played by the clock. HEIC and AVIF through the `heif` plugin; camera raw
  files show their embedded preview, and are developed in full through the `raw` plugin.
- PDF documents, with page turning, zoom and printing.
- Text and source code with syntax highlighting, Markdown, CSV and TSV tables, spreadsheets (XLSX, ODS),
  JSON and JSONL as trees, and facts for office documents.
- EPUB books and CBZ comics, fonts (including WOFF), and archives (ZIP, tar, 7z and others) listed and
  read in place.
- Video and audio, played by your own mpv through the `mpv` plugin, with facts, pictures and
  conversion through the `ffmpeg` plugin.

### What it does

- One window for everything, and the same format library as the launcher's quick-look pane.
- Edits to pictures save in place with undo, redo, Revert To and Save a Copy; the original is kept first.
- Export and print for images, PDF and text; clips and audio extraction through the `ffmpeg` plugin.
- Follows the desktop's light and dark appearance, accent colour and reduced motion live.
- Opens files given as paths or `file://` URIs, forwards later opens to the running window through D-Bus,
  and stays warm for ten minutes after the last window closes.
- No codec library is linked: playback of video, conversion, HEIC and raw development run your own tools as separate programs.

### Release basics

- `anyview --version` and `--help`; the plugins answer `--version` too.
- A panic prints a short report (thread, message, location, version) and keeps the last ten under
  `$XDG_STATE_HOME/anyview/crash/`.
- `THIRD-PARTY-NOTICES.md` for every crate built in, installed under `share/doc/anyview`.
- AppStream metainfo and a corrected desktop entry category.
- `cargo deny` passes in full, with each accepted advisory explained in `deny.toml`.
- quire, blitz-kit and the other repositories are pinned git dependencies: a fresh clone builds with
  `cargo build --locked`.
