//! What the platform the viewer runs on can do for the person: the file actions that need a
//! service of the desktop (a file chooser, a print dialog, a way to share, a file manager). The
//! binary says which it has, from the platform parts it was built with; the views offer an
//! action only when its service is there, so no control is shown that can do nothing. The views
//! never ask which operating system they run on, only this data.

use anyview_core::FileAction;
use ds_core::word::Word;

/// One service of the desktop that a file action stands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum DesktopService {
    /// A dialog to choose files with: Open… and ⌘O, and the welcome window's Open….
    FileChooser,
    /// A print dialog: Print… and ⌘P.
    PrintDialog,
    /// A way to send a file to someone: Share….
    Sharing,
    /// A file manager to show a file in: Show in Folder.
    FileManager,
}

/// The desktop services the platform has, a set of [`DesktopService`]. The default is all of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlatformAbilities(u8);

impl PlatformAbilities {
    /// Every service: the default, and what a desktop with all its services has.
    pub const ALL: PlatformAbilities = PlatformAbilities(0b1111);

    /// No service: a platform with none of the desktop's services.
    pub const NONE: PlatformAbilities = PlatformAbilities(0);

    /// Just `services`.
    #[must_use]
    pub fn of(services: impl IntoIterator<Item = DesktopService>) -> PlatformAbilities {
        services
            .into_iter()
            .fold(PlatformAbilities::NONE, PlatformAbilities::with)
    }

    /// Whether `service` is there.
    pub fn has(self, service: DesktopService) -> bool {
        self.0 & bit(service) != 0
    }

    /// The same set with `service` in it.
    #[must_use]
    pub fn with(self, service: DesktopService) -> PlatformAbilities {
        PlatformAbilities(self.0 | bit(service))
    }

    /// The same set without `service`.
    #[must_use]
    pub fn without(self, service: DesktopService) -> PlatformAbilities {
        PlatformAbilities(self.0 & !bit(service))
    }

    /// Whether `action` can be done here. An action no desktop service stands behind always can.
    pub fn offers(self, action: FileAction) -> bool {
        match action {
            FileAction::Open => self.has(DesktopService::FileChooser),
            FileAction::Print => self.has(DesktopService::PrintDialog),
            FileAction::Share => self.has(DesktopService::Sharing),
            FileAction::RevealInFolder => self.has(DesktopService::FileManager),
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

/// The bit that stands for `service` in a set.
fn bit(service: DesktopService) -> u8 {
    match service {
        DesktopService::FileChooser => 0b0001,
        DesktopService::PrintDialog => 0b0010,
        DesktopService::Sharing => 0b0100,
        DesktopService::FileManager => 0b1000,
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

    #[test]
    fn each_desktop_action_follows_its_own_service_and_the_rest_always_stand() {
        // action, the service it stands on
        const BEHIND: &[(FileAction, DesktopService)] = &[
            (FileAction::Open, DesktopService::FileChooser),
            (FileAction::Print, DesktopService::PrintDialog),
            (FileAction::Share, DesktopService::Sharing),
            (FileAction::RevealInFolder, DesktopService::FileManager),
        ];
        assert_eq!(
            PlatformAbilities::of(DesktopService::ALL.iter().copied()),
            PlatformAbilities::ALL
        );
        for action in FileAction::ALL {
            assert!(PlatformAbilities::ALL.offers(*action), "{action:?}");
            let behind = BEHIND.iter().any(|(a, _)| a == action);
            assert_eq!(
                PlatformAbilities::NONE.offers(*action),
                !behind,
                "{action:?} with no service"
            );
        }
        for (gone, service) in BEHIND {
            let without = PlatformAbilities::ALL.without(*service);
            let lost: Vec<FileAction> = FileAction::ALL
                .iter()
                .copied()
                .filter(|action| !without.offers(*action))
                .collect();
            assert_eq!(lost, [*gone]);
            assert_eq!(without.with(*service), PlatformAbilities::ALL);
        }
    }
}
