//! Images: the raster stage's view. A picture is decoded and uploaded on a worker (`doc`),
//! placed by pure arithmetic over the stage machine's state (`geometry`), and drawn by a
//! `TextureLayer` that shows the visible texels (`view`).

mod doc;
mod geometry;
mod view;

pub use doc::{RasterBackend, RasterDoc, RasterDone, RasterJob, RasterTarget};

use crate::families::view::{Area, Held, StageCx, StageView};
use crate::io::{OpenError, OpenLink};
use crate::{Command, PanelTab, PanelTabs, Stage, StageFamily, StageParams, Ticket};
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
    type Doc = RasterDoc;

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
        vec![
            CapsuleSlot::button(Command::Stage(ZoomOut), "Zoom out", Icon::Minus),
            CapsuleSlot::Readout(format!("{}%", percent.0 / 10)),
            CapsuleSlot::button(Command::Stage(ZoomIn), "Zoom in", Icon::Plus),
            CapsuleSlot::Divider,
            CapsuleSlot::button(Command::File(RotateLeft), "Rotate left", Icon::Undo),
            CapsuleSlot::button(Command::File(RotateRight), "Rotate right", Icon::Refresh),
        ]
    }

    fn panel(_doc: &Arc<RasterDoc>, _tab: PanelTab, _cx: &StageCx) -> Option<Element> {
        None
    }
}
