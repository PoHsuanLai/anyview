//! The viewer as a region of another program's window: what a host gives the pane (its focus, how
//! much chrome it wants, a place to hear that Esc has given the keyboard back) and the link by
//! which the host reads the pane's commands and runs one. The window itself is `window.rs`, drawn
//! with `Presentation::Pane`; nothing here decides what the viewer does.

use super::palette::hit_row;
use super::session::{BRIEF_HITS, WHOLE_HITS};
use crate::{Command, HitIndex, HitLine, HitList, TypedText};
use dioxus::prelude::*;
use ds::components::menus::palette::palette_group::PaletteRow;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// How much of the viewer's own chrome a pane draws. The pane never draws a titlebar, traffic
/// lights, a palette, sheets or a welcome window, whatever this says: those are the host's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PaneChrome {
    /// The content alone: no capsule and no info panel. Right-click still opens the context menu.
    Minimal,
    /// The capsule of controls, shown when the pointer is over the pane.
    WithCapsule,
    /// The capsule and the info panel, which opens from the context menu's Get Info and is an
    /// overlay when the pane is narrow.
    #[default]
    WithPanel,
}

/// What the pane has to say to its host and what the host can ask of it: the commands the open
/// file offers, a way to run one, and a find in the file with its hits. The host makes one with
/// [`use_pane_link`] and hands it to the pane; the pane fills it in. Reading
/// [`PaneLink::commands`] or [`PaneLink::hits`] in a component subscribes it, so the host's
/// palette follows the stage as the file changes and the search answers.
#[derive(Clone, Copy, PartialEq)]
pub struct PaneLink {
    listed: Signal<PaneListing>,
    runner: Signal<Option<Callback<Command>>>,
    found: Signal<PaneHits>,
    finder: Signal<Option<Callback<Option<TypedText>>>>,
}

/// The hits of the find that is up: how many, and the words around the first of them.
#[derive(Debug, Clone, PartialEq, Default)]
struct PaneHits {
    total: u32,
    lines: Vec<HitLine>,
}

/// The commands the pane lists, under the open file's name.
#[derive(Debug, Clone, PartialEq, Default)]
struct PaneListing {
    title: String,
    commands: Vec<Command>,
}

impl std::fmt::Debug for PaneLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaneLink").finish_non_exhaustive()
    }
}

/// A link with nothing listed and no pane behind it yet. Call it once, in the host component that
/// owns the pane; the link is `Copy`, like the signals it holds.
pub fn use_pane_link() -> PaneLink {
    PaneLink {
        listed: use_signal(PaneListing::default),
        runner: use_signal(|| None),
        found: use_signal(PaneHits::default),
        finder: use_signal(|| None),
    }
}

impl PaneLink {
    /// The commands the open file offers now (a rotate on a picture, Zoom to Fit, Share…), in the
    /// order the viewer's own palette lists them; none until a file has opened. Reading it in a
    /// component re-renders that component when the list changes.
    #[must_use]
    pub fn commands(&self) -> Vec<Command> {
        self.listed.read().commands.clone()
    }

    /// The open file's name, which titles its group in a palette.
    #[must_use]
    pub fn title(&self) -> String {
        self.listed.read().title.clone()
    }

    /// Run `command` as the viewer's palette would run the row, on the pane's machine. Nothing
    /// happens when no pane is showing.
    pub fn run(&self, command: Command) {
        let runner = self.runner.try_peek().ok().and_then(|held| *held);
        if let Some(runner) = runner {
            runner.call(command);
        }
    }

    /// Search the open file for `query`, marking the hits in the pane and going to the one nearest
    /// the place shown; the hits arrive as [`PaneLink::hits`] once the search has run. A new query
    /// replaces the last, and an empty one ends the find. Nothing happens for a file that cannot be
    /// searched (a picture) or when no pane is showing.
    pub fn find(&self, query: &TypedText) {
        let finder = self.finder.try_peek().ok().and_then(|held| *held);
        if let Some(finder) = finder {
            finder.call((!query.is_empty()).then(|| query.clone()));
        }
    }

    /// Put the find away: its marks leave the file and [`PaneLink::hits`] lists none. Nothing
    /// happens when no find is up.
    pub fn end_find(&self) {
        let finder = self.finder.try_peek().ok().and_then(|held| *held);
        if let Some(finder) = finder {
            finder.call(None);
        }
    }

    /// The hits of the find that is up as rows of a palette, each yielding `Command::FindHit` for
    /// [`PaneLink::run`] (which makes it the current hit): the words around the match with the
    /// match marked, and where it is (`Line 42`, `Page 7`). `HitList::Brief` lists the first few
    /// and, when there are more, a `Command::ShowAllHits` row that run does nothing with: the host
    /// lists `HitList::Whole` (at most 200) in answer. None while no find is up or nothing was
    /// found. Reading it subscribes the component.
    #[must_use]
    pub fn hits(&self, list: HitList) -> Vec<PaletteRow<Command>> {
        let held = self.found.read();
        let shown = match list {
            HitList::Brief => held.total.min(BRIEF_HITS),
            HitList::Whole => held.total.min(WHOLE_HITS),
        };
        let more = list == HitList::Brief && held.total > BRIEF_HITS;
        let mut rows: Vec<PaletteRow<Command>> = (0..shown)
            .map(|hit| {
                let at = hit as usize;
                hit_row(Command::FindHit(HitIndex(hit)), at, held.lines.get(at))
            })
            .collect();
        if more {
            rows.push(PaletteRow::new(
                Command::ShowAllHits,
                format!("Show All {}", held.total),
            ));
        }
        rows
    }

    /// Replace the hits listed; the same hits wake nobody.
    pub(super) fn publish_hits(self, total: u32, lines: Vec<HitLine>) {
        let mut found = self.found;
        let same = {
            let held = found.peek();
            held.total == total && held.lines == lines
        };
        if !same {
            found.set(PaneHits { total, lines });
        }
    }

    /// The pane is showing: `finder` searches for a query, or ends the find for none.
    pub(super) fn serve_find(self, finder: Callback<Option<TypedText>>) {
        let mut slot = self.finder;
        slot.set(Some(finder));
    }

    /// Replace what is listed; a list that is the same wakes nobody.
    pub(super) fn publish(self, title: String, commands: Vec<Command>) {
        let mut listed = self.listed;
        let same = {
            let held = listed.peek();
            held.title == title && held.commands == commands
        };
        if !same {
            listed.set(PaneListing { title, commands });
        }
    }

    /// The pane is showing: `runner` runs a command on its machine.
    pub(super) fn serve(self, runner: Callback<Command>) {
        let mut slot = self.runner;
        slot.set(Some(runner));
    }

    /// The pane has gone: nothing runs and nothing is listed. The host's signals may be gone first,
    /// which is why this does not insist.
    pub(super) fn withdraw(self) {
        let (mut runner, mut listed) = (self.runner, self.listed);
        let (mut finder, mut found) = (self.finder, self.found);
        if let Ok(mut slot) = runner.try_write() {
            *slot = None;
        }
        if let Ok(mut slot) = finder.try_write() {
            *slot = None;
        }
        if let Ok(mut held) = found.try_write() {
            *held = PaneHits::default();
        }
        if let Ok(mut held) = listed.try_write() {
            *held = PaneListing::default();
        }
    }
}

/// Where a pane sits in its host: the host's focus, whether the keyboard goes back to the host,
/// and the link. The host's `ViewerPane` provides it as a context to the window; a window with
/// none handles keys as if focused and has nobody to give them back to.
#[derive(Clone, Copy)]
#[non_exhaustive]
pub struct PaneSeat {
    pub(super) focused: ReadSignal<bool>,
    pub(super) link: PaneLink,
    pub(super) chrome: PaneChrome,
    pub(super) unfocus: Callback<()>,
    pub(super) lost: Option<Callback<()>>,
}

impl std::fmt::Debug for PaneSeat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaneSeat")
            .field("chrome", &self.chrome)
            .finish_non_exhaustive()
    }
}

impl PaneSeat {
    /// A seat that handles keys while `focused`, fills `link`, and calls `unfocus` when Esc has
    /// nothing left to undo. It draws all the chrome a pane has.
    #[must_use]
    pub fn new(focused: ReadSignal<bool>, link: PaneLink, unfocus: Callback<()>) -> PaneSeat {
        PaneSeat {
            focused,
            link,
            chrome: PaneChrome::default(),
            unfocus,
            lost: None,
        }
    }

    /// The same seat calling `lost` when the pane's root loses the keyboard focus to something
    /// else in the host while `focused` still says the pane has it (a click elsewhere, Tab out).
    /// Focus moving between the pane's own parts is not a loss.
    #[must_use]
    pub fn with_focus_lost(self, lost: Callback<()>) -> PaneSeat {
        PaneSeat {
            lost: Some(lost),
            ..self
        }
    }

    /// The same seat drawing `chrome`.
    #[must_use]
    pub fn with_chrome(self, chrome: PaneChrome) -> PaneSeat {
        PaneSeat { chrome, ..self }
    }
}

/// Wait until the events being dispatched now have all been: a task spawned from a handler is
/// polled only after the focus change's `focusout` and `focusin` have both been heard.
pub(super) async fn after_events() {
    struct Once(bool);
    impl Future for Once {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            if self.0 {
                Poll::Ready(())
            } else {
                self.0 = true;
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }
    Once(false).await;
}
