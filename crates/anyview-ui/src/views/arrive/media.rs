//! What the player and the PDF worker say, as inputs: notices drained from the player, and a
//! page answer.

use super::super::carry::Carry;
use super::results::send;
use crate::{Stage, StageIn, Ticket, ViewerIn};
use dioxus::prelude::*;

/// The player of `ticket` has news. A wake that comes before the document has landed is not lost:
/// the line keeps what it has, and `landed` drains it.
pub(super) fn media_arrived(c: &Carry, ticket: Ticket) {
    if c.shelf.shown_now().map(|(held, _)| held) == Some(ticket) {
        drain_media(c);
    }
}

/// Read what the player reported: the shelf keeps it for the views, the stage machine hears the
/// part that moves it, and the place the person is at is kept for next time when it has moved.
pub(in crate::views) fn drain_media(c: &Carry) {
    let Some((_, doc)) = c.shelf.shown_now() else {
        return;
    };
    let Some(line) = doc.view().line() else {
        return;
    };
    let notices = line.drain();
    if notices.is_empty() {
        return;
    }
    c.shelf.media.apply(&notices);
    for notice in &notices {
        if let Some(input) = stage_input_of(notice) {
            send(c, ViewerIn::Stage(StageIn::Media(input)));
        }
        if let Some(walk) = walk_of(notice) {
            send(c, ViewerIn::Navigate(walk));
        }
    }
    remember_place(c);
}

/// The input the stage machine takes from a piece of the player's news, if it takes one.
fn stage_input_of(notice: &crate::MediaNotice) -> Option<crate::MediaIn> {
    match notice {
        crate::MediaNotice::Player(event) => Some(crate::MediaIn::Player(*event)),
        crate::MediaNotice::Position(at) => Some(crate::MediaIn::Position(*at)),
        crate::MediaNotice::Failed(error) => Some(crate::MediaIn::Failed(*error)),
        crate::MediaNotice::Tracks(_)
        | crate::MediaNotice::Chapters(_)
        | crate::MediaNotice::Speed(_)
        | crate::MediaNotice::Abilities(_)
        | crate::MediaNotice::Picture(_)
        | crate::MediaNotice::Next
        | crate::MediaNotice::Previous => None,
    }
}

/// The walk along the folder's files that the player's news asks for, if it asks.
fn walk_of(notice: &crate::MediaNotice) -> Option<crate::NavigateIn> {
    match notice {
        crate::MediaNotice::Next => Some(crate::NavigateIn::Next),
        crate::MediaNotice::Previous => Some(crate::NavigateIn::Previous),
        crate::MediaNotice::Player(_)
        | crate::MediaNotice::Position(_)
        | crate::MediaNotice::Tracks(_)
        | crate::MediaNotice::Chapters(_)
        | crate::MediaNotice::Speed(_)
        | crate::MediaNotice::Abilities(_)
        | crate::MediaNotice::Picture(_)
        | crate::MediaNotice::Failed(_) => None,
    }
}

/// Keep where the person is in the recording when it is not where it was last kept (to the
/// second, or in its settings), while the recording is open and not still opening, and once the
/// player has said where playback is: until then it has not applied the place the person left.
fn remember_place(c: &Carry) {
    let mut shelf = c.shelf;
    let Some(dispatch) = c.dispatch() else {
        return;
    };
    let opening = matches!(
        dispatch.machine.state().peek().stage,
        Stage::Media(crate::MediaStage::Opening)
    );
    if opening {
        return;
    }
    let Some(place) = shelf.media.place() else {
        return;
    };
    let kept = crate::MediaPlace::of(&shelf.left_at.peek());
    if kept.is_none_or(|kept| !kept.same_as(place)) {
        shelf.left_at.set(place.resume());
        c.edge.request(crate::HostRequest::Remember(place.resume()));
    }
}

/// What a PDF worker made: the shelf takes it in, and the stage machine hears of it when it must.
/// An answer for a file the person left is dropped by its ticket.
pub(super) fn pdf_arrived(c: &Carry, ticket: Ticket, answer: crate::PdfAnswer) {
    if c.shelf.probe.peek().ticket() != Some(ticket) {
        return;
    }
    let Some(dispatch) = c.dispatch() else {
        return;
    };
    let stage = dispatch.machine.state().peek().stage.clone();
    if let Some(input) = c.shelf.pdf.arrived(answer, &stage) {
        dispatch.send(ViewerIn::Stage(input));
    }
}
