//! Decode and thumbnail through the host's `PluginRunner`, against the plugin built for this run and
//! stand-ins for libheif's tools.

#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::{PixelArea, PixelLen};
use anyview_platform::{PlatformError, PluginRunner, Timeouts};
use anyview_plugin_protocol::{
    Capability, DecodeRequest, ErrorCode, HostMessage, PluginMessage, read_frame, write_frame,
};
use std::process::{Command, Stdio};
use std::time::Duration;
use support::{Scratch, file, gone, with_dec, with_thumbnailer};

fn failure(error: &PlatformError, wanted: ErrorCode) -> bool {
    matches!(error, PlatformError::PluginFailed { code, .. } if *code == wanted)
}

#[test]
fn a_decode_is_the_png_the_tool_wrote_within_the_pixel_budget() {
    let scratch = Scratch::new();
    let png = scratch.png("wide.png", 64, 32);
    let log = scratch.path("args.log");
    let dec = scratch.fake_dec(&format!(
        "echo \"$1|$2\" > {}; cp {} \"$2\"",
        log.display(),
        png.display()
    ));
    let plugin = scratch.install(&with_dec(&dec));
    let heic = scratch.heic();
    let runner = PluginRunner::default();
    let whole = runner
        .decode(&plugin, &file(&heic), PixelArea(10_000))
        .unwrap();
    assert_eq!((whole.size().width.0, whole.size().height.0), (64, 32));
    assert_eq!(whole.rgba().len(), 64 * 32 * 4);
    assert_eq!(&whole.rgba()[..4], &[255, 0, 0, 255], "left is red");
    assert_eq!(
        &whole.rgba()[60 * 4..61 * 4],
        &[0, 0, 255, 255],
        "right is blue"
    );
    let said = std::fs::read_to_string(&log).unwrap();
    let (input, output) = said.trim().split_once('|').unwrap();
    assert_eq!(input, heic.display().to_string());
    assert!(output.ends_with("/out.png"), "{output}");
    let small = runner
        .decode(&plugin, &file(&heic), PixelArea(200))
        .unwrap();
    assert!(small.size().area() <= PixelArea(200), "{:?}", small.size());
}

#[test]
fn a_file_of_several_pictures_gives_the_first_numbered_one() {
    let scratch = Scratch::new();
    let first = scratch.png("one.png", 8, 8);
    let second = scratch.png("two.png", 16, 16);
    let dec = scratch.fake_dec(&format!(
        "d=$(dirname \"$2\"); cp {} \"$d/out-2.png\"; cp {} \"$d/out-1.png\"",
        second.display(),
        first.display()
    ));
    let plugin = scratch.install(&with_dec(&dec));
    let picture = PluginRunner::default()
        .decode(&plugin, &file(&scratch.heic()), PixelArea(1_000_000))
        .unwrap();
    assert_eq!((picture.size().width.0, picture.size().height.0), (8, 8));
}

#[test]
fn a_thumbnail_comes_from_the_thumbnailer_and_falls_back_to_a_decode() {
    let scratch = Scratch::new();
    let png = scratch.png("p.png", 64, 32);
    let log = scratch.path("thumb.log");
    let thumbnailer = scratch.script(
        "fake-thumbnailer",
        &format!(
            "echo \"$1 $2\" > {}; cp {} \"$4\"",
            log.display(),
            png.display()
        ),
    );
    let plugin = scratch.install(&with_thumbnailer(&thumbnailer));
    let heic = file(&scratch.heic());
    let runner = PluginRunner::default();
    let thumb = runner.thumbnail(&plugin, &heic, PixelLen(32)).unwrap();
    assert_eq!((thumb.size().width.0, thumb.size().height.0), (32, 16));
    assert_eq!(std::fs::read_to_string(&log).unwrap().trim(), "-s 32");

    // A thumbnailer that fails, and a decoder that works.
    let broken = scratch.script("broken-thumbnailer", "echo no >&2; exit 1");
    let dec = scratch.fake_dec(&format!("cp {} \"$2\"", png.display()));
    let mut args = with_dec(&dec);
    args.extend(with_thumbnailer(&broken));
    let plugin = scratch.install(&args);
    let thumb = runner.thumbnail(&plugin, &heic, PixelLen(16)).unwrap();
    assert_eq!((thumb.size().width.0, thumb.size().height.0), (16, 8));
}

#[test]
fn a_thumbnail_with_only_the_decoder_is_a_scaled_decode() {
    let scratch = Scratch::new();
    let png = scratch.png("p.png", 64, 32);
    let dec = scratch.fake_dec(&format!("cp {} \"$2\"", png.display()));
    let plugin = scratch.install(&with_dec(&dec));
    let thumb = PluginRunner::default()
        .thumbnail(&plugin, &file(&scratch.heic()), PixelLen(32))
        .unwrap();
    assert_eq!((thumb.size().width.0, thumb.size().height.0), (32, 16));
}

#[test]
fn a_tool_that_is_not_there_leaves_the_plugin_offering_nothing() {
    let scratch = Scratch::new();
    let plugin = scratch.install(&with_dec(&scratch.path("no-such-heif-dec")));
    let runner = PluginRunner::default();
    let hello = runner.hello(&plugin).unwrap();
    assert!(hello.provides.is_empty(), "{hello:?}");
    let error = runner
        .decode(&plugin, &file(&scratch.heic()), PixelArea(1_000))
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

#[test]
fn the_hello_says_what_this_machine_has() {
    let scratch = Scratch::new();
    let dec = scratch.fake_dec("true");
    let thumbnailer = scratch.script("t", "true");
    let runner = PluginRunner::default();
    let both = scratch.install(&[with_dec(&dec), with_thumbnailer(&thumbnailer)].concat());
    assert_eq!(
        runner.hello(&both).unwrap().provides,
        [Capability::Thumbnail, Capability::Decode]
    );
    let only = scratch.install(
        &with_thumbnailer(&thumbnailer)
            .into_iter()
            .chain(with_dec(&scratch.path("absent")))
            .collect::<Vec<_>>(),
    );
    assert_eq!(
        runner.hello(&only).unwrap().provides,
        [Capability::Thumbnail]
    );
}

#[test]
fn a_tool_that_fails_or_writes_rubbish_is_an_error_naming_it() {
    let scratch = Scratch::new();
    let heic = file(&scratch.heic());
    let runner = PluginRunner::default();
    let failing = scratch.fake_dec("echo 'Unsupported codec: hevc' >&2; exit 1");
    let plugin = scratch.install(&with_dec(&failing));
    let error = runner.decode(&plugin, &heic, PixelArea(1_000)).unwrap_err();
    assert!(failure(&error, ErrorCode::Failed), "{error:?}");
    assert!(
        error.to_string().contains("Unsupported codec: hevc"),
        "{error}"
    );

    let rubbish = scratch.fake_dec("echo not a png > \"$2\"");
    let plugin = scratch.install(&with_dec(&rubbish));
    let error = runner.decode(&plugin, &heic, PixelArea(1_000)).unwrap_err();
    assert!(failure(&error, ErrorCode::Corrupt), "{error:?}");

    let nothing = scratch.fake_dec("true");
    let plugin = scratch.install(&with_dec(&nothing));
    let error = runner.decode(&plugin, &heic, PixelArea(1_000)).unwrap_err();
    assert!(failure(&error, ErrorCode::Failed), "{error:?}");

    let absent = scratch.path("absent.heic");
    let error = runner
        .decode(&plugin, &file(&absent), PixelArea(1_000))
        .unwrap_err();
    assert!(failure(&error, ErrorCode::Unreadable), "{error:?}");
}

#[test]
fn a_host_that_gives_up_waiting_kills_the_tool_with_the_plugin() {
    let scratch = Scratch::new();
    let pid = scratch.path("pid");
    let dec = scratch.fake_dec(&format!("echo $$ > {}; sleep 60", pid.display()));
    let plugin = scratch.install(&with_dec(&dec));
    let runner = PluginRunner::new(Timeouts::default().with_silence(Duration::from_millis(700)));
    let error = runner
        .decode(&plugin, &file(&scratch.heic()), PixelArea(1_000))
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
        let dec = scratch.fake_dec(&format!("echo $$ > {}; sleep 60", pid.display()));
        let args = with_dec(&dec);
        let mut child = Command::new(support::PLUGIN)
            .args(&args)
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
            path: scratch.heic(),
            max_area: 1_000,
        });
        write_frame(&mut stdin, &request, &[]).unwrap();
        for _ in 0..200 {
            if pid.exists() && std::fs::read_to_string(&pid).is_ok_and(|text| !text.is_empty()) {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        if cancel {
            write_frame(&mut stdin, &HostMessage::Cancel, &[]).unwrap();
        }
        drop(stdin);
        assert!(gone(&pid), "cancel={cancel}: the tool is still running");
        let status = child.wait().unwrap();
        // The plugin answered `Cancelled` (or found nobody to tell) and ended.
        let _ = status;
    }
}
