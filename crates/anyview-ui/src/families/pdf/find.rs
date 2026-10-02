//! The find bar over the pages: the shared bar, wired to the PDF machine's inputs. It shows while
//! the stage machine is finding and holds nothing of its own: what is typed is the machine's
//! query, and the buttons are machine inputs.

use crate::families::find_bar::{FindBar, FindStep};
use crate::families::view::StageCx;
use crate::{PdfIn, PdfStage, Stage, StageIn, TypedText};
use dioxus::prelude::*;

#[component]
pub(super) fn PdfFinding(cx: StageCx) -> Element {
    let Stage::Pdf(PdfStage::Finding { query, hits, .. }) = &cx.stage else {
        return rsx! {};
    };
    let (query, hits) = (query.clone(), *hits);
    let send = cx.send;
    rsx! {
        FindBar {
            query,
            hits,
            typing: cx.typing,
            onfind: move |text: TypedText| send.call(StageIn::Pdf(PdfIn::Find(text))),
            onstep: move |step: FindStep| {
                send.call(StageIn::Pdf(match step {
                    FindStep::Next => PdfIn::NextHit,
                    FindStep::Previous => PdfIn::PreviousHit,
                    FindStep::Close => PdfIn::CloseFind,
                }));
            },
        }
    }
}
