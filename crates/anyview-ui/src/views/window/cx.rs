//! The stage's context: the machine's state and the shelf's results as values, and the inputs the
//! stage sends back as callbacks.

use super::super::shelf::Dispatch;
use crate::families::{Area, FrameLook};
use crate::io::{FileAccess, HostRequest, Job};
use crate::{Command, MarkupIn, PictureEditIn, StageCx, StageIn, Ticket, Viewer, ViewerIn};
use dioxus::prelude::*;

/// The context the window's stage is drawn from, for a window or (`hosted`) a pane, which changes
/// nothing in the file. `attributes` are the frame's theme attributes.
pub(super) fn stage_cx(
    state: &Viewer,
    dispatch: Dispatch,
    edge: &crate::Edge,
    ticket: Ticket,
    area: Option<Area>,
    attributes: String,
    hosted: bool,
) -> StageCx {
    let shelf = dispatch.shelf;
    let worker = edge.clone();
    let requester = edge.clone();
    let lines_edge = edge.clone();
    StageCx {
        stage: state.stage.clone(),
        hand: state.hand,
        picture: state.picture.clone(),
        edit: EventHandler::new(move |input: PictureEditIn| {
            dispatch.send(ViewerIn::Picture(input));
        }),
        trim: state.trim.clone(),
        trim_in: EventHandler::new(move |input: crate::TrimIn| {
            dispatch.send(ViewerIn::Trim(input));
        }),
        markup: state.markup.clone(),
        mark: EventHandler::new(move |input: MarkupIn| dispatch.send(ViewerIn::Markup(input))),
        ticket,
        area,
        send: EventHandler::new(move |input: StageIn| dispatch.send(ViewerIn::Stage(input))),
        run: EventHandler::new(move |command: Command| dispatch.send(ViewerIn::Run(command))),
        lines: (shelf.lines)(),
        sorted: (shelf.sorted)(),
        ask_lines: EventHandler::new(move |(first, rows): (anyview_core::LineIndex, u32)| {
            if let Some((held, doc)) = shelf.shown_now()
                && let Some(job) = doc.view().lines(held, first, rows)
            {
                lines_edge.submit(job);
            }
        }),
        hits: (shelf.hits)(),
        work: EventHandler::new(move |job: Job| worker.submit(job)),
        request: EventHandler::new(move |request: HostRequest| requester.request(request)),
        pdf: shelf.pdf,
        media: shelf.media,
        frame: FrameLook { attributes },
        platform: shelf.platform,
        // A pane changes nothing in the file: what edits is the host's to offer.
        access: if hosted {
            FileAccess::ReadOnly
        } else {
            shelf
                .probe
                .read()
                .found()
                .map_or(FileAccess::Writable, |probed| probed.access)
        },
        text_edit: shelf.edit.session,
        text_editor: shelf.edit.handle,
    }
}
