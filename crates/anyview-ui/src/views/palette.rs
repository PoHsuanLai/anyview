//! The ⌘K palette: every command of the open file, each with its keys. The palette machine owns
//! the query, the selection and what Enter runs; this draws its rows and hands the keys it takes
//! (arrows, Enter, Esc, ⌘K) to the root as they arrive, so the machine is the one place they mean
//! anything.

use crate::{Command, RowIndex, TypedText};
use anyview_core::shortcut;
use dioxus::prelude::*;
use ds::components::lists::row::chord::RowChord;
use ds::components::menus::palette::palette_claim::{Claim, FieldKey};
use ds::components::menus::palette::palette_group::{PaletteGroup, PaletteRow};
use ds::prelude::{CommandPalette, Shortcut};

/// The keys shown beside a command.
fn keys_of(command: &Command) -> Option<Shortcut> {
    match command {
        Command::File(action) => shortcut(*action),
        Command::Stage(command) => Some(command.shortcut()),
    }
}

/// The palette over the window. `rows` are the machine's ranked rows; `selection` its highlight.
#[component]
pub(super) fn Palette(
    query: TypedText,
    rows: Vec<Command>,
    selection: RowIndex,
    ontyped: EventHandler<TypedText>,
    onpick: EventHandler<RowIndex>,
    onkey: EventHandler<KeyboardEvent>,
    onclose: EventHandler<()>,
) -> Element {
    let entries: Vec<PaletteRow<usize>> = rows
        .iter()
        .enumerate()
        .map(|(index, command)| PaletteRow {
            chord: keys_of(command).map(RowChord::always).unwrap_or_default(),
            ..PaletteRow::new(index, command.label())
        })
        .collect();
    let tokens: Vec<String> = query
        .as_str()
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    rsx! {
        CommandPalette::<usize> {
            label: "Commands",
            placeholder: "Type a command",
            query: query.as_str().to_owned(),
            tokens,
            groups: vec![PaletteGroup::list("Commands", entries)],
            empty: "No matching command",
            selected: Some(selection.0),
            oninput: move |text: String| ontyped.call(TypedText::new(text)),
            onpick: move |index: usize| onpick.call(RowIndex(index)),
            onclose: move |()| onclose.call(()),
            claim: Callback::new(move |key: FieldKey| {
                // The keys the palette machine reads are the machine's: they never reach the
                // field or the palette's own selection.
                if crate::PaletteIn::from_key(&super::keys::keys_of(&key.event)).is_some() {
                    onkey.call(key.event);
                    Claim::Take
                } else {
                    Claim::Pass
                }
            }),
        }
    }
}
