//! A HEIC file on a machine without libheif's tools, end to end under the harness with the binary's
//! own wiring: the real HEIF plugin, the shipped helpers file, and a fake installer that never
//! touches a package manager. The file shows as its facts with the `Needs` row and an Install…
//! button; the sheet asks; the (fake) install leaves the tool; the file opens again with the
//! plugin, in the same window.

#![allow(clippy::unwrap_used)]

mod support;

use anyview::host::{HelperHost, ImageHost, PluginRegistry};
use anyview::media::MediaPlugins;
use anyview_platform::PluginRunner;
use anyview_plugin::{Candidate, Manifest, Origin, Plugins, Readiness};
use ds_harness::{Driver, Input, Query};
use ds_helpers::{Catalog, Environment, FakeInstaller, Installer, Outcome};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use support::{Wired, open_wired, until};

/// The manifest template the installer fills in, and the file of tools the viewer ships.
const TEMPLATE: &str = include_str!("../../../dist/plugins/anyview-heif.toml.in");
const HELPERS: &str = include_str!("../../../dist/helpers/anyview.toml");

/// The HEIF plugin built beside the test binaries, when this run built it (`cargo test
/// --workspace` does; `cargo build -p anyview-heif` does otherwise).
fn heif_program() -> Option<PathBuf> {
    let built = std::env::current_exe()
        .ok()?
        .parent()?
        .parent()?
        .join("anyview-heif");
    if built.is_file() {
        Some(built)
    } else {
        eprintln!("SKIPPED: the HEIF plugin is not built (cargo build -p anyview-heif)");
        None
    }
}

/// The manifest for `program`, told to use `dec` as libheif's decoder.
fn heif_plugins(program: &Path, dec: &Path) -> Plugins {
    let text = TEMPLATE
        .replace(
            "@PREFIX@/libexec/anyview/anyview-heif",
            &program.display().to_string(),
        )
        .replace(
            "args = []",
            &format!("args = [\"--heif-dec\", {:?}]", dec.display().to_string()),
        );
    Plugins::resolve(vec![Candidate {
        manifest: Manifest::parse(&text).unwrap(),
        origin: Origin::User,
        readiness: Readiness::Ready,
    }])
}

/// A 64 by 32 PNG, left half red and right half blue.
fn png(at: &Path) {
    let image = image::RgbImage::from_fn(64, 32, |x, _| {
        if x < 32 {
            image::Rgb([255, 0, 0])
        } else {
            image::Rgb([0, 0, 255])
        }
    });
    image.save(at).unwrap();
}

fn click(rig: &mut support::Rig, selector: &str) {
    let at = rig.harness.centre(selector).unwrap_or_else(|| {
        panic!("no {selector}:\n{}", rig.harness.html());
    });
    rig.harness.send(Input::click(at));
}

#[test]
fn a_heic_without_its_tool_installs_it_from_the_sheet_and_opens_in_place() {
    let Some(program) = heif_program() else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let tools = dir.path().join("tools");
    let dec = tools.join("heif-dec");
    let file = dir.path().join("photo.heic");
    std::fs::write(
        &file,
        b"\0\0\0\x18ftypheic\0\0\0\0mif1heic and some more bytes",
    )
    .unwrap();
    let still = dir.path().join("still.png");
    png(&still);
    std::fs::write(dir.path().join("os-release"), "ID=fedora\n").unwrap();

    let registry = PluginRegistry::fixed(heif_plugins(&program, &dec));
    let fake = FakeInstaller::new(Outcome::Installed).leaving(tools.clone(), &["heif-dec"]);
    let helpers = Arc::new(HelperHost::new(
        Catalog::parse(HELPERS).unwrap(),
        Environment {
            path: tools.clone().into(),
            os_release: dir.path().join("os-release"),
        },
        Installer::Fake(fake.clone()),
        registry.clone(),
    ));
    let images =
        ImageHost::following(registry, PluginRunner::default()).offering(Arc::clone(&helpers));
    let mut rig = open_wired(
        &file,
        dir.path(),
        Arc::new(MediaPlugins::default()),
        Wired {
            images: Some(images),
            helpers: Some(helpers),
            sizer: None,
        },
    );

    // The tool is missing: the facts card with its row and the button, and no sheet by itself.
    until(&mut rig.harness, "the facts card", |harness| {
        harness.count(".viewer-peek") > 0
    });
    let card = rig.harness.text_of(".viewer-peek").unwrap_or_default();
    assert!(
        card.contains("Needs") && card.contains("heif-dec"),
        "{card}"
    );
    assert_eq!(rig.harness.count(".ds-alert"), 0, "nothing asks by itself");
    assert!(fake.asked().is_empty());

    // Install… opens the question, worded from the shipped file.
    click(
        &mut rig,
        ".viewer-peek .viewer-failed-actions .ds-button:first-child",
    );
    until(&mut rig.harness, "the question", |harness| {
        harness.count(".ds-alert") > 0
    });
    assert_eq!(
        rig.harness.text_of(".ds-alert-title").unwrap_or_default(),
        "Anyview needs libheif tools to open HEIC photos."
    );
    assert!(fake.asked().is_empty(), "asking installs nothing");

    // The fake leaves an empty stand-in for the tool; the person's machine would have the real one,
    // so the stand-in takes the real one's place the moment it lands.
    let writer = {
        let (dec, still) = (dec.clone(), still.clone());
        std::thread::spawn(move || {
            let started = std::time::Instant::now();
            while !dec.exists() && started.elapsed().as_secs() < 20 {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            std::fs::write(&dec, format!("#!/bin/sh\ncp {} \"$2\"\n", still.display())).unwrap();
        })
    };
    click(
        &mut rig,
        ".ds-alert-footer .ds-alert-slot:first-child .ds-button",
    );
    until(
        &mut rig.harness,
        "the file opened with the plugin",
        |harness| harness.count(".viewer-raster") > 0,
    );
    writer.join().unwrap();
    assert_eq!(rig.harness.count(".ds-alert"), 0, "the sheet is gone");
    assert_eq!(rig.harness.count(".viewer-peek"), 0, "the card is gone");
    let asked = fake.asked();
    assert_eq!(asked.len(), 1, "the installer was asked once");
    assert_eq!(asked[0].candidates[0].as_str(), "libheif-tools");
    assert!(
        rig.workforce.notices().is_empty(),
        "every job ran to its end and posted its result"
    );
}
