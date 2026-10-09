//! `Instance` over the per-user socket (latchkey), built on every platform: the first claim
//! listens, later ones hand their request over and leave, and a socket a killed viewer left
//! behind is taken over. Every socket and lock lives in a scratch directory under `TMPDIR`
//! (`LatchkeyInstance::under`): nothing here reads `XDG_RUNTIME_DIR`, so the person's runtime
//! directory is never touched.

use anyview_core::{FilePath, LineIndex, NonEmpty, ResultsId, Resume, Sequence, SequenceOrigin};
use anyview_platform::portable::LatchkeyInstance;
use anyview_platform::{Claim, Handoff, Instance, Primary, Request};
#[cfg(unix)]
use std::path::{Path, PathBuf};
use std::time::Duration;

fn path(text: &str) -> FilePath {
    FilePath::new(text).unwrap()
}

/// A scratch directory short enough for a socket path, under `TMPDIR`.
fn scratch() -> tempfile::TempDir {
    tempfile::Builder::new().prefix("lk").tempdir().unwrap()
}

async fn primary(instance: &LatchkeyInstance) -> Primary {
    match instance.claim(&Request::Open(Vec::new())).await.unwrap() {
        Claim::Primary(primary) => primary,
        Claim::Forwarded => panic!("nobody was running: this launch must be the viewer"),
    }
}

async fn next(primary: &mut Primary) -> Request {
    tokio::time::timeout(Duration::from_secs(10), primary.next())
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn the_first_launch_listens_and_a_second_hands_its_files_over() {
    let dir = scratch();
    let mut first = primary(&LatchkeyInstance::under(dir.path().to_owned())).await;

    let second = LatchkeyInstance::under(dir.path().to_owned());
    let sent = [
        Request::Open(vec![path("/pics/a.png"), path("/pics/b b.png")]),
        Request::Peek(path("/docs/c.pdf")),
        Request::Play(path("/music/d.flac")),
        Request::Open(Vec::new()),
    ];
    for request in &sent {
        let claim = second.claim(request).await.unwrap();
        assert!(matches!(claim, Claim::Forwarded), "{request:?}");
    }
    for request in sent {
        assert_eq!(next(&mut first).await, request);
    }
}

#[tokio::test]
async fn a_handoff_keeps_its_results_and_its_place() {
    let dir = scratch();
    let mut first = primary(&LatchkeyInstance::under(dir.path().to_owned())).await;

    let entries = NonEmpty::from_vec(vec![path("/a/1.pdf"), path("/a/2.pdf")]).unwrap();
    let sequence = Sequence::starting_at(
        entries,
        &path("/a/2.pdf"),
        SequenceOrigin::Results(ResultsId(9)),
    )
    .unwrap();
    let handed = Handoff {
        file: path("/a/2.pdf"),
        resume: Resume::Text {
            line: LineIndex(41),
        },
        sequence: Some(sequence),
    };
    let claim = LatchkeyInstance::under(dir.path().to_owned())
        .claim(&Request::Handoff(handed.clone()))
        .await
        .unwrap();
    assert!(matches!(claim, Claim::Forwarded));
    assert_eq!(next(&mut first).await, Request::Handoff(handed));
}

#[tokio::test]
async fn two_scratch_directories_are_two_viewers() {
    let (one, two) = (scratch(), scratch());
    let _first = primary(&LatchkeyInstance::under(one.path().to_owned())).await;
    let _second = primary(&LatchkeyInstance::under(two.path().to_owned())).await;
}

/// The socket latchkey binds under `runtime`.
#[cfg(unix)]
fn socket_under(runtime: &Path) -> PathBuf {
    runtime.join("anyview").join("agent.sock")
}

#[cfg(unix)]
#[tokio::test]
async fn a_socket_left_by_a_killed_viewer_is_taken_over() {
    let dir = scratch();
    let socket = socket_under(dir.path());
    std::fs::create_dir_all(socket.parent().unwrap()).unwrap();
    // A viewer killed with SIGKILL leaves its socket standing and its lock released. Dropping a
    // std listener closes it without unlinking the file, which is the same state.
    drop(std::os::unix::net::UnixListener::bind(&socket).unwrap());
    assert!(socket.exists(), "the stale socket is on disk");

    let mut first = primary(&LatchkeyInstance::under(dir.path().to_owned())).await;
    let claim = LatchkeyInstance::under(dir.path().to_owned())
        .claim(&Request::Peek(path("/docs/c.pdf")))
        .await
        .unwrap();
    assert!(matches!(claim, Claim::Forwarded));
    assert_eq!(next(&mut first).await, Request::Peek(path("/docs/c.pdf")));
}

#[cfg(unix)]
#[tokio::test]
async fn closing_the_viewer_removes_its_socket_and_the_next_launch_is_the_viewer() {
    let dir = scratch();
    let socket = socket_under(dir.path());
    let first = primary(&LatchkeyInstance::under(dir.path().to_owned())).await;
    assert!(socket.exists());
    drop(first);
    assert!(!socket.exists(), "the socket goes with the viewer");

    let _again = primary(&LatchkeyInstance::under(dir.path().to_owned())).await;
}

#[cfg(unix)]
#[tokio::test]
async fn a_line_that_is_not_a_request_is_refused_and_the_viewer_carries_on() {
    use std::io::{BufRead, BufReader, Write};
    let dir = scratch();
    let mut first = primary(&LatchkeyInstance::under(dir.path().to_owned())).await;
    let socket = socket_under(dir.path());

    let refused = tokio::task::spawn_blocking(move || {
        let mut stream = std::os::unix::net::UnixStream::connect(socket).unwrap();
        // A relative path is the one the D-Bus service refuses too.
        stream
            .write_all(b"{\"op\":\"peek\",\"file\":\"relative/file.png\"}\n")
            .unwrap();
        let mut reply = String::new();
        BufReader::new(stream).read_line(&mut reply).unwrap();
        reply
    })
    .await
    .unwrap();
    assert!(refused.contains("\"ok\":false"), "{refused}");

    let claim = LatchkeyInstance::under(dir.path().to_owned())
        .claim(&Request::Play(path("/music/d.flac")))
        .await
        .unwrap();
    assert!(matches!(claim, Claim::Forwarded));
    assert_eq!(next(&mut first).await, Request::Play(path("/music/d.flac")));
}
