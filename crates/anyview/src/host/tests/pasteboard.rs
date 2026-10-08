use crate::host::pasteboard::{PasteError, Pasteboard, copy_text};
use anyview_ui::Notice;
use std::cell::RefCell;

/// A clipboard that keeps what it was given, as a desktop's does once the owner holds on.
#[derive(Default)]
struct Held(RefCell<Option<String>>);

impl Pasteboard for Held {
    fn put_text(&self, text: &str) -> Result<(), PasteError> {
        *self.0.borrow_mut() = Some(text.to_owned());
        Ok(())
    }
}

/// A clipboard the desktop refuses.
struct Refusing;

impl Pasteboard for Refusing {
    fn put_text(&self, _: &str) -> Result<(), PasteError> {
        Err(PasteError("no selection owner".into()))
    }
}

#[test]
fn copy_path_leaves_the_path_on_the_clipboard_and_says_so_quietly() {
    let board = Held::default();
    let copied = copy_text(&board, "/home/me/photo 1.png");
    assert_eq!(board.0.borrow().as_deref(), Some("/home/me/photo 1.png"));
    assert_eq!(copied.problem, None);
    assert_eq!(copied.notice, Notice::say("Path copied"));
}

#[test]
fn a_clipboard_that_refuses_is_logged_and_the_person_is_told_plainly() {
    let copied = copy_text(&Refusing, "/p");
    assert_eq!(
        copied.problem.as_deref(),
        Some("cannot copy: no selection owner")
    );
    assert_eq!(
        copied.notice,
        Notice::say("Couldn\u{2019}t copy to the clipboard")
    );
}

/// Run by hand inside a private compositor (`WAYLAND_DISPLAY` set to its socket): sets the
/// selection through the real clipboard, then holds the process for `ANYVIEW_PASTE_HOLD`
/// seconds so `wl-paste` can read it.
#[test]
#[ignore = "needs a compositor of its own; never the desktop's"]
fn the_system_clipboard_holds_the_path_for_another_client() {
    use crate::host::SystemPasteboard;
    let copied = copy_text(&SystemPasteboard, "/home/me/photo 1.png");
    assert_eq!(copied.problem, None);
    let hold = std::env::var("ANYVIEW_PASTE_HOLD")
        .ok()
        .and_then(|seconds| seconds.parse().ok())
        .unwrap_or(0);
    std::thread::sleep(std::time::Duration::from_secs(hold));
}
