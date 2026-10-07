use super::support::{desktop, desktop_installing};
use crate::host::{HelperHost, Hosting, ImageHost, Outcome, PluginRegistry, Task};
use crate::media::MediaPlugins;
use anyview_core::{FormatKind, Helper};
use anyview_plugin::{
    Candidate, Manifest, Origin, PluginError, Plugins, Readiness, Route, Subject,
};
use anyview_plugin_protocol::Capability;
use anyview_ui::{HelperEnd, HelperSource, HelperWords};
use ds::prelude::Word;
use ds_helpers::{Catalog, Environment, FakeInstaller, Installer, Missing, Outcome as Installed};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

/// The file the viewer ships, as `dist/install.sh` installs it.
const SHIPPED: &str = include_str!("../../../../../dist/helpers/anyview.toml");

/// A machine to look for tools on: a folder that is its `PATH`, and an `os-release`.
struct Machine {
    dir: tempfile::TempDir,
}

impl Machine {
    fn new(os_release: &str) -> Machine {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("bin")).unwrap();
        std::fs::write(dir.path().join("os-release"), os_release).unwrap();
        Machine { dir }
    }

    fn fedora() -> Machine {
        Machine::new("ID=fedora\n")
    }

    fn bin(&self) -> PathBuf {
        self.dir.path().join("bin")
    }

    fn environment(&self) -> Environment {
        Environment {
            path: self.bin().into(),
            os_release: self.dir.path().join("os-release"),
        }
    }

    /// A tool of this name appears on the `PATH`, as a package install leaves it.
    fn install(&self, tool: &str) {
        use std::os::unix::fs::PermissionsExt;
        let file = self.bin().join(tool);
        std::fs::write(&file, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

/// The plugins read again, counted: what `reread` does for a registry that is told a tool came.
fn counting_registry() -> (PluginRegistry, Arc<AtomicUsize>) {
    let reads = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&reads);
    let registry = PluginRegistry::rereading(Plugins::none(), move || {
        counted.fetch_add(1, Ordering::SeqCst);
        Plugins::none()
    });
    (registry, reads)
}

fn host_of(
    machine: &Machine,
    catalog: &str,
    installer: &FakeInstaller,
    registry: PluginRegistry,
) -> Arc<HelperHost> {
    Arc::new(HelperHost::new(
        Catalog::parse(catalog).unwrap(),
        machine.environment(),
        Installer::Fake(installer.clone()),
        registry,
    ))
}

/// A host over the shipped file, installing what `answer` says and leaving `leaving` on the `PATH`.
fn host(
    machine: &Machine,
    answer: Installed,
    leaving: &[&str],
) -> (Arc<HelperHost>, FakeInstaller) {
    let fake = FakeInstaller::new(answer).leaving(machine.bin(), leaving);
    let host = host_of(machine, SHIPPED, &fake, PluginRegistry::default());
    (host, fake)
}

/// What a scripted installer is told to answer with: the request's own `Missing` replaces it.
fn nothing() -> Missing {
    Missing {
        capability: ds_helpers::Capability::new("heic-decode").unwrap(),
        package: None,
        program: None,
    }
}

#[test]
fn the_shipped_file_declares_exactly_the_tools_the_viewer_names() {
    let catalog = Catalog::parse(SHIPPED).expect("the shipped file parses with quire's catalog");
    let declared: Vec<&str> = catalog.capabilities().map(|c| c.as_str()).collect();
    let mut named: Vec<&str> = Helper::ALL.iter().map(|helper| helper.slug()).collect();
    named.sort_unstable();
    assert_eq!(declared, named, "one table for each helper, no other");
    for capability in catalog.capabilities() {
        let entry = catalog.get(capability).unwrap();
        assert!(
            !entry.probe.is_empty(),
            "{capability} names what to look for"
        );
        for family in [ds_helpers::Family::Dnf, ds_helpers::Family::Apt] {
            assert!(
                !entry.candidates(family).is_empty(),
                "{capability} names a package for {family:?}"
            );
        }
    }
}

#[test]
fn the_shipped_file_loads_from_a_data_dir_where_the_installer_puts_it() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("quire/helpers");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("anyview.toml"), SHIPPED).unwrap();
    let catalog = Catalog::load("anyview", &[dir.path().to_owned()]).unwrap();
    assert_eq!(catalog.capabilities().count(), Helper::ALL.len());
}

#[test]
fn the_sheet_is_worded_from_the_file_for_this_distribution() {
    // distribution, tool, helper, words
    let cases: Vec<(&str, Helper, HelperWords)> = vec![
        (
            "ID=fedora\n",
            Helper::HeicDecode,
            HelperWords {
                app: "Anyview".to_owned(),
                tool: "libheif tools".to_owned(),
                purpose: "open HEIC photos".to_owned(),
            },
        ),
        (
            "ID=ubuntu\nID_LIKE=debian\n",
            Helper::HeicDecode,
            HelperWords {
                app: "Anyview".to_owned(),
                tool: "libheif tools".to_owned(),
                purpose: "open HEIC photos".to_owned(),
            },
        ),
        (
            "ID=fedora\n",
            Helper::VideoPlayback,
            HelperWords {
                app: "Anyview".to_owned(),
                tool: "mpv".to_owned(),
                purpose: "play videos".to_owned(),
            },
        ),
    ];
    for (os_release, helper, want) in cases {
        let machine = Machine::new(os_release);
        let (host, _) = host(&machine, Installed::Installed, &[]);
        assert_eq!(host.words(helper), Some(want), "{os_release:?} {helper:?}");
    }
}

#[test]
fn a_helper_the_file_does_not_declare_is_not_offered() {
    let machine = Machine::fedora();
    let only_heic = "[heic-decode]\ntool=\"libheif tools\"\npurpose=\"open HEIC photos\"\n\
                     probe=[\"heif-dec\"]\n";
    let fake = FakeInstaller::new(Installed::Installed);
    let host = host_of(&machine, only_heic, &fake, PluginRegistry::default());
    let fact = anyview_core::Fact {
        label: anyview_core::FactLabel::Needs,
        value: anyview_core::FactValue::text("x"),
    };
    assert_eq!(
        host.need(fact.clone(), Helper::HeicDecode).helper,
        Some(Helper::HeicDecode)
    );
    assert_eq!(host.need(fact, Helper::VideoPlayback).helper, None);
    assert_eq!(host.words(Helper::VideoPlayback), None);
}

#[test]
fn a_system_that_cannot_install_offers_no_install_beside_the_row() {
    let machine = Machine::fedora();
    let fake = FakeInstaller::new(Installed::Installed);
    let catalog = Catalog::parse(SHIPPED).unwrap();
    let host = HelperHost::new(
        catalog,
        machine.environment(),
        Installer::Fake(fake),
        PluginRegistry::default(),
    )
    .installing_where(false);
    let fact = anyview_core::Fact {
        label: anyview_core::FactLabel::Needs,
        value: anyview_core::FactValue::text("x"),
    };
    assert!(!host.offers(Helper::HeicDecode));
    let need = host.need(fact, Helper::HeicDecode);
    assert_eq!(need.helper, None, "the row stays, with no Install...");
}

#[tokio::test]
async fn what_the_installer_answers_is_what_the_window_is_told() {
    // name, installer's answer, the end the window hears
    let cases: Vec<(&str, Installed, HelperEnd)> = vec![
        ("installed", Installed::Installed, HelperEnd::Installed),
        ("declined", Installed::Declined, HelperEnd::Declined),
        (
            "no package",
            Installed::NotFound(nothing()),
            HelperEnd::NotFound("libheif-tools".to_owned()),
        ),
        (
            "no way to install",
            Installed::Unsupported(nothing()),
            HelperEnd::Unsupported("heif-dec".to_owned()),
        ),
        (
            "failed",
            Installed::Failed("No network.".to_owned()),
            HelperEnd::Failed("No network.".to_owned()),
        ),
    ];
    for (name, answer, want) in cases {
        let machine = Machine::fedora();
        let (host, fake) = host(&machine, answer, &["heif-dec"]);
        assert_eq!(host.provide(Helper::HeicDecode).await, want, "{name}");
        let asked = fake.asked();
        assert_eq!(asked.len(), 1, "{name}: the installer was asked once");
        let packages: Vec<&str> = asked[0].candidates.iter().map(|p| p.as_str()).collect();
        assert_eq!(
            packages,
            ["libheif-tools"],
            "{name}: for this distribution's package"
        );
    }
}

#[tokio::test]
async fn a_tool_that_is_already_there_is_installed_without_asking_the_installer() {
    let machine = Machine::fedora();
    machine.install("heif-convert");
    let (host, fake) = host(&machine, Installed::Declined, &[]);
    assert_eq!(host.provide(Helper::HeicDecode).await, HelperEnd::Installed);
    assert!(fake.asked().is_empty());
}

#[tokio::test]
async fn an_install_that_worked_makes_the_registry_read_the_plugins_again() {
    let machine = Machine::fedora();
    let (registry, reads) = counting_registry();
    let fake = FakeInstaller::new(Installed::Installed).leaving(machine.bin(), &["mpv"]);
    let host = host_of(&machine, SHIPPED, &fake, registry);
    assert_eq!(
        host.provide(Helper::VideoPlayback).await,
        HelperEnd::Installed
    );
    assert!(
        reads.load(Ordering::SeqCst) >= 1,
        "the plugins are read again"
    );

    let (registry, reads) = counting_registry();
    let fake = FakeInstaller::new(Installed::Declined);
    let host = host_of(&Machine::fedora(), SHIPPED, &fake, registry);
    assert_eq!(
        host.provide(Helper::VideoPlayback).await,
        HelperEnd::Declined
    );
    assert_eq!(reads.load(Ordering::SeqCst), 0, "a no changes nothing");
}

#[tokio::test]
async fn the_desktop_carries_an_install_out_and_says_how_it_ended() {
    let dir = tempfile::tempdir().unwrap();
    let machine = Machine::fedora();
    let (helpers, _) = host(&machine, Installed::Installed, &["heif-dec"]);
    let (installing, _fakes) = desktop_installing(dir.path(), helpers);
    let outcome = installing
        .carry_out(Task::Provide(Helper::HeicDecode))
        .await
        .unwrap();
    assert_eq!(
        outcome,
        Outcome::Helped(Helper::HeicDecode, HelperEnd::Installed)
    );
    assert!(
        machine.bin().join("heif-dec").exists(),
        "the fake left the tool"
    );

    let (without, _fakes) = desktop(dir.path(), vec![]);
    let outcome = without
        .carry_out(Task::Provide(Helper::HeicDecode))
        .await
        .unwrap();
    assert_eq!(
        outcome,
        Outcome::Helped(
            Helper::HeicDecode,
            HelperEnd::Unsupported("heic-decode".to_owned())
        ),
        "a program with no list of its tools cannot install one"
    );
}

#[test]
fn the_package_managers_words_go_to_the_log_and_the_sheet_not_a_toast() {
    let failed = Outcome::Helped(
        Helper::HeicDecode,
        HelperEnd::Failed("No network.".to_owned()),
    );
    assert_eq!(
        super::super::feedback::line_of(&failed),
        Some("cannot install heic-decode: No network.".to_owned())
    );
    for quiet in [HelperEnd::Installed, HelperEnd::Declined] {
        let outcome = Outcome::Helped(Helper::HeicDecode, quiet);
        assert_eq!(super::super::feedback::line_of(&outcome), None);
    }
}

/// Wait for the next tool a window is told of.
fn next(heard: &Receiver<Helper>) -> Helper {
    heard
        .recv_timeout(Duration::from_secs(10))
        .expect("a window is told")
}

// The test waits on a plain channel, so the task that follows the tools needs a thread of its own.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_tool_installed_some_other_way_is_told_to_every_window_after_the_plugins_are_read() {
    let machine = Machine::fedora();
    machine.install("mpv");
    let (registry, reads) = counting_registry();
    let fake = FakeInstaller::new(Installed::Declined);
    let host = host_of(&machine, SHIPPED, &fake, registry);
    // What is there when the program starts is no news: the first look records it.
    let following = tokio::spawn(host.following());
    let (tell_a, a) = channel();
    let (tell_b, b) = channel();
    let (tell_late, late) = channel();
    let first = host.listen(move |helper| {
        // A window that closed has no receiver, and nobody is left to tell.
        let _gone = tell_a.send(helper);
    });
    let _second = host.listen(move |helper| {
        // A window that closed has no receiver, and nobody is left to tell.
        let _gone = tell_b.send(helper);
    });
    let gone = host.listen(move |helper| {
        // A window that closed has no receiver, and nobody is left to tell.
        let _gone = tell_late.send(helper);
    });
    drop(gone);

    machine.install("heif-dec");
    host.look_again();
    assert_eq!(
        next(&a),
        Helper::HeicDecode,
        "mpv was there already: not news"
    );
    assert_eq!(next(&b), Helper::HeicDecode);
    assert!(
        reads.load(Ordering::SeqCst) >= 1,
        "read before they were told"
    );
    assert!(
        late.try_recv().is_err(),
        "a window that closed hears nothing"
    );

    // A window that stops listening is not told the next one.
    drop(first);
    machine.install("dcraw_emu");
    host.look_again();
    assert_eq!(next(&b), Helper::RawDecode);
    assert!(a.try_recv().is_err());
    following.abort();
}

#[test]
fn a_manifest_whose_mpv_is_absent_offers_to_install_it_and_one_whose_plugin_is_absent_does_not() {
    let machine = Machine::fedora();
    let (helpers, _) = host(&machine, Installed::Installed, &[]);
    let unready = |reason: PluginError| {
        let text = "id = \"mpv\"\nname = \"mpv\"\nprotocol = 1\n[[provides]]\ncapability = \"play\"\n\
                    kinds = [\"video\"]\nmpv = \"/usr/bin/mpv\"\ncplugin = \"/opt/av/cplugin.so\"\n";
        Plugins::resolve(vec![Candidate {
            manifest: Manifest::parse(text).unwrap(),
            origin: Origin::System,
            readiness: Readiness::Unready(reason),
        }])
    };
    let video = Subject {
        kind: FormatKind::Video,
        mime: None,
    };
    let missing_of = |plugins: &Plugins| {
        let Route::Missing(missing) = plugins.route(Capability::Play, &video) else {
            panic!("a recording with no usable player routes to the package that would play it");
        };
        missing
    };
    let missing = |path: &str| PluginError::FileMissing { path: path.into() };
    // name, the plugins, whether a tool is offered
    let cases: Vec<(&str, Plugins, bool)> = vec![
        ("mpv is absent", unready(missing("/usr/bin/mpv")), true),
        (
            "the C plugin is absent",
            unready(missing("/opt/av/cplugin.so")),
            false,
        ),
        ("no plugin installed", Plugins::none(), false),
    ];
    for (name, plugins, offered) in cases {
        let missing = missing_of(&plugins);
        let media = MediaPlugins::new(plugins, Default::default()).offering(Arc::clone(&helpers));
        let need = media.need_of(&missing);
        assert_eq!(need.helper.is_some(), offered, "{name}");
        assert!(
            need.fact.value.as_str().contains("anyview-mpv"),
            "{name}: the row still says what it needs"
        );
    }
    // Without a helpers file nothing is offered, whatever the manifest says.
    let plugins = unready(missing("/usr/bin/mpv"));
    let missing = missing_of(&plugins);
    let media = MediaPlugins::new(plugins, Default::default());
    assert_eq!(media.need_of(&missing).helper, None);
}

#[test]
fn a_picture_plugin_whose_tool_is_absent_offers_the_tool_for_its_kind_of_file() {
    use anyview_core::{FileHead, FileName, SniffStep, sniff};
    let machine = Machine::fedora();
    let (helpers, _) = host(&machine, Installed::Installed, &[]);
    let sniffed = |name: &str, bytes: &[u8]| {
        let SniffStep::Done(sniffed) = sniff(&FileHead::new(bytes), &FileName::new(name).unwrap())
        else {
            panic!("{name}");
        };
        sniffed
    };
    let heic = sniffed("a.heic", b"\0\0\0\x18ftypheic\0\0\0\0mif1heic");
    let raw = sniffed("a.nef", b"II*\0\x08\0\0\0\0\0\0\0");
    let offering = ImageHost::without_plugins().offering(helpers);
    assert_eq!(
        offering.need_for_tools(&heic).helper,
        Some(Helper::HeicDecode)
    );
    assert_eq!(
        offering.need_for_tools(&raw).helper,
        Some(Helper::RawDecode)
    );
    assert_eq!(
        ImageHost::without_plugins().need_for_tools(&heic).helper,
        None
    );
}
