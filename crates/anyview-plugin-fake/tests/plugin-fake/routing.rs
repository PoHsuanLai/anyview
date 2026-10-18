//! The routing seam end to end: a kind with no built-in back end is asked of its plugin, or says
//! which package would serve it.

use crate::support;

use anyview_core::{FactLabel, FormatKind, PixelLen};
use anyview_platform::{PluginFacts, PluginRunner};
use anyview_plugin::{MissingPlugin, Subject};
use support::{Scratch, Where};

#[test]
fn a_book_with_a_plugin_gets_the_plugins_facts() {
    let scratch = Scratch::new();
    scratch.install(Where::System, "fake", 1, &[]);
    let path = scratch.book("a.book", "Emma");
    let answer = PluginRunner::default()
        .peek_facts(&scratch.plugins(), &support::book(), &path)
        .unwrap();
    let PluginFacts::Facts(facts) = answer else {
        panic!("facts, got {answer:?}");
    };
    assert_eq!(facts.value(FactLabel::Title).unwrap().as_str(), "Emma");
}

#[test]
fn a_video_with_no_plugin_names_the_package_to_install() {
    let scratch = Scratch::new();
    scratch.install(Where::System, "fake", 1, &[]);
    let path = scratch.book("clip.mp4", "not really a video");
    let video = Subject {
        kind: FormatKind::Video,
        mime: None,
    };
    let answer = PluginRunner::default()
        .peek_facts(&scratch.plugins(), &video, &path)
        .unwrap();
    let PluginFacts::Missing(missing) = answer else {
        panic!("missing, got {answer:?}");
    };
    let MissingPlugin { package, .. } = missing;
    assert_eq!(package.name, "anyview-ffmpeg");
    let fact = missing.fact();
    assert_eq!(fact.label, FactLabel::Needs);
    assert!(fact.value.as_str().starts_with("anyview-ffmpeg"));
}

#[test]
fn a_kind_no_package_is_known_for_is_unserved() {
    let scratch = Scratch::new();
    let path = scratch.book("a.pdf", "x");
    let pdf = Subject {
        kind: FormatKind::Pdf,
        mime: None,
    };
    let answer = PluginRunner::default()
        .peek_facts(&scratch.plugins(), &pdf, &path)
        .unwrap();
    assert_eq!(answer, PluginFacts::Unserved);
}

#[test]
fn a_poster_comes_from_the_plugin_that_serves_the_kind_and_from_none_otherwise() {
    let scratch = Scratch::new();
    scratch.install(Where::System, "fake", 1, &[]);
    let runner = PluginRunner::default();
    let book = scratch.book("a.book", "Emma");
    let served = runner
        .poster(&scratch.plugins(), &support::book(), &book, PixelLen(8))
        .unwrap()
        .expect("the plugin serves a book");
    assert_eq!(served.size().width.0, 8);
    let video = Subject {
        kind: FormatKind::Video,
        mime: None,
    };
    let clip = scratch.book("clip.mp4", "not really a video");
    assert!(
        runner
            .poster(&scratch.plugins(), &video, &clip, PixelLen(8))
            .unwrap()
            .is_none(),
        "no plugin serves a video: the card keeps its plate"
    );
}
