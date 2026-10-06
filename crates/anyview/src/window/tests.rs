use super::*;
use anyview_core::{
    FilePath, NonEmpty, PageIndex, Permille, ResultsId, Resume, Sequence, SequenceOrigin, Zoom,
};
use anyview_platform::Handoff;

fn touch(dir: &std::path::Path, name: &str) -> FilePath {
    std::fs::write(dir.join(name), "x").unwrap();
    FilePath::new(dir.join(name)).unwrap()
}

#[test]
fn the_sequence_is_the_visible_files_of_the_folder_by_name_pointing_at_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let b = touch(dir.path(), "b.png");
    let a = touch(dir.path(), "a.png");
    let c = touch(dir.path(), "c.txt");
    touch(dir.path(), ".hidden");
    std::fs::create_dir(dir.path().join("folder")).unwrap();

    let sequence = sequence_around(&b).unwrap();
    let entries: Vec<&FilePath> = sequence.entries().iter().collect();
    assert_eq!(entries, vec![&a, &b, &c], "no hidden file and no folder");
    assert_eq!(sequence.current(), &b);
}

#[test]
fn a_file_that_is_not_there_or_a_folder_that_cannot_be_read_has_no_sequence() {
    let dir = tempfile::tempdir().unwrap();
    let missing = FilePath::new(dir.path().join("gone.png")).unwrap();
    assert_eq!(sequence_around(&missing), None, "not among the files");
    let nowhere = FilePath::new(dir.path().join("no-such-folder/a.png")).unwrap();
    assert_eq!(sequence_around(&nowhere), None, "no folder");
    assert_eq!(
        Opening::around(missing.clone()),
        Opening {
            file: missing,
            sequence: None,
            resume: Resume::Nothing,
        },
        "the window still opens, to say the file is not there"
    );
}

mod remaking {
    use super::*;
    use crate::host::Appearances;
    use crate::host::{Clock, Desktop, Media, Services, Store, Trash, TrashError};
    use crate::media::{MediaHub, PlayerHost};
    use crate::runtime::PoolSize;
    use crate::seam::{NoticeWaker, Workforce};
    use crate::window::root::{remade, spec_for, stacking_for};
    use anyview_media::AudioDriver;
    use anyview_platform::testing::{
        FakeApps, FakeLinks, FakeMediaSession, FakePicker, FakePrinter, FakeReveal, FakeShare,
        FakeStacking, StackingSupport,
    };
    use anyview_platform::{PickOutcome, PrintOutcome, Stacking, StackingOutcome};
    use anyview_store::Viewed;
    use anyview_ui::Look;
    use anyview_ui::Presentation;
    use ds_blitz::{Decorations, WindowSize, WindowSpec};
    use std::sync::Arc;

    struct NoTrash;

    impl Trash for NoTrash {
        fn trash(&self, _file: &FilePath) -> Result<(), TrashError> {
            Ok(())
        }
    }

    /// A seed over fakes, for the window of `file` in `presentation`.
    fn seed(
        runtime: &tokio::runtime::Runtime,
        dir: &std::path::Path,
        file: &FilePath,
        presentation: Presentation,
        stacking: FakeStacking,
    ) -> Seed {
        let workforce = Workforce::start(
            PoolSize::exactly(std::num::NonZeroUsize::MIN),
            NoticeWaker::default(),
        )
        .unwrap();
        let now: Clock = Arc::new(|| Viewed(1));
        let hub = MediaHub::start(
            runtime.handle(),
            || std::future::ready(FakeMediaSession::new()),
            None,
            AudioDriver::Null,
            Arc::new(crate::media::MediaPlugins::default()),
        );
        let desktop = Desktop::new(
            runtime.handle().clone(),
            FakeApps::default(),
            FakeReveal::default(),
            FakeShare::default(),
            FakePrinter::answering(PrintOutcome::Printed),
            NoTrash,
            FakePicker::answering(PickOutcome::Cancelled),
            FakeLinks::default(),
            Services {
                versions: anyview_store::Versions::under_state(&dir.join("state")),
                store: Store::new(&dir.join("store"), now),
                media: Media {
                    hub: hub.clone(),
                    exports: workforce.exports(),
                    scratch: dir.join("cache"),
                },
            },
        );
        let factory = Factory::new(
            workforce.workers(),
            Arc::new(desktop),
            Arc::new(crate::host::CachedPictures(
                anyview_platform::testing::FakeThumbnails::default(),
            )),
            None,
            Appearances::fixed(Look::default()),
            Arc::new(PlayerHost::new(hub)),
            Arc::new(stacking),
        );
        Seed {
            factory,
            opening: Opening::around(file.clone()),
            presentation,
        }
    }

    #[test]
    fn the_small_window_is_small_and_borderless_and_the_others_are_ordinary() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let file = touch(dir.path(), "clip.mkv");
        let stacking = FakeStacking::with(StackingSupport::Supported);
        let of = |presentation| {
            spec_for(&seed(
                &runtime,
                dir.path(),
                &file,
                presentation,
                stacking.clone(),
            ))
        };
        assert_eq!(
            of(Presentation::Mini),
            WindowSpec::new("clip.mkv", WindowSize::new(480, 270))
                .with_decorations(Decorations::Client)
        );
        for ordinary in [
            Presentation::Window,
            Presentation::Peek,
            Presentation::Background,
        ] {
            assert_eq!(
                of(ordinary),
                WindowSpec::new("clip.mkv", WindowSize::new(1000, 700)),
                "{ordinary:?}"
            );
        }
    }

    #[test]
    fn a_window_made_again_is_the_same_file_in_the_presentation_asked_for() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let file = touch(dir.path(), "clip.mkv");
        let beside = touch(dir.path(), "other.png");
        let old = seed(
            &runtime,
            dir.path(),
            &beside,
            Presentation::Window,
            FakeStacking::with(StackingSupport::Supported),
        );
        let mini = remade(&old, &file, Presentation::Mini);
        assert_eq!(mini.presentation, Presentation::Mini);
        assert_eq!(
            mini.opening.file, file,
            "the file shown, not the one the old window was opened on"
        );
        assert_eq!(
            mini.opening
                .sequence
                .as_ref()
                .map(|sequence| sequence.entries().count().get()),
            Some(2),
            "with the folder's files for the arrow keys"
        );
    }

    #[test]
    fn only_the_small_window_asks_to_stay_above_and_a_refusal_is_reported() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let file = touch(dir.path(), "clip.mkv");
        for (support, want) in [
            (StackingSupport::Supported, StackingOutcome::Applied),
            (StackingSupport::Unsupported, StackingOutcome::Unsupported),
        ] {
            let stacking = FakeStacking::with(support);
            let made = seed(
                &runtime,
                dir.path(),
                &file,
                Presentation::Mini,
                stacking.clone(),
            );
            assert_eq!(
                stacking_for(Presentation::Mini, made.factory.stacking.as_ref()),
                Some(want),
                "{support:?}"
            );
            assert_eq!(stacking.requested(), vec![Stacking::KeepAbove]);
            for other in [
                Presentation::Window,
                Presentation::Peek,
                Presentation::Background,
            ] {
                assert_eq!(
                    stacking_for(other, made.factory.stacking.as_ref()),
                    None,
                    "{other:?}"
                );
            }
            assert_eq!(stacking.requested().len(), 1, "the others asked nothing");
        }
    }
}

#[test]
fn a_handed_file_opens_with_the_results_and_the_place_it_brought() {
    let dir = tempfile::tempdir().unwrap();
    let a = touch(dir.path(), "a.pdf");
    let b = touch(dir.path(), "b.pdf");
    touch(dir.path(), "c.pdf");
    let results = Sequence::starting_at(
        NonEmpty::from_vec(vec![b.clone(), a.clone()]).unwrap(),
        &a,
        SequenceOrigin::Results(ResultsId(3)),
    )
    .unwrap();
    let place = Resume::Pdf {
        page: PageIndex(2),
        offset: Permille(0),
        zoom: Zoom::Fit,
    };
    let opening = Opening::handed(Handoff {
        file: a.clone(),
        resume: place.clone(),
        sequence: Some(results.clone()),
    });
    assert_eq!(opening.file, a);
    assert_eq!(opening.resume, place);
    assert_eq!(
        opening.sequence,
        Some(results),
        "the results are the sequence, not the folder (which holds c.pdf too)"
    );
}
