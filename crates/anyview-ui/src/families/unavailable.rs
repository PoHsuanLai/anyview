//! The card of a file the viewer has a stage for and cannot use right now: a recording nothing
//! plays, a picture whose decoder is not installed. One layout for all of them, so a missing tool
//! reads the same wherever it is missing: the file's name and summary, one line saying what is
//! missing and what it is for (or that the file cannot be used), Install… when the host offers
//! one, Show in Folder, and the file's facts below, scrolling when they do not fit. A video whose
//! frame could be decoded shows it above the name; one whose frame could not keeps the icon.

use crate::families::InfoCard;
use crate::families::view::StageCx;
use crate::io::{DesktopService, HostRequest};
use crate::{Command, Need};
use anyview_core::{FactLabel, Facts, FileAction, FormatKind, Helper};
use dioxus::prelude::*;
use ds::components::content::image_source::ImageSource;
use ds::components::controls::button::Button;
use ds_core::word::Word;

/// The line under the summary: the `Needs` row of `needs` as a person reads it, or `unusable`
/// (what the file cannot do) when nothing is known to fix it.
pub(crate) fn note_of(needs: Option<&Need>, unusable: &str) -> String {
    match needs {
        // A plugin that is installed and turned off is not missing anything: the row is a sentence.
        Some(need) if need.turn_on.is_some() => format!("{}.", need.fact.value.as_str()),
        Some(need) => format!("{}: {}", need.fact.label.label(), need.fact.value.as_str()),
        None => unusable.to_owned(),
    }
}

/// `facts` without the `Needs` row, which the card's note already says.
pub(crate) fn listed(facts: &Facts) -> Facts {
    facts
        .rows()
        .iter()
        .filter(|row| row.label != FactLabel::Needs)
        .fold(Facts::empty(), |listed, row| listed.with_fact(row.clone()))
}

/// The card of the file called `name`, of `kind`, whose rows are `facts`. `note` is the line under
/// the summary; `install` is the tool whose installation the host offers, if any, and `turn_on` the
/// Extension whose Turn On… opens Settings, when the platform has one to open.
#[component]
pub(crate) fn UnavailableCard(
    name: String,
    kind: FormatKind,
    facts: Facts,
    note: String,
    install: Option<Helper>,
    #[props(default)] turn_on: Option<String>,
    #[props(default)] poster: Option<ImageSource>,
    cx: StageCx,
) -> Element {
    let run = cx.run;
    let request = cx.request;
    let platform = cx.platform;
    let turn_on = turn_on.filter(|_| platform.has(DesktopService::Settings));
    rsx! {
        div { class: "viewer-peek",
            div { class: "viewer-peek-body",
                InfoCard {
                    name,
                    kind: Some(kind),
                    poster,
                    facts,
                    note: Some(note),
                    actions: rsx! {
                        div { class: "viewer-failed-actions",
                            if let Some(helper) = install {
                                Button {
                                    label: "Install…",
                                    onclick: move |_| run.call(Command::Install(helper)),
                                }
                            }
                            if let Some(extension) = turn_on {
                                Button {
                                    label: "Turn On…",
                                    onclick: move |_| request.call(HostRequest::TurnOn(extension.clone())),
                                }
                            }
                            if platform.offers(FileAction::RevealInFolder) {
                                Button {
                                    label: "Show in Folder",
                                    onclick: move |_| run.call(Command::File(FileAction::RevealInFolder)),
                                }
                            }
                        }
                    },
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::{Fact, FactValue};

    #[test]
    fn a_missing_tool_is_named_once_and_a_file_nothing_fixes_says_so() {
        let need = Need::passive(Fact::new(
            FactLabel::Needs,
            FactValue::text("anyview-mpv (to play it)"),
        ));
        assert_eq!(
            note_of(Some(&need), "This recording can\u{2019}t be played."),
            "Needs: anyview-mpv (to play it)"
        );
        assert_eq!(
            note_of(None, "This recording can\u{2019}t be played."),
            "This recording can\u{2019}t be played."
        );
        let off = Need::turned_off(
            Fact::new(FactLabel::Needs, FactValue::text("mpv is turned off")),
            "org.quire.anyview.Mpv",
        );
        assert_eq!(note_of(Some(&off), "unused"), "mpv is turned off.");
        let facts = Facts::empty()
            .with(FactLabel::Kind, FactValue::text("AVI video"))
            .with_fact(need.fact.clone());
        let listed = listed(&facts);
        let labels: Vec<_> = listed.rows().iter().map(|row| row.label).collect();
        assert_eq!(labels, [FactLabel::Kind]);
    }
}
