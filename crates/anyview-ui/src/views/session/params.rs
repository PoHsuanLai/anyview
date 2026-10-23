//! Everything the machines read besides their inputs, worked out from what the window knows now.

use super::commands::{commands, markup_commands, picture_commands};
use super::offers::{Exports, MarkupOffer, Offers, PictureOffer, Playback};
use super::probe::Probe;
use super::rows::rows_for;
use crate::families::{LineWindow, LoadedDoc, views_of};
use crate::io::NaturalSize;
use crate::{
    Command, ContextParams, EditOffer, ExportFacts, FileAccess, MediaOffer, Motion, Palette,
    PaletteParams, PaletteScope, PaneChrome, PanelParams, PlatformAbilities, Presentation,
    SheetParams, Spot, Stage, StageCommand, StageParams, TextParams, TextViews, TypedText,
    ViewerParams,
};
use anyview_core::{Adjust, FindOptions, FormatKind};
use anyview_machines::seam::{WrapChoices, entries};
use ds::prelude::MotionLevel;

/// What the window's settings and its player say right now.
#[derive(Debug, Clone, Copy)]
pub(in crate::views) struct Live {
    /// How much the window moves.
    pub level: MotionLevel,
    /// What the player can do.
    pub abilities: crate::MediaAbilities,
    /// What the platform can do.
    pub platform: PlatformAbilities,
    /// The pointer tool in use.
    pub tool: crate::Tool,
    /// What the person marked to keep of the recording.
    pub marks: crate::TrimMarks,
    /// What the open picture can be changed with.
    pub picture: PictureOffer,
    /// What has been done to the open picture.
    pub adjust: Adjust,
    /// What the open PDF lets the person mark, and what has been written on it.
    pub markup: MarkupOffer,
    /// How the person last chose to wrap each kind of text.
    pub wraps: WrapChoices,
    /// How the viewer is on screen: a pane is read-only and has no palette, so it lists no Find.
    pub presentation: Presentation,
    /// How much chrome a pane draws; a window ignores it.
    pub chrome: PaneChrome,
    /// How the find matches: its option rows are listed with a find.
    pub find: FindOptions,
}

/// Everything the machines read besides their inputs, from what the window knows now.
pub(in crate::views) fn params(
    stage: &Stage,
    doc: Option<&LoadedDoc>,
    probe: &Probe,
    area: Option<crate::Area>,
    palette: (&Palette, PaletteScope),
    lines: Option<&LineWindow>,
    live: Live,
) -> ViewerParams {
    let (palette, scope) = palette;
    let query = match palette {
        Palette::Open { query, .. } => query.clone(),
        Palette::Closed => TypedText::EMPTY,
    };
    let Live {
        level,
        abilities,
        platform,
        tool,
        marks,
        picture,
        adjust,
        markup,
        wraps,
        presentation,
        chrome,
        find,
    } = live;
    let pane = presentation == Presentation::Pane;
    let kind = probe.found().map(|probed| probed.sniffed.kind());
    let playback = doc.map_or(Playback::Playable, |doc| match (kind, doc.view().line()) {
        (Some(FormatKind::Video | FormatKind::Audio), None) => Playback::Unplayable,
        _ => Playback::Playable,
    });
    let offer = doc.map_or(EditOffer::Plain, |doc| doc.view().edit_offer());
    // A pane shows a file and changes nothing in it: what edits is the host's to offer.
    let access = if pane {
        FileAccess::ReadOnly
    } else {
        probe
            .found()
            .map_or(FileAccess::Writable, |probed| probed.access)
    };
    let mut measured = match doc {
        Some(doc) => doc.view().params(stage, area, lines),
        None => StageParams::default().with_text(TextParams {
            views: kind.map_or(TextViews::default(), views_of),
            ..TextParams::default()
        }),
    };
    if let (Stage::Raster(raster), Some(area), Some(doc), false) =
        (stage, area, doc, adjust.is_none())
        && let Some(NaturalSize::Pixels(base)) = doc.view().natural()
    {
        measured.raster =
            crate::families::adjusted_raster(measured.raster.clone(), raster, base, adjust, area);
    }
    // A pane, and a file that refuses a save, change nothing: no cell and no node is typed into.
    if pane || access == FileAccess::ReadOnly {
        measured.table.editable = crate::Editable::No;
        measured.tree.editable = crate::Editable::No;
    }
    if pane {
        measured.text.editable = crate::Editable::No;
    }
    measured.media.abilities = abilities;
    if let Some(kind) = kind {
        measured.text.wrap = wraps.of(kind);
    }
    measured.raster.motion = match level {
        MotionLevel::Reduced => Motion::Reduced,
        MotionLevel::Standard => Motion::Standard,
    };
    let exports = match (stage, doc) {
        (Stage::Media(_), Some(doc)) => {
            let media = doc.view().media_offer();
            if media.first().is_some() || media.needs().is_some() {
                Exports::Offered
            } else {
                Exports::Nothing
            }
        }
        _ => Exports::Offered,
    };
    let offers = Offers {
        playback,
        edit: offer,
        access,
        exports,
        platform,
        tool,
    };
    let mut listed = commands(kind, stage, &measured, offers);
    listed.extend(picture_commands(stage, picture, tool));
    listed.extend(markup_commands(stage, markup));
    if pane {
        // Finding is the palette's, and a pane draws none: its host starts one (`PaneLink::find`).
        listed.retain(|command| *command != Command::Stage(StageCommand::Find));
    }
    let mut panel = doc.map_or_else(PanelParams::default, |doc| doc.view().panel_params());
    if pane && chrome != PaneChrome::WithPanel {
        panel.tabs = crate::PanelTabs::default();
    }
    if matches!(stage, Stage::Media(_)) {
        panel.tabs = crate::families::media_tabs(panel.tabs, abilities);
    }
    if panel.tabs.first().is_some() {
        listed.push(Command::ToggleSidebar);
    }
    let files = listed
        .iter()
        .copied()
        .filter_map(|command| match command {
            Command::File(action) => Some(action),
            Command::Stage(_)
            | Command::OpenFile
            | Command::ToggleSidebar
            | Command::UseTool(_)
            | Command::Picture(_)
            | Command::Markup(_)
            | Command::ShowView(_)
            | Command::FindHit(_)
            | Command::ShowAllHits
            | Command::FindOption(_)
            | Command::Install(_) => None,
        })
        .collect();
    let context = ContextParams {
        entries: entries(&listed, panel.tabs),
        centre: centre_of(area),
    };
    let palette = PaletteParams {
        rows: rows_for(listed, &query, scope, (stage, find)),
    };
    let sheet = SheetParams {
        media: doc.map_or_else(MediaOffer::default, |doc| doc.view().media_offer()),
        edit: offer,
        export: export_facts(stage, doc, &measured, marks),
    };
    ViewerParams::default()
        .with_files(files)
        .with_context(context)
        .with_palette(palette)
        .with_panel(panel)
        .with_platform(platform)
        .with_sheet(sheet)
        .with_stage(measured)
}

/// What the export sheet is told of the open file: the pages of a document and where the reader
/// is in it, the size of a picture, and the part of a recording marked to keep.
pub(super) fn export_facts(
    stage: &Stage,
    doc: Option<&LoadedDoc>,
    measured: &StageParams,
    marks: crate::TrimMarks,
) -> ExportFacts {
    let mut facts = ExportFacts::default();
    match stage {
        Stage::Pdf(pdf) => {
            facts.pages = Some(measured.pdf.pages);
            facts.page = pdf.place().page;
        }
        Stage::Raster(_) => {
            facts.image = doc.and_then(|doc| match doc.view().natural() {
                Some(NaturalSize::Pixels(size)) => Some(size),
                Some(NaturalSize::Points(_) | NaturalSize::Compact(_)) | None => None,
            });
        }
        Stage::Media(_) => {
            facts.marks = marks.is_set().then(|| marks.range());
        }
        Stage::NoStage | Stage::Text(_) | Stage::Table(_) | Stage::Tree(_) => {}
    }
    facts
}

/// The middle of the content, where a menu opened by a key goes: the window's own middle until
/// the content has been measured.
pub(super) fn centre_of(area: Option<crate::Area>) -> Spot {
    area.map_or_else(Spot::default, |area| Spot {
        x: (area.origin.x.0 + area.size.width.0 / 2.0).round() as i32,
        y: (area.origin.y.0 + area.size.height.0 / 2.0).round() as i32,
    })
}
