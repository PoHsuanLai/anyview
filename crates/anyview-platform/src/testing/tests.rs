use super::*;
use crate::apps::{AppEntry, Association, DesktopId};
use crate::instance::{Claim, Instance, Request};
use crate::media::{MediaControl, MediaSession, MediaState, PlaybackStatus};
use crate::stacking::{Stacking, StackingOutcome, WindowStacking};
use crate::thumbnail::{ThumbPixels, ThumbSize, ThumbnailCache};
use anyview_core::{ByteLen, FilePath, FileStamp, ModTime, PixelLen, PixelSize};

fn file(path: &str) -> FilePath {
    FilePath::new(path).unwrap()
}

fn stamp(modified: i64) -> FileStamp {
    FileStamp {
        len: ByteLen(1),
        modified: ModTime(modified),
    }
}

#[tokio::test]
async fn a_first_launch_owns_the_name_and_receives_forwarded_requests() {
    let fake = FakeInstance::new(FakeRole::FirstLaunch);
    let mine = Request::Open(vec![file("/a.png")]);
    let Claim::Primary(mut primary) = fake.claim(&mine).await.unwrap() else {
        panic!("a first launch is the primary");
    };
    let theirs = Request::Peek(file("/b.png"));
    fake.forward(theirs.clone());
    assert_eq!(primary.next().await, Some(theirs));
    assert_eq!(fake.claimed(), vec![mine]);
}

#[tokio::test]
async fn a_second_launch_forwards_and_the_request_is_recorded() {
    let fake = FakeInstance::new(FakeRole::SecondLaunch);
    let request = Request::Play(file("/a.flac"));
    assert!(matches!(
        fake.claim(&request).await.unwrap(),
        Claim::Forwarded
    ));
    assert_eq!(fake.claimed(), vec![request]);
}

#[tokio::test]
async fn the_media_fake_records_states_and_delivers_pressed_controls() {
    let mut fake = FakeMediaSession::new();
    let mut state = MediaState::stopped();
    state.status = PlaybackStatus::Playing;
    fake.publish(&state).await.unwrap();
    fake.press(MediaControl::Next);
    assert_eq!(fake.published(), vec![state]);
    assert_eq!(fake.next_control().await, Some(MediaControl::Next));
}

#[test]
fn the_thumbnail_fake_keeps_one_thumbnail_per_file_and_size() {
    let fake = FakeThumbnails::default();
    let size = PixelSize {
        width: PixelLen(1),
        height: PixelLen(1),
    };
    let first = ThumbPixels::new(size, vec![1, 2, 3, 4]).unwrap();
    let second = ThumbPixels::new(size, vec![5, 6, 7, 8]).unwrap();
    let path = file("/a.png");
    fake.store(&path, &stamp(1), ThumbSize::Normal, &first)
        .unwrap();
    fake.store(&path, &stamp(2), ThumbSize::Normal, &second)
        .unwrap();
    assert_eq!(fake.len(), 1);
    let found = |at| fake.lookup(&path, &stamp(at), ThumbSize::Normal).unwrap();
    assert_eq!(found(1), None);
    assert_eq!(found(2), Some(second));
}

#[test]
fn the_stacking_fake_refuses_keep_above_only_when_unsupported() {
    const CASES: &[(&str, StackingSupport, Stacking, StackingOutcome)] = &[
        (
            "supported",
            StackingSupport::Supported,
            Stacking::KeepAbove,
            StackingOutcome::Applied,
        ),
        (
            "unsupported",
            StackingSupport::Unsupported,
            Stacking::KeepAbove,
            StackingOutcome::Unsupported,
        ),
        (
            "normal is always applied",
            StackingSupport::Unsupported,
            Stacking::Normal,
            StackingOutcome::Applied,
        ),
    ];
    for (name, support, asked, want) in CASES {
        let fake = FakeStacking::with(*support);
        assert_eq!(fake.request(*asked), *want, "{name}");
        assert_eq!(fake.requested(), vec![*asked], "{name}");
    }
}

#[test]
fn the_apps_fake_offers_its_entries_and_records_what_opens() {
    let id = DesktopId::new("a.desktop").unwrap();
    let entry = AppEntry {
        id: id.clone(),
        name: "A".to_owned(),
        association: Association::Default,
    };
    let fake = FakeApps::offering(vec![entry.clone()]);
    let mime = anyview_core::Mime::parse("image/png").unwrap();
    assert_eq!(crate::AppsForType::apps_for(&fake, &mime), vec![entry]);
    let target = file("/a.png");
    crate::AppsForType::open_with(&fake, &id, &target).unwrap();
    assert_eq!(fake.opened(), vec![(id, target)]);
}
