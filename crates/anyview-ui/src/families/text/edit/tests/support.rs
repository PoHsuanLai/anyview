//! A rig: the editor in a harness window, and the reads and gestures the tests share.

#![allow(clippy::unwrap_used)]

use crate::Wrap;
use crate::families::TEXT_CSS;
use crate::families::text::edit::lines::EditLines;
use anyview_text::{EditText, Session};
use dioxus::prelude::*;
use ds::edit::handle::{EditHandle, use_edit_handle};
use ds::host::position::TextPosition;
use ds::prelude::{Appearance, Ds, Material, Point, Px, Rect, ShortcutKey};
use ds_harness::{Clock, Driver, Harness, HarnessConfig, Input, Query, Viewport};
use std::cell::RefCell;
use std::time::Duration;

#[derive(Clone)]
struct Config {
    text: String,
    wrap: bool,
    rows: u32,
    width: u32,
}

thread_local! {
    static CONFIG: RefCell<Option<Config>> = const { RefCell::new(None) };
    static SESSION: RefCell<Option<Signal<Option<Session>>>> = const { RefCell::new(None) };
    static FIRST: RefCell<Option<Signal<u32>>> = const { RefCell::new(None) };
    static HANDLE: RefCell<Option<EditHandle>> = const { RefCell::new(None) };
}

#[allow(non_snake_case)]
fn Root() -> Element {
    let handle = use_edit_handle();
    HANDLE.with(|slot| *slot.borrow_mut() = Some(handle));
    let config = CONFIG.with(|c| c.borrow().clone()).unwrap();
    let session = use_signal(|| {
        let text = EditText::from_bytes(config.text.clone().into_bytes()).unwrap();
        Some(Session::open(text))
    });
    let mut first = use_signal(|| 0_u32);
    SESSION.with(|slot| *slot.borrow_mut() = Some(session));
    FIRST.with(|slot| *slot.borrow_mut() = Some(first));
    let wrap = if config.wrap { Wrap::On } else { Wrap::Off };
    rsx! {
        Ds { appearance: Appearance::default(), material: Material::Sheet,
            style { {TEXT_CSS} }
            div { style: "width:{config.width}px",
                EditLines {
                    session,
                    handle,
                    doc: None,
                    first: first(),
                    rows: config.rows,
                    wrap,
                    hits: None,
                    current: None,
                    onscroll: move |line: u32| first.set(line),
                    onchanged: |_| {},
                }
            }
        }
    }
}

/// The two display scales the owner's machines run.
pub(super) const SCALES: [u16; 2] = [100, 200];

pub(super) struct Rig {
    pub harness: Harness,
}

pub(super) fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

impl Rig {
    /// The editor over `text`, `rows` lines tall in a window `width` wide, focused by a click.
    pub fn open(text: &str, wrap: bool, rows: u32, width: u32, scale_percent: u16) -> Rig {
        CONFIG.with(|c| {
            *c.borrow_mut() = Some(Config {
                text: text.to_owned(),
                wrap,
                rows,
                width,
            });
        });
        let view = Viewport {
            width,
            height: 400,
            scale_percent,
        };
        let mut harness = Harness::new(Root, HarnessConfig::new(view).with_clock(Clock::Virtual));
        harness.advance(ms(50));
        let mut rig = Rig { harness };
        let first = rig.at(0, 0);
        rig.harness.send(Input::click(first));
        rig.settle();
        rig
    }

    pub fn settle(&mut self) {
        self.harness.advance(ms(60));
    }

    /// `read` of the session the editor edits.
    fn read<R>(&mut self, read: impl FnOnce(&Session) -> R) -> R {
        let session = SESSION.with(|slot| *slot.borrow()).unwrap();
        self.harness.within(|| {
            let held = session.peek();
            read(held.as_ref().unwrap())
        })
    }

    pub fn text(&mut self) -> String {
        self.read(|session| session.buffer().text())
    }

    /// The selection as the range it covers.
    pub fn range(&mut self) -> (usize, usize) {
        self.read(|session| {
            let range = session.selection().range();
            (range.start, range.end)
        })
    }

    /// Where the selection began and where it ends.
    pub fn selection(&mut self) -> (usize, usize) {
        self.read(|session| (session.selection().anchor, session.selection().focus))
    }

    pub fn caret(&mut self) -> usize {
        self.read(Session::caret)
    }

    /// The IME's preedit, while one is up.
    pub fn preedit(&mut self) -> Option<String> {
        self.read(|session| session.preedit().map(|preedit| preedit.text.clone()))
    }

    /// The line at the top of the room.
    pub fn first(&mut self) -> u32 {
        let first = FIRST.with(|slot| *slot.borrow()).unwrap();
        self.harness.within(|| *first.peek())
    }

    pub fn handle(&self) -> EditHandle {
        HANDLE.with(|h| *h.borrow()).unwrap()
    }

    /// The caret box the surface lays out at `line`, `col`, in window coordinates.
    pub fn rect_of(&mut self, line: usize, col: usize) -> Rect {
        let position = TextPosition::new(line.to_string(), col);
        let handle = self.handle();
        self.harness
            .within(|| handle.caret_rect(&position).found())
            .unwrap()
    }

    /// A point just right of the boundary at `line`, `col`, on the middle of its row.
    pub fn at(&mut self, line: usize, col: usize) -> Point {
        let rect = self.rect_of(line, col);
        Point {
            x: Px(rect.origin.x.0 + 1.0),
            y: Px(rect.origin.y.0 + rect.size.height.0 / 2.0),
        }
    }

    pub fn click(&mut self, line: usize, col: usize) {
        // Past the multi-click window, so it is a first click, not the second of a double.
        self.harness.advance(ms(600));
        let at = self.at(line, col);
        self.harness.send(Input::click(at));
        self.settle();
    }

    pub fn press(&mut self, key: ShortcutKey) {
        self.harness.send(Input::key(key));
        self.settle();
    }

    pub fn chord(&mut self, held: &[ShortcutKey], key: ShortcutKey) {
        self.harness.send(Input::chord(held, key));
        self.settle();
    }

    pub fn shift(&mut self, key: ShortcutKey) {
        self.chord(&[ShortcutKey::Shift], key);
    }

    /// A letter with the command key held (Command on this desktop).
    pub fn command(&mut self, c: char) {
        self.chord(&[ShortcutKey::Super], ShortcutKey::Char(c));
    }

    pub fn type_text(&mut self, text: &str) {
        for c in text.chars() {
            let key = if c == ' ' {
                ShortcutKey::Space
            } else {
                ShortcutKey::Char(c)
            };
            self.harness.send(Input::key(key));
        }
        self.settle();
    }

    /// The caret `<div>` the editor drew, in window coordinates.
    pub fn drawn_caret(&self) -> Option<Rect> {
        self.harness.rect(".viewer-caret")
    }

    /// The selection boxes as the page draws them, for a failure message.
    pub fn boxes_html(&self) -> String {
        let html = self.harness.html();
        html.split("<div")
            .filter(|part| part.contains("viewer-selection"))
            .map(|part| part.chars().take(160).collect::<String>())
            .collect::<Vec<_>>()
            .join(" | ")
    }

    pub fn selection_boxes(&self) -> usize {
        self.harness.count(".viewer-selection")
    }
}

pub(super) fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1.0
}

pub(super) const SAMPLE: &str = "fn main() {\n    println!(\"hello world\");\n}\n\nshort\n";

/// The offset line `n` of `text` starts at.
pub(super) fn start_of(text: &str, n: usize) -> usize {
    text.split_inclusive('\n').take(n).map(str::len).sum()
}
