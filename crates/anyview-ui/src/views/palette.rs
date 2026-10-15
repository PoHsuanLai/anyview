//! The ⌘K palette: every command of the open file, each with its keys. As a find (⌘F) it lists the
//! places in the file the text names first, under "In This File", then the find's options (Match
//! Case, Whole Words) as checked rows that leave the palette up, and then the commands, as mailo's
//! search lists mail before commands. The palette machine owns the query, the selection and what
//! Enter runs; this draws its rows and hands the keys it takes (arrows, Enter, Esc, ⌘K, ⌘F) to the
//! root as they arrive, so the machine is the one place they mean anything.

use super::press::press_of;
use crate::{Act, Chords, Command, HitLine, PaletteIn, PaletteIndex, PaletteScope, TypedText};
use anyview_core::{FindOption, FindOptions};
use anyview_text::MAX_HITS;
use chordkit::Context;
use dioxus::prelude::*;
use ds::components::lists::row::chord::RowChord;
use ds::components::menus::palette::palette_claim::{Claim, FieldKey};
use ds::components::menus::palette::palette_group::{PaletteGroup, PaletteRow};
use ds::focus::request::use_focus_request;
use ds::host::caret::InitialCaret;
use ds::prelude::{
    Accessory, AfterPick, Check, CommandPalette, RunTone, TextLine, TextRun, use_keys,
};

/// The row of one hit: the words around it with the match marked, and where it is. A hit whose
/// words were not kept is listed by where it is. `value` is what picking the row yields, and
/// `index` the hit's place in the find (for the row of a hit that has no words).
pub(super) fn hit_row<T>(value: T, index: usize, line: Option<&HitLine>) -> PaletteRow<T> {
    let Some(line) = line else {
        return PaletteRow::new(value, format!("Match {}", index + 1));
    };
    if line.context.is_empty() {
        let title = if line.place.is_empty() {
            format!("Match {}", index + 1)
        } else {
            line.place.clone()
        };
        return PaletteRow::new(value, title);
    }
    let (context, matched) = (&line.context, &line.matched);
    let marked = matched.start < matched.end
        && matched.end <= context.len()
        && context.is_char_boundary(matched.start)
        && context.is_char_boundary(matched.end);
    let runs: Vec<TextRun> = if marked {
        [
            (&context[..matched.start], RunTone::Plain),
            (&context[matched.clone()], RunTone::Mark),
            (&context[matched.end..], RunTone::Plain),
        ]
        .into_iter()
        .filter(|(text, _)| !text.is_empty())
        .map(|(text, tone)| TextRun::new(text, tone))
        .collect()
    } else {
        vec![TextRun::new(context.clone(), RunTone::Plain)]
    };
    PaletteRow {
        // A hit with no place to name (a page's) is listed by its words alone.
        detail: (!line.place.is_empty()).then(|| TextLine::from(line.place.clone())),
        ..PaletteRow::new(value, TextLine::Runs(runs))
    }
}

/// The row of a find option: its name, checked while the option is on. Picking it leaves the palette
/// up, so the find can be refined and its hits watched change. The name is drawn as given: the text
/// typed is the find's, and marks nothing here.
fn option_row(index: usize, option: FindOption, options: FindOptions) -> PaletteRow<usize> {
    let check = if options.has(option) {
        Check::On
    } else {
        Check::Off
    };
    PaletteRow {
        accessory: Accessory::Check(check),
        after: AfterPick::KeepOpen,
        ..PaletteRow::new(
            index,
            TextLine::Runs(vec![TextRun::new(option.label(), RunTone::Plain)]),
        )
    }
}

/// The palette over the window. `rows` are the machine's ranked rows; `selection` its highlight.
/// As a find (`scope`), the rows that are hits are drawn from `hits` (the first `rows` of them),
/// `found` is how many there are in all, and the rows that are find options are drawn checked as
/// `options` have them.
#[component]
pub(super) fn Palette(
    query: TypedText,
    rows: Vec<Command>,
    selection: PaletteIndex,
    scope: PaletteScope,
    hits: Vec<HitLine>,
    found: u32,
    options: FindOptions,
    ontyped: EventHandler<TypedText>,
    onpick: EventHandler<PaletteIndex>,
    onkey: EventHandler<KeyboardEvent>,
    onclose: EventHandler<()>,
) -> Element {
    let keymap = use_keys();
    let finding = matches!(scope, PaletteScope::Find(_));
    let mut in_file: Vec<PaletteRow<usize>> = Vec::new();
    let mut refining: Vec<PaletteRow<usize>> = Vec::new();
    let mut commands: Vec<PaletteRow<usize>> = Vec::new();
    // A search keeps no more hits than it can step through: a count at that is a count of at least.
    let at_least = if usize::try_from(found).is_ok_and(|found| found >= MAX_HITS) {
        "+"
    } else {
        ""
    };
    let (mut listed, mut all_row) = (0_u32, false);
    for (index, command) in rows.iter().enumerate() {
        match command {
            Command::FindHit(hit) => {
                listed += 1;
                in_file.push(hit_row(index, hit.0 as usize, hits.get(hit.0 as usize)))
            }
            Command::ShowAllHits => {
                all_row = true;
                in_file.push(PaletteRow::new(
                    index,
                    format!("Show All {found}{at_least}"),
                ));
            }
            Command::FindOption(option) => refining.push(option_row(index, *option, options)),
            Command::File(_)
            | Command::Stage(_)
            | Command::OpenFile
            | Command::UseTool(_)
            | Command::Picture(_)
            | Command::Markup(_)
            | Command::ShowView(_)
            | Command::Install(_) => commands.push(PaletteRow {
                chord: command.shortcut().map(RowChord::always).unwrap_or_default(),
                ..PaletteRow::new(index, command.label())
            }),
        }
    }
    // A list that stops short of the hits says how far it goes, as Preview's says "More than 1,000".
    let in_file_title = if all_row || found <= listed {
        "In This File".to_owned()
    } else {
        format!("In This File \u{b7} the first {listed} of {found}{at_least}")
    };
    let groups = if finding {
        vec![
            PaletteGroup::list(in_file_title, in_file),
            PaletteGroup::list("Options", refining),
            PaletteGroup::list("Commands", commands),
        ]
    } else {
        vec![PaletteGroup::list("Commands", commands)]
    };
    // A find opened on the last find's text has it selected, so the first key typed replaces it;
    // so does Find pressed again over the text of a find (below), as it does in Safari and GNOME.
    let caret = if finding && !query.is_empty() {
        InitialCaret::SelectAll
    } else {
        InitialCaret::End
    };
    let field = use_focus_request().with_caret(caret);
    let selects_again = finding && !query.is_empty();
    rsx! {
        CommandPalette::<usize> {
            label: if finding { "Find" } else { "Commands" },
            placeholder: if finding { "Find in this file" } else { "Type a command" },
            query: query.as_str().to_owned(),
            // The query is shown as typed: no chip echoes its words.
            tokens: Vec::new(),
            groups,
            empty: if finding { "No matches" } else { "No matching command" },
            selected: Some(selection.0),
            focus: Some(field),
            oninput: move |text: String| ontyped.call(TypedText::new(text)),
            onpick: move |index: usize| onpick.call(PaletteIndex(index)),
            onclose: move |()| onclose.call(()),
            claim: Callback::new(move |key: FieldKey| {
                // The keys the palette machine reads are the machine's: they never reach the
                // field or the palette's own selection.
                // Find is also the window's: it makes what is typed a find.
                let press = press_of(keymap, &key.event, Context::TextEntry, Chords::Viewer);
                let find = press.as_ref().and_then(|press| press.act()) == Some(Act::Find);
                if press
                    .as_ref()
                    .is_some_and(|press| PaletteIn::from_press(press).is_some() || find)
                {
                    // Find over the text of a find selects it all: the next key replaces it.
                    if find && selects_again {
                        field.request();
                    }
                    onkey.call(key.event);
                    Claim::Take
                } else {
                    Claim::Pass
                }
            }),
        }
    }
}
