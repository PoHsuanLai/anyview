//! Images: the raster stage's view. A picture is decoded and uploaded on a worker (`doc`),
//! placed by pure arithmetic over the stage machine's state (`geometry`), and drawn by a
//! `TextureLayer` that shows the visible texels (`view`).

mod doc;
mod geometry;
mod view;

pub use doc::{RasterBackend, RasterDoc, RasterDone, RasterJob, RasterTarget};

use crate::families::view::{Area, Held, StageCx, StageView};
use crate::io::{NaturalSize, OpenError, OpenLink};
use crate::{
    Animation, Command, FrameCount, LoadFlow, PanelTab, PanelTabs, RasterIn, Stage, StageFamily,
    StageIn, StageParams, Ticket,
};
use anyview_core::{Facts, Permille, Resume, Sniffed, Source};
use dioxus::prelude::*;
use ds::components::chrome::capsule::model::CapsuleSlot;
use ds::components::chrome::capsule::priority::RankedSlot;
use ds::prelude::Icon;
use std::sync::Arc;

/// How soon each control goes when the capsule is too wide for the stage (quire's capsule).
const RANK_ROTATE: u8 = 2;
const RANK_ZOOM: u8 = 1;

/// Raster and vector images.
#[derive(Debug, Clone, Copy)]
pub struct RasterStageView;

impl StageView for RasterStageView {
    const FAMILY: StageFamily = StageFamily::Raster;
    const FLOW: LoadFlow = LoadFlow::PeekThenOpen;
    type Doc = RasterDoc;

    fn first_frame(
        _ticket: Ticket,
        src: &Source,
        sniffed: &Sniffed,
        link: &OpenLink,
    ) -> Result<Option<RasterDoc>, OpenError> {
        doc::first_frame(src, sniffed, link)
    }

    fn open(
        ticket: Ticket,
        src: &Source,
        sniffed: &Sniffed,
        link: &OpenLink,
    ) -> Result<RasterDoc, OpenError> {
        let target = RasterTarget {
            source: src.clone(),
            sniffed: sniffed.clone(),
            texture: link.texture.clone(),
            plugins: Arc::clone(&link.image_plugins),
        };
        let done = <RasterBackend as crate::io::Backend>::run(
            &target,
            &mut (),
            RasterJob::Decode { ticket },
            &crate::io::Stop::new(),
        );
        done.result
    }

    fn facts(doc: &RasterDoc) -> Facts {
        doc.facts.clone()
    }

    fn tabs(_doc: &RasterDoc) -> PanelTabs {
        PanelTabs::of(&[PanelTab::Info])
    }

    fn params(doc: &RasterDoc, stage: &Stage, area: Option<Area>) -> StageParams {
        let timing = crate::RasterParams {
            delays: doc.delays(),
            runs: doc.runs,
            ..crate::RasterParams::default()
        };
        let (Stage::Raster(raster), Some(area)) = (stage, area) else {
            return StageParams {
                raster: timing,
                ..StageParams::default()
            };
        };
        let turn = geometry::turn_of(raster);
        let fit = geometry::fit(doc.size, turn, area);
        let shown = geometry::scale_of(raster, fit);
        StageParams {
            raster: crate::RasterParams {
                viewport: crate::Viewport { shown, fit },
                centre: geometry::centre_of(raster, doc.size),
                ..timing
            },
            ..StageParams::default()
        }
    }

    fn natural(doc: &RasterDoc) -> Option<NaturalSize> {
        // With no picture to show (its plugin is missing) there is nothing to fit the window to.
        doc.needs.is_none().then_some(NaturalSize::Pixels(doc.size))
    }

    fn arrived(doc: &RasterDoc, stage: &Stage, _left_at: &Resume) -> Vec<StageIn> {
        let animation = match stage {
            Stage::Raster(raster) => geometry::animation_of(raster),
            Stage::NoStage
            | Stage::Pdf(_)
            | Stage::Media(_)
            | Stage::Text(_)
            | Stage::Table(_)
            | Stage::Tree(_) => return Vec::new(),
        };
        match (
            doc.plays(),
            animation,
            std::num::NonZeroU32::new(doc.frames),
        ) {
            (true, Animation::Still, Some(count)) => {
                vec![StageIn::Raster(RasterIn::Animated(FrameCount(count)))]
            }
            (
                true,
                Animation::Playing { .. } | Animation::Paused { .. } | Animation::Ended { .. },
                _,
            )
            | (true, Animation::Still, None)
            | (false, _, _) => Vec::new(),
        }
    }

    fn stage(doc: &Arc<RasterDoc>, cx: &StageCx) -> Element {
        rsx! { view::RasterContent { doc: Held(Arc::clone(doc)), cx: cx.clone() } }
    }

    fn edit_offer(doc: &RasterDoc) -> crate::EditOffer {
        doc.offer
    }

    fn lacks(doc: &RasterDoc) -> Option<anyview_core::Helper> {
        doc.lacking
    }

    fn slots(doc: &RasterDoc, cx: &StageCx) -> Vec<RankedSlot<Command>> {
        if doc.needs.is_some() {
            return Vec::new();
        }
        use crate::StageCommand::{ZoomIn, ZoomOut};
        use anyview_core::FileAction::{RotateLeft, RotateRight};
        let percent = match (&cx.stage, cx.area) {
            (Stage::Raster(raster), Some(area)) => {
                let turn = geometry::turn_of(raster);
                let fit = geometry::fit(doc.size, turn, area);
                geometry::scale_of(raster, fit)
            }
            _ => Permille::WHOLE,
        };
        // Soonest to go first when the stage is narrow (quire's capsule): the rotate buttons, then
        // the zoom. An animation's play button stays.
        let mut slots = vec![
            CapsuleSlot::button(Command::Stage(ZoomOut), "Zoom out", Icon::Minus)
                .droppable(RANK_ZOOM),
            CapsuleSlot::Readout(format!("{}%", percent.0 / 10)).droppable(RANK_ZOOM),
            CapsuleSlot::button(Command::Stage(ZoomIn), "Zoom in", Icon::Plus).droppable(RANK_ZOOM),
        ];
        if doc.offer != crate::EditOffer::Withheld {
            slots.push(CapsuleSlot::Divider.essential());
            slots.push(
                CapsuleSlot::button(Command::File(RotateLeft), "Rotate left", Icon::RotateLeft)
                    .droppable(RANK_ROTATE),
            );
            slots.push(
                CapsuleSlot::button(
                    Command::File(RotateRight),
                    "Rotate right",
                    Icon::RotateRight,
                )
                .droppable(RANK_ROTATE),
            );
        }
        if let (true, Stage::Raster(raster)) = (doc.plays(), &cx.stage) {
            let (label, icon) = match geometry::animation_of(raster) {
                Animation::Playing { .. } => ("Pause", Icon::Pause),
                Animation::Still | Animation::Paused { .. } | Animation::Ended { .. } => {
                    ("Play", Icon::Play)
                }
            };
            slots.push(CapsuleSlot::Divider.essential());
            slots.push(
                CapsuleSlot::button(
                    Command::Stage(crate::StageCommand::TogglePlayback),
                    label,
                    icon,
                )
                .essential(),
            );
        }
        slots
    }

    fn panel(_doc: &Arc<RasterDoc>, _tab: PanelTab, _cx: &StageCx) -> Option<Element> {
        None
    }
}
