//! What the host lends the views to read from: where a file was left, and the shared thumbnail.

use super::support::{PNG, desktop, probed};
use crate::host::{CachedPictures, HostedResume, Hosting, Task};
use anyview_core::{LineIndex, PixelLen, PixelSize, Resume};
use anyview_platform::testing::FakeThumbnails;
use anyview_platform::{ThumbPixels, ThumbSize, ThumbnailCache};
use anyview_ui::{FirstFrameSource, ResumeSource};
use std::sync::Arc;

#[tokio::test]
async fn the_views_recall_what_the_store_kept_for_this_version_of_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = probed(dir.path(), "a.png", PNG);
    let (desktop, _) = desktop(dir.path(), vec![]);
    let hosting: Arc<dyn Hosting> = Arc::new(desktop);
    let recall = HostedResume(Arc::clone(&hosting));
    let kept = Resume::Text { line: LineIndex(9) };

    assert_eq!(
        recall.recall(file.source.path(), file.source.stamp()),
        Resume::Nothing
    );
    hosting
        .carry_out(Task::Remember {
            source: file.source.clone(),
            resume: kept.clone(),
        })
        .await
        .unwrap();
    let flushing = Arc::clone(&hosting);
    tokio::task::spawn_blocking(move || flushing.flush())
        .await
        .unwrap();

    let reading = Arc::new(recall);
    let source = file.source.clone();
    let recalled =
        tokio::task::spawn_blocking(move || reading.recall(source.path(), source.stamp()))
            .await
            .unwrap();
    assert_eq!(recalled, kept);
}

#[test]
fn the_first_frame_is_the_shared_thumbnail_of_this_version_of_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = probed(dir.path(), "a.png", PNG);
    let cache = FakeThumbnails::default();
    let pictures = CachedPictures(cache.clone());
    assert!(pictures.picture(&file.source).is_none(), "none made yet");

    let size = PixelSize {
        width: PixelLen(2),
        height: PixelLen(1),
    };
    let made = ThumbPixels::new(size, vec![9; 8]).unwrap();
    cache
        .store(
            file.source.path(),
            &file.source.stamp(),
            ThumbSize::Normal,
            &made,
        )
        .unwrap();

    let found = pictures.picture(&file.source).unwrap();
    assert_eq!(found.size(), size);
    assert_eq!(found.bytes(), made.rgba());
}
