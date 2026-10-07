//! A file whose plugin cannot run because the system lacks a tool, under the harness: the `Needs`
//! row stays as the passive state and gains an Install… button; the button asks, the sheet follows
//! the install, and a tool that arrives (installed here or in a terminal) opens the file again with
//! the plugin. Nothing pops up by itself, and Not Now asks nothing of the host.

#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::{Fact, FactLabel, FactValue, Helper, PixelLen, PixelSize};
use anyview_image::Rgba8;
use anyview_ui::{
    HelperEnd, HelperSource, HelperWords, HostRequest, ImagePlugins, MediaOffer, Need,
    PluginPicture,
};
use ds::prelude::ShortcutKey;
use ds_harness::{Driver, Harness, Input, Query};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use support::{Answer, FakePlayer, Requests, Wiring, folder, press, settle, wired};

/// A HEIC decoder that exists once `installed` is set: until then it says the tool is missing, as
/// the plugin does when its `heif-dec` is not on the machine.
#[derive(Debug, Default)]
struct HeifTool {
    installed: AtomicBool,
}

impl ImagePlugins for HeifTool {
    fn decode(
        &self,
        _source: &anyview_core::Source,
        _sniffed: &anyview_core::Sniffed,
        _max_area: anyview_core::PixelArea,
    ) -> PluginPicture {
        if self.installed.load(Ordering::SeqCst) {
            let size = PixelSize {
                width: PixelLen(4),
                height: PixelLen(2),
            };
            return PluginPicture::Pixels(Rgba8::new(size, vec![200; 4 * 2 * 4]).unwrap());
        }
        PluginPicture::Missing(Need {
            fact: Fact {
                label: FactLabel::Needs,
                value: FactValue::text(
                    "libheif's heif-dec (or heif-convert) for the HEIF plugin (to show it)",
                ),
            },
            helper: Some(Helper::HeicDecode),
        })
    }
}

/// What the install sheet says of each tool: a stand-in for the host's catalog.
#[derive(Debug)]
struct Words;

impl HelperSource for Words {
    fn words(&self, helper: Helper) -> Option<HelperWords> {
        let (tool, purpose, package, program) = match helper {
            Helper::HeicDecode => (
                "libheif tools",
                "open HEIC photos",
                "libheif-tools",
                "heif-dec",
            ),
            Helper::VideoPlayback => ("mpv", "play videos", "mpv", "mpv"),
            Helper::MediaProbe => (
                "FFmpeg",
                "read and convert audio and video",
                "ffmpeg",
                "ffprobe",
            ),
            Helper::RawDecode => return None,
        };
        Some(HelperWords {
            app: "Anyview".to_owned(),
            tool: tool.to_owned(),
            purpose: purpose.to_owned(),
            package: package.to_owned(),
            program: program.to_owned(),
        })
    }
}

/// A HEIC file in a folder of its own.
fn heic() -> (tempfile::TempDir, PathBuf) {
    let (dir, _) = folder(&[]);
    let path = dir.path().join("photo.heic");
    std::fs::write(
        &path,
        b"\0\0\0\x18ftypheic\0\0\0\0mif1heic and some more bytes",
    )
    .unwrap();
    let path = std::fs::canonicalize(path).unwrap();
    (dir, path)
}

struct Opened {
    harness: Harness,
    requests: Requests,
    edge: anyview_ui::Edge,
    tool: Arc<HeifTool>,
    _dir: tempfile::TempDir,
}

/// The HEIC window with its tool missing and the host ready to word the sheet.
fn open_heic() -> Opened {
    let (dir, path) = heic();
    let tool = Arc::new(HeifTool::default());
    let (mut harness, requests, edge) = wired(
        &[path],
        0,
        ds::prelude::Appearance::default(),
        Wiring {
            image_plugins: Some(Arc::clone(&tool) as Arc<dyn ImagePlugins>),
            helpers: Some(Arc::new(Words)),
            ..Wiring::default()
        },
    );
    settle(&mut harness);
    Opened {
        harness,
        requests,
        edge,
        tool,
        _dir: dir,
    }
}

fn provided(requests: &Requests) -> usize {
    requests
        .lock()
        .unwrap()
        .iter()
        .filter(|request| matches!(request, HostRequest::Provide(_)))
        .count()
}

fn click(harness: &mut Harness, selector: &str) {
    let at = harness
        .centre(selector)
        .unwrap_or_else(|| panic!("no {selector}:\n{}", harness.html()));
    harness.send(Input::click(at));
    settle(harness);
}

/// The Install… button of the card.
const INSTALL_ON_CARD: &str = ".viewer-peek .viewer-failed-actions .ds-button:first-child";
/// The sheet's default button: Install… while it asks, Close after.
const SHEET_DEFAULT: &str = ".ds-alert-footer .ds-alert-slot:first-child .ds-button";

fn phase(harness: &Harness) -> Option<String> {
    harness.attr(".ds-alert", "data-phase")
}

#[test]
fn a_missing_tool_is_a_row_with_an_install_button_and_no_sheet_of_its_own() {
    let Opened { harness, _dir, .. } = open_heic();
    let text = harness.text_of(".viewer-peek").unwrap_or_default();
    assert!(text.contains("Needs"), "the row stays: {text}");
    assert!(text.contains("Install"), "and gains the button: {text}");
    assert_eq!(
        harness.count(".viewer-peek .viewer-failed-actions .ds-button"),
        2,
        "Install… and Open With…"
    );
    assert_eq!(harness.count(".ds-alert"), 0, "nothing asks by itself");
}

#[test]
fn install_asks_then_installs_then_the_file_opens_with_the_plugin() {
    let Opened {
        mut harness,
        requests,
        edge,
        tool,
        _dir,
        ..
    } = open_heic();
    assert_eq!(harness.count(".viewer-raster"), 0, "no picture yet");

    click(&mut harness, INSTALL_ON_CARD);
    assert_eq!(phase(&harness).as_deref(), Some("ask"));
    assert_eq!(
        harness.text_of(".ds-alert-title").unwrap_or_default(),
        "Anyview needs libheif tools to open HEIC photos."
    );
    assert_eq!(provided(&requests), 0, "asking installs nothing");

    click(&mut harness, SHEET_DEFAULT);
    assert_eq!(phase(&harness).as_deref(), Some("installing"));
    assert_eq!(
        harness.count(".ds-alert-footer .ds-button"),
        0,
        "nothing to press while the system installs"
    );
    assert_eq!(
        requests
            .lock()
            .unwrap()
            .iter()
            .filter_map(|request| {
                if let HostRequest::Provide(helper) = request {
                    Some(*helper)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>(),
        [Helper::HeicDecode],
        "the host was asked, once, for this tool"
    );

    tool.installed.store(true, Ordering::SeqCst);
    edge.helped(Helper::HeicDecode, HelperEnd::Installed);
    settle(&mut harness);
    assert_eq!(harness.count(".ds-alert"), 0, "the sheet is gone");
    assert!(
        harness.count(".viewer-raster") > 0,
        "the file opened again with the plugin:\n{}",
        harness.html()
    );
    assert_eq!(harness.count(".viewer-peek"), 0, "the card is gone");
}

#[test]
fn not_now_asks_nothing_of_the_host_and_leaves_the_row() {
    let Opened {
        mut harness,
        requests,
        _dir,
        ..
    } = open_heic();
    click(&mut harness, INSTALL_ON_CARD);
    assert_eq!(phase(&harness).as_deref(), Some("ask"));
    press(&mut harness, ShortcutKey::Escape);
    assert_eq!(harness.count(".ds-alert"), 0);
    assert_eq!(provided(&requests), 0);
    assert!(harness.count(".viewer-peek") > 0, "the row is still there");
}

#[test]
fn return_installs_as_the_default_button_does() {
    let Opened {
        mut harness,
        requests,
        _dir,
        ..
    } = open_heic();
    click(&mut harness, INSTALL_ON_CARD);
    press(&mut harness, ShortcutKey::Enter);
    assert_eq!(provided(&requests), 1);
    assert_eq!(phase(&harness).as_deref(), Some("installing"));
}

#[test]
fn nothing_closes_the_sheet_while_the_system_installs() {
    let Opened {
        mut harness,
        requests,
        _dir,
        ..
    } = open_heic();
    click(&mut harness, INSTALL_ON_CARD);
    press(&mut harness, ShortcutKey::Enter);
    press(&mut harness, ShortcutKey::Escape);
    press(&mut harness, ShortcutKey::Enter);
    assert_eq!(phase(&harness).as_deref(), Some("installing"));
    assert_eq!(provided(&requests), 1, "and it asks only once");
}

#[test]
fn declining_the_password_closes_the_sheet_quietly() {
    let Opened {
        mut harness,
        edge,
        _dir,
        ..
    } = open_heic();
    click(&mut harness, INSTALL_ON_CARD);
    press(&mut harness, ShortcutKey::Enter);
    edge.helped(Helper::HeicDecode, HelperEnd::Declined);
    settle(&mut harness);
    assert_eq!(harness.count(".ds-alert"), 0);
    assert_eq!(harness.count(".ds-toast"), 0, "no toast for a no");
    assert!(harness.count(".viewer-peek") > 0, "the row is still there");
}

#[test]
fn what_the_system_could_not_do_is_said_in_the_sheet() {
    // end, phase, words in the title, words in the body
    type Case = (
        &'static str,
        HelperEnd,
        &'static str,
        &'static str,
        &'static str,
    );
    let cases: Vec<Case> = vec![
        (
            "no package",
            HelperEnd::NotFound,
            "not-found",
            "libheif tools is not in your software sources",
            "libheif-tools",
        ),
        (
            "no way to install",
            HelperEnd::Unsupported,
            "unsupported",
            "Anyview cannot install libheif tools here",
            "heif-dec",
        ),
        (
            "the package manager failed",
            HelperEnd::Failed("No network.".to_owned()),
            "failed",
            "libheif tools was not installed",
            "No network.",
        ),
    ];
    for (name, end, want_phase, title, body) in cases {
        let Opened {
            mut harness,
            edge,
            requests,
            ..
        } = open_heic();
        click(&mut harness, INSTALL_ON_CARD);
        press(&mut harness, ShortcutKey::Enter);
        edge.helped(Helper::HeicDecode, end);
        settle(&mut harness);
        assert_eq!(phase(&harness).as_deref(), Some(want_phase), "{name}");
        assert_eq!(
            harness.text_of(".ds-alert-title").unwrap_or_default(),
            title,
            "{name}"
        );
        let said = harness.text_of(".ds-alert-body").unwrap_or_default();
        assert!(said.contains(body), "{name}: {said}");
        // Return closes it, and the row is still there to try again from.
        press(&mut harness, ShortcutKey::Enter);
        assert_eq!(harness.count(".ds-alert"), 0, "{name}: closed");
        assert!(harness.count(".viewer-peek") > 0, "{name}: the row stays");
        assert_eq!(provided(&requests), 1, "{name}: nothing asked again");
    }
}

#[test]
fn a_tool_installed_elsewhere_opens_the_file_again() {
    let Opened {
        mut harness,
        edge,
        tool,
        _dir,
        ..
    } = open_heic();
    assert_eq!(harness.count(".viewer-raster"), 0);
    tool.installed.store(true, Ordering::SeqCst);
    edge.available(Helper::HeicDecode);
    settle(&mut harness);
    assert!(harness.count(".viewer-raster") > 0, "{}", harness.html());
    assert_eq!(harness.count(".viewer-peek"), 0);
}

#[test]
fn a_tool_installed_elsewhere_closes_the_sheet_that_asked_for_it() {
    let Opened {
        mut harness,
        edge,
        tool,
        requests,
        _dir,
        ..
    } = open_heic();
    click(&mut harness, INSTALL_ON_CARD);
    assert_eq!(phase(&harness).as_deref(), Some("ask"));
    tool.installed.store(true, Ordering::SeqCst);
    edge.available(Helper::HeicDecode);
    settle(&mut harness);
    assert_eq!(harness.count(".ds-alert"), 0);
    assert!(harness.count(".viewer-raster") > 0, "{}", harness.html());
    assert_eq!(
        provided(&requests),
        0,
        "nothing was installed by this window"
    );
}

#[test]
fn another_tool_appearing_changes_nothing() {
    let Opened {
        mut harness,
        edge,
        tool,
        _dir,
        ..
    } = open_heic();
    // The tool this file needs is there, but only a tool it does not need is announced.
    tool.installed.store(true, Ordering::SeqCst);
    edge.available(Helper::RawDecode);
    settle(&mut harness);
    assert_eq!(harness.count(".viewer-raster"), 0, "it did not reopen");
    assert!(harness.count(".viewer-peek") > 0);
}

#[test]
fn a_recording_without_its_player_offers_the_same_install() {
    let (dir, _) = folder(&[]);
    let path = dir.path().join("clip.mp4");
    std::fs::write(
        &path,
        b"\0\0\0\x18ftypisom\0\0\0\0isomiso2mp41 and some more bytes",
    )
    .unwrap();
    let path = std::fs::canonicalize(path).unwrap();
    let player = FakePlayer::answering(Answer::Lacks);
    let (mut harness, requests, edge) = wired(
        &[path],
        0,
        ds::prelude::Appearance::default(),
        Wiring {
            player: Some(Arc::clone(&player)),
            helpers: Some(Arc::new(Words)),
            ..Wiring::default()
        },
    );
    settle(&mut harness);
    assert_eq!(player.starts(), 0, "no player was started");
    assert_eq!(harness.count(".ds-alert"), 0, "the card, not a sheet");
    click(&mut harness, INSTALL_ON_CARD);
    assert_eq!(
        harness.text_of(".ds-alert-title").unwrap_or_default(),
        "Anyview needs mpv to play videos."
    );
    press(&mut harness, ShortcutKey::Enter);
    assert_eq!(provided(&requests), 1);
    player.answer_from_now(Answer::Plays);
    edge.helped(Helper::VideoPlayback, HelperEnd::Installed);
    settle(&mut harness);
    assert_eq!(harness.count(".ds-alert"), 0);
    assert_eq!(player.starts(), 1, "the file opened again and played");
    assert_eq!(harness.count(".viewer-peek"), 0, "the card is gone");
}

#[test]
fn a_row_the_viewer_cannot_install_for_has_no_button() {
    let (dir, _) = folder(&[]);
    let path = dir.path().join("clip.mp4");
    std::fs::write(
        &path,
        b"\0\0\0\x18ftypisom\0\0\0\0isomiso2mp41 and some more bytes",
    )
    .unwrap();
    let path = std::fs::canonicalize(path).unwrap();
    let (harness, _, _) = wired(
        &[path],
        0,
        ds::prelude::Appearance::default(),
        Wiring {
            player: Some(FakePlayer::answering(Answer::Missing)),
            ..Wiring::default()
        },
    );
    assert_eq!(
        harness.count(".viewer-peek .viewer-failed-actions .ds-button"),
        2,
        "Open With… and Show in Folder, as before"
    );
    assert!(
        !harness
            .text_of(".viewer-peek")
            .unwrap_or_default()
            .contains("Install")
    );
}

#[test]
fn the_sheet_that_says_nothing_can_be_exported_offers_to_install_what_adds_it() {
    let (dir, _) = folder(&[]);
    let path = dir.path().join("clip.mp4");
    std::fs::write(
        &path,
        b"\0\0\0\x18ftypisom\0\0\0\0isomiso2mp41 and some more bytes",
    )
    .unwrap();
    let path = std::fs::canonicalize(path).unwrap();
    let fact = Fact {
        label: FactLabel::Needs,
        value: FactValue::text("a working FFmpeg for the FFmpeg plugin (to convert it)"),
    };
    let player = FakePlayer::answering(Answer::Plays)
        .offering(MediaOffer::new(vec![], Some(fact)).installable(Helper::MediaProbe));
    let (mut harness, requests, edge) = wired(
        &[path],
        0,
        ds::prelude::Appearance::default(),
        Wiring {
            player: Some(player),
            helpers: Some(Arc::new(Words)),
            ..Wiring::default()
        },
    );
    settle(&mut harness);
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('k')));
    settle(&mut harness);
    for letter in "export".chars() {
        harness.send(Input::key(ShortcutKey::Char(letter)));
    }
    settle(&mut harness);
    press(&mut harness, ShortcutKey::Enter);
    let said = harness.text_of(".viewer-sheet").unwrap_or_default();
    assert!(said.contains("There is nothing to export yet"), "{said}");
    assert_eq!(
        harness.count(".viewer-sheet-buttons .ds-button"),
        2,
        "Install… and OK"
    );

    click(&mut harness, ".viewer-sheet-buttons .ds-button:first-child");
    assert_eq!(phase(&harness).as_deref(), Some("ask"));
    assert_eq!(
        harness.text_of(".ds-alert-title").unwrap_or_default(),
        "Anyview needs FFmpeg to read and convert audio and video."
    );
    assert_eq!(
        harness.count(".viewer-sheet"),
        0,
        "the question took its place"
    );
    press(&mut harness, ShortcutKey::Enter);
    assert_eq!(provided(&requests), 1);
    edge.helped(Helper::MediaProbe, HelperEnd::Installed);
    settle(&mut harness);
    assert_eq!(harness.count(".ds-alert"), 0);
}
