use super::*;
use chordkit::{Action, DefaultChord};
use ds_core::standard_action::StandardAction;
use ds_core::testing::word_matches_serde;
use ds_core::vocab::Shortcut;
use ds_core::word::Word;

#[test]
fn actions_are_stored_as_their_slugs() {
    word_matches_serde::<FileAction>();
    let json = serde_json::to_string(&FileAction::RevealInFolder).unwrap();
    assert_eq!(json, "\"reveal_in_folder\"");
    assert_eq!(
        serde_json::from_str::<FileAction>(&json).unwrap(),
        FileAction::RevealInFolder
    );
}

#[test]
fn reach_says_where_each_action_appears() {
    use FileAction::*;
    use Reach::{Both, Launcher, Viewer};
    const CASES: &[(FileAction, Reach)] = &[
        (Open, Launcher),
        (RevealInFolder, Both),
        (CopyFile, Both),
        (CopyPath, Both),
        (Share, Both),
        (Rename, Both),
        (Duplicate, Both),
        (MoveToTrash, Both),
        (Print, Viewer),
        (Export, Viewer),
        (SaveCopy, Viewer),
        (RevertTo, Viewer),
        (RotateLeft, Viewer),
        (RotateRight, Viewer),
        (FlipHorizontal, Viewer),
        (FlipVertical, Viewer),
        (PlayInBackground, Both),
        (PlayInMiniWindow, Both),
        (ConvertTo, Both),
    ];
    assert_eq!(
        CASES.len(),
        FileAction::ALL.len(),
        "the table names every action"
    );
    for (action, want) in CASES {
        assert_eq!(reach(*action), *want, "{action:?}");
    }
}

/// What an action's shortcut must be.
#[derive(Debug, Clone, Copy)]
enum Want {
    Nothing,
    /// The standard action's own keys.
    Standard(StandardAction),
    /// The viewer's own action, with its default chord as the keymap reads it.
    Own(&'static str, &'static str),
}

#[test]
fn shortcuts_are_the_standard_ones_or_the_viewers_own() {
    use FileAction::*;
    use Want::{Nothing, Own, Standard};
    const CASES: &[(FileAction, Want)] = &[
        (Open, Standard(StandardAction::Open)),
        (RevealInFolder, Standard(StandardAction::Reveal)),
        (CopyFile, Standard(StandardAction::Copy)),
        (CopyPath, Own("anyview.copy-path", "Primary+Alt+C")),
        (Share, Nothing),
        (Rename, Nothing),
        (Duplicate, Own("anyview.duplicate", "Primary+D")),
        (
            MoveToTrash,
            Own("anyview.move-to-trash", "Primary+Backspace"),
        ),
        (Print, Standard(StandardAction::Print)),
        (Export, Own("anyview.export", "Primary+Shift+E")),
        (SaveCopy, Standard(StandardAction::SaveAs)),
        (RevertTo, Nothing),
        (RotateLeft, Own("anyview.rotate-left", "Primary+[")),
        (RotateRight, Own("anyview.rotate-right", "Primary+]")),
        (FlipHorizontal, Nothing),
        (FlipVertical, Nothing),
        (PlayInBackground, Nothing),
        (PlayInMiniWindow, Nothing),
        (ConvertTo, Nothing),
    ];
    assert_eq!(
        CASES.len(),
        FileAction::ALL.len(),
        "the table names every action"
    );
    for (action, want) in CASES {
        let got = shortcut(*action);
        match want {
            Nothing => assert_eq!(got, None, "{action:?}"),
            Standard(standard) => {
                assert_eq!(got, Some(Shortcut::standard(*standard)), "{action:?}");
                assert_eq!(
                    file_action_of(&Action::Standard(*standard)),
                    Some(*action),
                    "{action:?}"
                );
            }
            Own(id, chord) => {
                let own = own_keys().into_iter().find(|(file, _, _)| file == action);
                let (_, app, default) = own.unwrap_or_else(|| panic!("{action:?} has no row"));
                assert_eq!(app.id(), *id, "{action:?}");
                assert_eq!(Ok(default), chord.parse::<DefaultChord>(), "{action:?}");
                assert_eq!(
                    got,
                    Shortcut::from_default_chord(default),
                    "{action:?}: the shown keys are the default chord"
                );
                assert_eq!(
                    file_action_of(&Action::App(app)),
                    Some(*action),
                    "{action:?}"
                );
            }
        }
    }
}
