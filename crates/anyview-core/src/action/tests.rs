use super::*;
use ds_core::standard_action::StandardAction;
use ds_core::testing::word_matches_serde;
use ds_core::vocab::{Shortcut, ShortcutKey};
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
        (OpenWith, Both),
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
    /// The viewer's own keys, in the Mac's modifier order.
    Own(&'static [ShortcutKey]),
}

#[test]
fn shortcuts_are_the_standard_ones_or_the_viewers_own() {
    use FileAction::*;
    use ShortcutKey::{Alt, Backspace, Char, Shift, Super};
    use Want::{Nothing, Own, Standard};
    const CASES: &[(FileAction, Want)] = &[
        (Open, Standard(StandardAction::Open)),
        (OpenWith, Own(&[Alt, Super, Char('o')])),
        (RevealInFolder, Standard(StandardAction::Reveal)),
        (CopyFile, Standard(StandardAction::Copy)),
        (CopyPath, Own(&[Alt, Super, Char('c')])),
        (Share, Nothing),
        (Rename, Nothing),
        (Duplicate, Own(&[Super, Char('d')])),
        (MoveToTrash, Own(&[Super, Backspace])),
        (Print, Standard(StandardAction::Print)),
        (Export, Own(&[Shift, Super, Char('e')])),
        (SaveCopy, Standard(StandardAction::SaveAs)),
        (RevertTo, Nothing),
        (RotateLeft, Own(&[Super, Char('[')])),
        (RotateRight, Own(&[Super, Char(']')])),
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
            }
            Own(keys) => assert_eq!(got.map(|s| s.keys()), Some(keys.to_vec()), "{action:?}"),
        }
    }
}

#[test]
fn no_shortcut_takes_a_reserved_combination_unless_it_is_that_standard_action() {
    // The standard actions the viewer's actions are allowed to be.
    const MEANS: &[(FileAction, StandardAction)] = &[
        (FileAction::Open, StandardAction::Open),
        (FileAction::RevealInFolder, StandardAction::Reveal),
        (FileAction::CopyFile, StandardAction::Copy),
        (FileAction::Print, StandardAction::Print),
        (FileAction::SaveCopy, StandardAction::SaveAs),
    ];
    let mut seen: Vec<Vec<ShortcutKey>> = Vec::new();
    for action in FileAction::ALL {
        let Some(shortcut) = shortcut(*action) else {
            continue;
        };
        let keys = shortcut.keys();
        let owner = StandardAction::owning(&keys);
        let means = MEANS.iter().find(|(a, _)| a == action).map(|(_, s)| *s);
        assert_eq!(owner, means, "{action:?} uses {}", shortcut.glyphs());
        assert!(
            !seen.contains(&keys),
            "{action:?} repeats {}",
            shortcut.glyphs()
        );
        seen.push(keys);
    }
    assert!(seen.len() >= MEANS.len(), "the standard actions are bound");
}
