//! The one list of actions a file offers, shared by the launcher and the viewer.

use ds_core::word::Word;

/// Something a person can do to a file. The launcher's row menu, the viewer's ⌘K palette and its
/// menus all list from this one set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, Word)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum FileAction {
    /// Open in the viewer.
    Open,
    /// Open in another program, chosen from a list.
    OpenWith,
    /// Show the file in the file manager.
    RevealInFolder,
    /// Copy the file itself to the clipboard.
    CopyFile,
    /// Copy the file's path as text.
    CopyPath,
    /// Send the file to another person or program.
    Share,
    /// Give the file another name.
    Rename,
    /// Make a copy next to the file.
    Duplicate,
    /// Move the file to the trash.
    MoveToTrash,
    /// Print the file.
    Print,
    /// Write the content out in another form, through the export sheet.
    Export,
    /// Save the edited content as a new file.
    SaveCopy,
    /// Restore a kept version of the file.
    RevertTo,
    /// Turn the content a quarter turn counter-clockwise.
    RotateLeft,
    /// Turn the content a quarter turn clockwise.
    RotateRight,
    /// Mirror the content left to right.
    FlipHorizontal,
    /// Mirror the content top to bottom.
    FlipVertical,
    /// Play the audio with no window.
    PlayInBackground,
    /// Play the video in a small window above the others.
    PlayInMiniWindow,
    /// Open a menu of formats and write the file in one of them.
    ConvertTo,
}
