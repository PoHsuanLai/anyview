//! Images: the raster stage's view. A picture is decoded and uploaded on a worker (`doc`),
//! placed by pure arithmetic over the stage machine's state (`geometry`), and drawn by a
//! `TextureLayer` that shows the visible texels (`view`).

mod doc;
mod geometry;
mod view;

pub use doc::{RasterBackend, RasterDoc, RasterDone, RasterJob, RasterTarget};

use crate::families::view::{Area, Held, StageCx, StageView};
use crate::io::{OpenError, OpenLink};
use crate::{
    Animation, Command, FrameCount, LoadFlow, PanelTab, PanelTabs, RasterIn, Stage, StageFamily,
    StageIn, StageParams, Ticket,
};
use anyview_core::{Facts, Permille, Sniffed, Source};
use dioxus::prelude::*;
use ds::components::chrome::capsule::model::CapsuleSlot;
use ds::prelude::Icon;
use std::sync::Arc;

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
        let (Stage::Raster(raster), Some(area)) = (stage, area) else {
            return StageParams::default();
        };
        let turn = geometry::turn_of(raster);
        let fit = geometry::fit(doc.size, turn, area);
        let shown = geometry::scale_of(raster, fit);
        StageParams {
            raster: crate::RasterParams {
                viewport: crate::Viewport { shown, fit },
                centre: geometry::centre_of(raster, doc.size),
                ..crate::RasterParams::default()
            },
            ..StageParams::default()
        }
    }

    fn arrived(doc: &RasterDoc, stage: &Stage) -> Vec<StageIn> {
        let animation = match stage {
            Stage::Raster(raster) => geometry::animation_of(raster),
            Stage::NoStage | Stage::Pdf(_) | Stage::Media(_) | Stage::Text(_) => return Vec::new(),
        };
        match (
            doc.plays(),
            animation,
            std::num::NonZeroU32::new(doc.frames),
        ) {
            (true, Animation::Still, Some(count)) => {
                vec![StageIn::Raster(RasterIn::Animated(FrameCount(count)))]
            }
            (true, Animation::Playing { .. } | Animation::Paused { .. }, _)
            | (true, Animation::Still, None)
            | (false, _, _) => Vec::new(),
        }
    }

    fn stage(doc: &Arc<RasterDoc>, cx: &StageCx) -> Element {
        rsx! { view::RasterContent { doc: Held(Arc::clone(doc)), cx: cx.clone() } }
    }

    fn slots(doc: &RasterDoc, cx: &StageCx) -> Vec<CapsuleSlot<Command>> {
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
        let mut slots = vec![
            CapsuleSlot::button(Command::Stage(ZoomOut), "Zoom out", Icon::Minus),
            CapsuleSlot::Readout(format!("{}%", percent.0 / 10)),
            CapsuleSlot::button(Command::Stage(ZoomIn), "Zoom in", Icon::Plus),
            CapsuleSlot::Divider,
            CapsuleSlot::button(Command::File(RotateLeft), "Rotate left", Icon::Undo),
            CapsuleSlot::button(Command::File(RotateRight), "Rotate right", Icon::Refresh),
        ];
        if let (true, Stage::Raster(raster)) = (doc.plays(), &cx.stage) {
            let (label, icon) = match geometry::animation_of(raster) {
                Animation::Playing { .. } => ("Pause", Icon::Pause),
                Animation::Still | Animation::Paused { .. } => ("Play", Icon::Play),
            };
            slots.push(CapsuleSlot::Divider);
            slots.push(CapsuleSlot::button(
                Command::Stage(crate::StageCommand::TogglePlayback),
                label,
                icon,
            ));
        }
        slots
    }

    fn panel(_doc: &Arc<RasterDoc>, _tab: PanelTab, _cx: &StageCx) -> Option<Element> {
        None
    }
}
