//! Decode and thumbnail through the host's `PluginRunner`, against the plugin built for this run and
//! stand-ins for LibRaw's `dcraw_emu` and for `dcraw`.

#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::{PixelArea, PixelLen};
use anyview_platform::{PlatformError, PluginRunner, Timeouts};
use anyview_plugin_protocol::{
    Capability, DecodeRequest, ErrorCode, HostMessage, PluginMessage, read_frame, write_frame,
};
use std::process::{Command, Stdio};
use std::time::Duration;
use support::{Scratch, file, gone, with_dcraw, with_emu};

fn failure(error: &PlatformError, wanted: ErrorCode) -> bool {
    matches!(error, PlatformError::PluginFailed { code, .. } if *code == wanted)
}

#[test]
fn a_development_is_the_ppm_dcraw_emu_wrote_beside_its_input_in_a_scratch_folder() {
    let scratch = Scratch::new();
    let ppm = scratch.ppm("dev.ppm", 64, 32);
    let log = scratch.path("args.log");
    let emu = scratch.fake_emu(&format!(
        "echo \"$*|$last\" > {}; cp {} \"$last.ppm\"",
        log.display(),
        ppm.display()
    ));
    let plugin = scratch.install(&with_emu(&emu));
    let raw = scratch.raw();
    let runner = PluginRunner::default();
    let whole = runner
        .decode(&plugin, &file(&raw), PixelArea(10_000))
        .unwrap();
    assert_eq!((whole.size().width.0, whole.size().height.0), (64, 32));
    assert_eq!(&whole.rgba()[..4], &[255, 0, 0, 255], "left is red");
    let said = std::fs::read_to_string(&log).unwrap();
    let (args, input) = said.trim().split_once('|').unwrap();
    assert!(args.starts_with("-w "), "{args}");
    assert!(
        input.ends_with("/in.cr2"),
        "the tool works on a link in a scratch folder: {input}"
    );
    assert!(
        !raw.with_extension("cr2.ppm").exists(),
        "nothing is written beside the file"
    );
    let small = runner.decode(&plugin, &file(&raw), PixelArea(200)).unwrap();
    assert!(small.size().area() <= PixelArea(200), "{:?}", small.size());
}

#[test]
fn dcraw_writes_to_standard_output() {
    let scratch = Scratch::new();
    let ppm = scratch.ppm("dev.ppm", 40, 20);
    let jpeg = scratch.jpeg("preview.jpg", 30, 20);
    let dcraw = scratch.fake_dcraw(&format!(
        "case \"$*\" in *-e*) cat {};; *) cat {};; esac",
        jpeg.display(),
        ppm.display()
    ));
    let plugin = scratch.install(&with_dcraw(&dcraw));
    let raw = file(&scratch.raw());
    let runner = PluginRunner::default();
    let developed = runner.decode(&plugin, &raw, PixelArea(1_000_000)).unwrap();
    assert_eq!(
        (developed.size().width.0, developed.size().height.0),
        (40, 20)
    );
    let preview = runner.thumbnail(&plugin, &raw, PixelLen(15)).unwrap();
    assert_eq!((preview.size().width.0, preview.size().height.0), (15, 10));
    assert_eq!(
        &preview.rgba()[..4],
        &[0, 200, 0, 255],
        "the preview, not the development"
    );
}

#[test]
fn a_thumbnail_is_the_embedded_preview_and_without_one_a_small_development() {
    let scratch = Scratch::new();
    let jpeg = scratch.jpeg("preview.jpg", 60, 40);
    let ppm = scratch.ppm("dev.ppm", 64, 32);
    let preview_emu = scratch.fake_emu(&format!(
        "case \"$1\" in -e) cp {} \"$last.thumb.jpg\";; *) cp {} \"$last.ppm\";; esac",
        jpeg.display(),
        ppm.display()
    ));
    let plugin = scratch.install(&with_emu(&preview_emu));
    let raw = file(&scratch.raw());
    let runner = PluginRunner::default();
    let thumb = runner.thumbnail(&plugin, &raw, PixelLen(30)).unwrap();
    assert_eq!((thumb.size().width.0, thumb.size().height.0), (30, 20));
    assert_eq!(&thumb.rgba()[..4], &[0, 200, 0, 255]);

    // Extraction fails (a file with no preview): the development, scaled, stands in.
    let no_preview = scratch.fake_emu(&format!(
        "case \"$1\" in -e) echo 'no thumbnail' >&2; exit 1;; *) cp {} \"$last.ppm\";; esac",
        ppm.display()
    ));
    let plugin = scratch.install(&with_emu(&no_preview));
    let thumb = runner.thumbnail(&plugin, &raw, PixelLen(32)).unwrap();
    assert_eq!((thumb.size().width.0, thumb.size().height.0), (32, 16));
}

#[test]
fn a_tool_that_is_not_there_leaves_the_plugin_offering_nothing() {
    let scratch = Scratch::new();
    let plugin = scratch.install(&with_emu(&scratch.path("no-such-dcraw-emu")));
    let runner = PluginRunner::default();
    // A named tool that is absent is never replaced by another from the search path.
    let hello = runner.hello(&plugin).unwrap();
    let has_dcraw_on_path = std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join("dcraw").is_file()));
    if !has_dcraw_on_path {
        assert!(hello.provides.is_empty(), "{hello:?}");
        let error = runner
            .decode(&plugin, &file(&scratch.raw()), PixelArea(1_000))
            .unwrap_err();
        assert!(
            matches!(
                &error,
                PlatformError::PluginLacks {
                    capability: Capability::Decode,
                    ..
                }
            ),
            "{error:?}"
        );
    }
}

#[test]
fn the_hello_says_what_this_machine_has() {
    let scratch = Scratch::new();
    let emu = scratch.fake_emu("true");
    let plugin = scratch.install(&with_emu(&emu));
    assert_eq!(
        PluginRunner::default().hello(&plugin).unwrap().provides,
        [Capability::Thumbnail, Capability::Decode]
    );
}

#[test]
fn a_tool_that_fails_or_writes_rubbish_is_an_error_naming_it() {
    let scratch = Scratch::new();
    let raw = file(&scratch.raw());
    let runner = PluginRunner::default();
    let failing = scratch.fake_emu("echo 'Unsupported file format' >&2; exit 1");
    let plugin = scratch.install(&with_emu(&failing));
    let error = runner.decode(&plugin, &raw, PixelArea(1_000)).unwrap_err();
    assert!(failure(&error, ErrorCode::Failed), "{error:?}");
    assert!(
        error.to_string().contains("Unsupported file format"),
        "{error}"
    );

    let rubbish = scratch.fake_emu("echo not a picture > \"$last.ppm\"");
    let plugin = scratch.install(&with_emu(&rubbish));
    let error = runner.decode(&plugin, &raw, PixelArea(1_000)).unwrap_err();
    assert!(failure(&error, ErrorCode::Corrupt), "{error:?}");

    let nothing = scratch.fake_emu("true");
    let plugin = scratch.install(&with_emu(&nothing));
    let error = runner.decode(&plugin, &raw, PixelArea(1_000)).unwrap_err();
    assert!(failure(&error, ErrorCode::Failed), "{error:?}");

    let absent = file(&scratch.path("absent.cr2"));
    let error = runner
        .decode(&plugin, &absent, PixelArea(1_000))
        .unwrap_err();
    assert!(failure(&error, ErrorCode::Unreadable), "{error:?}");
}

#[test]
fn a_host_that_gives_up_waiting_kills_the_tool_with_the_plugin() {
    let scratch = Scratch::new();
    let pid = scratch.path("pid");
    let emu = scratch.fake_emu(&format!("echo $$ > {}; sleep 60", pid.display()));
    let plugin = scratch.install(&with_emu(&emu));
    let runner = PluginRunner::new(Timeouts {
        silence: Duration::from_millis(700),
        ..Timeouts::default()
    });
    let error = runner
        .decode(&plugin, &file(&scratch.raw()), PixelArea(1_000))
        .unwrap_err();
    assert!(
        matches!(error, PlatformError::PluginSilent { .. }),
        "{error:?}"
    );
    assert!(gone(&pid), "the stand-in tool is still running");
}

#[test]
fn a_host_that_goes_away_or_cancels_stops_the_tool() {
    for cancel in [false, true] {
        let scratch = Scratch::new();
        let pid = scratch.path("pid");
        let emu = scratch.fake_emu(&format!("echo $$ > {}; sleep 60", pid.display()));
        let mut child = Command::new(support::PLUGIN)
            .args(with_emu(&emu))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let hello = read_frame::<_, PluginMessage>(&mut stdout).unwrap();
        assert!(matches!(hello.message, PluginMessage::Hello(_)));
        let request = HostMessage::Decode(DecodeRequest {
            path: scratch.raw(),
            max_area: 1_000,
        });
        write_frame(&mut stdin, &request, &[]).unwrap();
        for _ in 0..200 {
            if std::fs::read_to_string(&pid).is_ok_and(|text| !text.is_empty()) {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        if cancel {
            write_frame(&mut stdin, &HostMessage::Cancel, &[]).unwrap();
        }
        drop(stdin);
        assert!(gone(&pid), "cancel={cancel}: the tool is still running");
        let _ = child.wait();
    }
}
