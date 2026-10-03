use super::*;
use anyview_core::FilePath;
use anyview_platform::Request;
use std::ffi::OsString;

fn path(text: &str) -> FilePath {
    FilePath::new(text).unwrap()
}

fn args(list: &[&str]) -> Vec<OsString> {
    list.iter().map(OsString::from).collect()
}

#[test]
fn arguments_become_the_request_a_launch_makes() {
    let cwd = path("/home/a");
    let cases: Vec<(&str, Vec<&str>, Result<Invocation, CliError>)> = vec![
        (
            "no arguments is the bus activation",
            vec![],
            Ok(Invocation::Launch(Request::Open(vec![]))),
        ),
        (
            "an absolute file stays where it is",
            vec!["/tmp/x.png"],
            Ok(Invocation::Launch(Request::Open(vec![path("/tmp/x.png")]))),
        ),
        (
            "a relative file is named against the working directory",
            vec!["pics/../x.png"],
            Ok(Invocation::Launch(Request::Open(vec![path(
                "/home/a/x.png",
            )]))),
        ),
        (
            "several files keep their order",
            vec!["b.txt", "a.txt"],
            Ok(Invocation::Launch(Request::Open(vec![
                path("/home/a/b.txt"),
                path("/home/a/a.txt"),
            ]))),
        ),
        (
            "peek takes the file after it",
            vec!["--peek", "x.pdf"],
            Ok(Invocation::Launch(Request::Peek(path("/home/a/x.pdf")))),
        ),
        (
            "the flag may follow its file",
            vec!["x.mp3", "--play"],
            Ok(Invocation::Launch(Request::Play(path("/home/a/x.mp3")))),
        ),
        (
            "a dash dash makes a file of a name that looks like a flag",
            vec!["--", "--peek"],
            Ok(Invocation::Launch(Request::Open(vec![path(
                "/home/a/--peek",
            )]))),
        ),
        ("help", vec!["x", "--help"], Ok(Invocation::Help)),
        (
            "peek without a file",
            vec!["--peek"],
            Err(CliError::MissingFile("--peek")),
        ),
        (
            "play with two files",
            vec!["--play", "a", "b"],
            Err(CliError::ManyFiles("--play")),
        ),
        (
            "peek and play together",
            vec!["--peek", "--play", "a"],
            Err(CliError::Conflict),
        ),
        (
            "an option that does not exist",
            vec!["--fullscreen", "a"],
            Err(CliError::UnknownFlag("--fullscreen".to_owned())),
        ),
        ("an empty name", vec![""], Err(CliError::EmptyName)),
        (
            "a file URI is the path it names, decoded",
            vec!["file:///tmp/a%20b%C3%A9.png", "file://localhost/x.txt"],
            Ok(Invocation::Launch(Request::Open(vec![
                path("/tmp/a b\u{e9}.png"),
                path("/x.txt"),
            ]))),
        ),
        (
            "a URI of another machine is refused",
            vec!["file://host/x.txt"],
            Err(CliError::NotLocal("file://host/x.txt".to_owned())),
        ),
        (
            "a URI of another scheme is refused",
            vec!["https://example.org/x.png"],
            Err(CliError::NotLocal("https://example.org/x.png".to_owned())),
        ),
    ];
    for (name, list, want) in cases {
        assert_eq!(parse(&args(&list), &cwd), want, "{name}");
    }
}

#[cfg(unix)]
#[test]
fn a_name_that_is_not_utf8_is_refused_whole() {
    use std::os::unix::ffi::OsStringExt;
    let bad = OsString::from_vec(vec![b'a', 0xff]);
    assert_eq!(
        parse(std::slice::from_ref(&bad), &path("/")),
        Err(CliError::NotUtf8(bad))
    );
}
