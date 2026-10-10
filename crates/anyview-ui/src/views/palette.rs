//! The ⌘K palette: every command of the open file, each with its keys. As a find (⌘F) it lists the
//! places in the file the text names first, under "In This File", and then the commands, as mailo's
//! search lists mail before commands. The palette machine owns the query, the selection and what
//! Enter runs; this draws its rows and hands the keys it takes (arrows, Enter, Esc, ⌘K, ⌘F) to the
//! root as they arrive, so the machine is the one place they mean anything.

use crate::{Command, HitLine, PaletteIndex, PaletteScope, PictureCommand, Tool, TypedText};
use anyview_core::shortcut;
use dioxus::prelude::*;
use ds::components::lists::row::chord::RowChord;
use ds::components::menus::palette::palette_claim::{Claim, FieldKey};
use ds::components::menus::palette::palette_group::{PaletteGroup, PaletteRow};
use ds::host::caret::InitialCaret;
use ds::prelude::{CommandPalette, RunTone, Shortcut, ShortcutKey, TextLine, TextRun};

/// The keys shown beside a command.
fn keys_of(command: &Command) -> Option<Shortcut> {
    match command {
        Command::File(action) => shortcut(*action),
        Command::Stage(command) => Some(command.shortcut()),
        Command::OpenFile => Some(Shortcut(vec![ShortcutKey::Super, ShortcutKey::Char('o')])),
        Command::UseTool(tool) => match tool {
            Tool::Crop => Some(Shortcut(vec![ShortcutKey::Char('c')])),
            Tool::Select | Tool::Pan => Some(Shortcut(vec![ShortcutKey::Char('h')])),
        },
        Command::Picture(command) => match command {
            PictureCommand::Save => {
                Some(Shortcut(vec![ShortcutKey::Super, ShortcutKey::Char('s')]))
            }
            PictureCommand::AdjustSize => None,
        },
        Command::ShowView(_) => Some(Shortcut(vec![ShortcutKey::Char('v')])),
        Command::FindHit(_) | Command::ShowAllHits | Command::Install(_) => None,
    }
}

/// The row of one hit: the words around it with the match marked, and where it is. A hit whose
/// words were not kept is listed by where it is.
fn hit_row(index: usize, line: Option<&HitLine>) -> PaletteRow<usize> {
    let Some(line) = line else {
        return PaletteRow::new(index, format!("Match {}", index + 1));
    };
    if line.context.is_empty() {
        return PaletteRow::new(index, line.place.clone());
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
        detail: Some(TextLine::from(line.place.clone())),
        ..PaletteRow::new(index, TextLine::Runs(runs))
    }
}

/// Whether `keys` is ⌘F, which the palette's field hands to the window to make the text a find.
fn is_find(keys: &[ShortcutKey]) -> bool {
    keys == [ShortcutKey::Super, ShortcutKey::Char('f')]
}

/// The palette over the window. `rows` are the machine's ranked rows; `selection` its highlight.
/// As a find (`scope`), the rows that are hits are drawn from `hits` (the first `rows` of them) and
/// `found` is how many there are in all.
#[component]
pub(super) fn Palette(
    query: TypedText,
    rows: Vec<Command>,
    selection: PaletteIndex,
    scope: PaletteScope,
    hits: Vec<HitLine>,
    found: u32,
    ontyped: EventHandler<TypedText>,
    onpick: EventHandler<PaletteIndex>,
    onkey: EventHandler<KeyboardEvent>,
    onclose: EventHandler<()>,
) -> Element {
    let finding = matches!(scope, PaletteScope::Find(_));
    let mut in_file: Vec<PaletteRow<usize>> = Vec::new();
    let mut commands: Vec<PaletteRow<usize>> = Vec::new();
    for (index, command) in rows.iter().enumerate() {
        match command {
            Command::FindHit(hit) => in_file.push(hit_row(index, hits.get(hit.0 as usize))),
            Command::ShowAllHits => {
                in_file.push(PaletteRow::new(index, format!("Show All {found}")));
            }
            Command::File(_)
            | Command::Stage(_)
            | Command::OpenFile
            | Command::UseTool(_)
            | Command::Picture(_)
            | Command::ShowView(_)
            | Command::Install(_) => commands.push(PaletteRow {
                chord: keys_of(command).map(RowChord::always).unwrap_or_default(),
                ..PaletteRow::new(index, command.label())
            }),
        }
    }
    let tokens: Vec<String> = query
        .as_str()
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    let groups = if finding {
        vec![
            PaletteGroup::list("In This File", in_file),
            PaletteGroup::list("Commands", commands),
        ]
    } else {
        vec![PaletteGroup::list("Commands", commands)]
    };
    // A find opened on the last find's text has it selected, so the first key typed replaces it.
    let caret = if finding && !query.is_empty() {
        InitialCaret::SelectAll
    } else {
        InitialCaret::End
    };
    rsx! {
        CommandPalette::<usize> {
            label: if finding { "Find" } else { "Commands" },
            placeholder: if finding { "Find in this file" } else { "Type a command" },
            query: query.as_str().to_owned(),
            tokens,
            groups,
            empty: if finding { "No matches" } else { "No matching command" },
            selected: Some(selection.0),
            initial_caret: caret,
            oninput: move |text: String| ontyped.call(TypedText::new(text)),
            onpick: move |index: usize| onpick.call(PaletteIndex(index)),
            onclose: move |()| onclose.call(()),
            claim: Callback::new(move |key: FieldKey| {
                // The keys the palette machine reads are the machine's: they never reach the
                // field or the palette's own selection.
                let keys = super::keys::keys_of(&key.event);
                if crate::PaletteIn::from_key(&keys).is_some() || is_find(&keys) {
                    onkey.call(key.event);
                    Claim::Take
                } else {
                    Claim::Pass
                }
            }),
        }
    }
}
