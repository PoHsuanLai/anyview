//! What the platform the viewer runs on can do for the person: the file actions that need a
//! service of the desktop (a file chooser, a print dialog, a way to share, a list of applications,
//! a file manager). The binary says which it has, from the platform parts it was built with; the
//! views offer an action only when its ability is there, so no control is shown that can do
//! nothing. The views never ask which operating system they run on, only this data.

use anyview_core::FileAction;

/// The desktop services behind the file actions, each there or not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformAbilities {
    /// A dialog to choose files with: Open… and ⌘O, and the welcome window's Open….
    pub pick_files: bool,
    /// A print dialog: Print… and ⌘P.
    pub print: bool,
    /// A way to send a file to someone: Share….
    pub share: bool,
    /// A list of the applications that open a file: Open With….
    pub open_with: bool,
    /// A file manager to show a file in: Show in Folder.
    pub reveal: bool,
}

impl PlatformAbilities {
    /// Every ability: the default, and what a desktop with all its services has.
    pub const ALL: PlatformAbilities = PlatformAbilities {
        pick_files: true,
        print: true,
        share: true,
        open_with: true,
        reveal: true,
    };

    /// No ability: a platform with none of the desktop's services.
    pub const NONE: PlatformAbilities = PlatformAbilities {
        pick_files: false,
        print: false,
        share: false,
        open_with: false,
        reveal: false,
    };

    /// Whether `action` can be done here. An action no desktop service stands behind always can.
    pub fn offers(self, action: FileAction) -> bool {
        match action {
            FileAction::Open => self.pick_files,
            FileAction::Print => self.print,
            FileAction::Share => self.share,
            FileAction::OpenWith => self.open_with,
            FileAction::RevealInFolder => self.reveal,
            FileAction::CopyFile
            | FileAction::CopyPath
            | FileAction::Rename
            | FileAction::Duplicate
            | FileAction::MoveToTrash
            | FileAction::Export
            | FileAction::SaveCopy
            | FileAction::RevertTo
            | FileAction::RotateLeft
            | FileAction::RotateRight
            | FileAction::FlipHorizontal
            | FileAction::FlipVertical
            | FileAction::ConvertTo
            | FileAction::PlayInMiniWindow
            | FileAction::PlayInBackground => true,
        }
    }
}

impl Default for PlatformAbilities {
    fn default() -> PlatformAbilities {
        PlatformAbilities::ALL
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ds_core::word::Word;

    #[test]
    fn each_desktop_action_follows_its_own_ability_and_the_rest_always_stand() {
        // action, the ability taken away from a full set
        const BEHIND: &[(FileAction, fn(&mut PlatformAbilities))] = &[
            (FileAction::Open, |a| a.pick_files = false),
            (FileAction::Print, |a| a.print = false),
            (FileAction::Share, |a| a.share = false),
            (FileAction::OpenWith, |a| a.open_with = false),
            (FileAction::RevealInFolder, |a| a.reveal = false),
        ];
        for action in FileAction::ALL {
            assert!(PlatformAbilities::ALL.offers(*action), "{action:?}");
            let behind = BEHIND.iter().any(|(a, _)| a == action);
            assert_eq!(
                PlatformAbilities::NONE.offers(*action),
                !behind,
                "{action:?} with no ability"
            );
        }
        for (gone, take_away) in BEHIND {
            let mut without = PlatformAbilities::ALL;
            take_away(&mut without);
            let lost: Vec<FileAction> = FileAction::ALL
                .iter()
                .copied()
                .filter(|action| !without.offers(*action))
                .collect();
            assert_eq!(lost, [*gone]);
        }
    }
}
