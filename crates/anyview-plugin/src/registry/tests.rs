use super::*;
use crate::missing::Package;
use anyview_core::{FormatKind, Mime};

/// A candidate whose manifest provides `probe` for `kinds` (and exports to `targets` when any)
/// from `/bin/<id>`.
fn candidate(id: &str, protocol: u32, origin: Origin, kinds: &str, mimes: &str) -> Candidate {
    let text = format!(
        "id = \"{id}\"\nname = \"{id}\"\nprotocol = {protocol}\n[program]\npath = \"/bin/{id}\"\n\
         [[provides]]\ncapability = \"probe\"\nkinds = [{kinds}]\nmimes = [{mimes}]\n"
    );
    Candidate {
        manifest: Manifest::parse(&text).unwrap(),
        origin,
        readiness: Readiness::Ready,
    }
}

fn video() -> Subject<'static> {
    Subject {
        kind: FormatKind::Video,
        mime: None,
    }
}

fn serving_id(plugins: &Plugins, subject: &Subject<'_>) -> Option<String> {
    plugins
        .serving(Capability::Probe, subject)
        .map(|plugin| plugin.manifest.id.as_str().to_owned())
}

#[test]
fn the_user_directory_overrides_the_system_one_for_an_id() {
    let system = candidate("tool", 1, Origin::System, "\"video\"", "");
    let user = candidate("tool", 1, Origin::User, "\"audio\"", "");
    let plugins = Plugins::resolve(vec![system, user.clone()]);
    assert_eq!(plugins.installed().len(), 1);
    assert_eq!(plugins.installed()[0].origin, Origin::User);
    // The user's copy handles audio only, so the system's video is gone with it.
    assert_eq!(serving_id(&plugins, &video()), None);
}

#[test]
fn a_higher_protocol_beats_the_user_directory() {
    // Protocol 2 is beyond this viewer, so it is set aside and the older one serves.
    let newer = candidate("tool", 2, Origin::System, "\"video\"", "");
    let older = candidate("tool", 1, Origin::User, "\"video\"", "");
    let plugins = Plugins::resolve(vec![newer, older]);
    assert_eq!(plugins.installed()[0].manifest.protocol, 1);
    assert_eq!(
        plugins.unusable()[0].reason,
        PluginError::ProtocolUnsupported {
            protocol: 2,
            supported: PROTOCOL_VERSION
        }
    );
}

#[test]
fn rank_prefers_a_higher_protocol_then_the_user() {
    let low_user = Installed {
        manifest: candidate("a", 1, Origin::User, "\"video\"", "").manifest,
        origin: Origin::User,
    };
    let high_system = Installed {
        manifest: candidate("a", 3, Origin::System, "\"video\"", "").manifest,
        origin: Origin::System,
    };
    let high_user = Installed {
        manifest: candidate("a", 3, Origin::User, "\"video\"", "").manifest,
        origin: Origin::User,
    };
    let mut all = vec![low_user.clone(), high_system.clone(), high_user.clone()];
    all.sort_by_key(rank);
    assert_eq!(all, [high_user, high_system, low_user]);
}

#[test]
fn a_plugin_whose_program_is_unusable_never_shadows_a_working_one() {
    let mut broken = candidate("tool", 1, Origin::User, "\"video\"", "");
    broken.readiness = Readiness::Unready(PluginError::FileMissing {
        path: "/bin/tool".into(),
    });
    let working = candidate("tool", 1, Origin::System, "\"video\"", "");
    let plugins = Plugins::resolve(vec![broken, working]);
    assert_eq!(plugins.installed()[0].origin, Origin::System);
    assert_eq!(plugins.unusable().len(), 1);
    assert_eq!(plugins.unusable()[0].origin, Origin::User);
}

#[test]
fn a_mime_match_beats_a_kind_match_and_ties_go_by_id() {
    let by_kind = candidate("a-kind", 1, Origin::System, "\"video\"", "");
    let by_mime = candidate("z-mime", 1, Origin::System, "", "\"video/x-odd\"");
    let plugins = Plugins::resolve(vec![by_mime, by_kind]);
    let odd = Mime::parse("video/x-odd").unwrap();
    let with_mime = Subject {
        kind: FormatKind::Video,
        mime: Some(&odd),
    };
    assert_eq!(serving_id(&plugins, &with_mime).as_deref(), Some("z-mime"));
    assert_eq!(serving_id(&plugins, &video()).as_deref(), Some("a-kind"));
}

#[test]
fn the_order_discovery_listed_files_in_does_not_matter() {
    let a = candidate("alpha", 1, Origin::System, "\"video\"", "");
    let b = candidate("beta", 1, Origin::System, "\"video\"", "");
    let forward = Plugins::resolve(vec![a.clone(), b.clone()]);
    let backward = Plugins::resolve(vec![b, a]);
    assert_eq!(forward, backward);
    assert_eq!(serving_id(&forward, &video()).as_deref(), Some("alpha"));
}

#[test]
fn a_kind_nobody_serves_names_its_package_or_nothing() {
    let plugins = Plugins::none();
    let audio = Subject {
        kind: FormatKind::Audio,
        mime: None,
    };
    let Route::Missing(missing) = plugins.route(Capability::Probe, &audio) else {
        panic!("a package is known for audio");
    };
    assert_eq!(missing.package, Package::from_static("anyview-ffmpeg"));
    let pdf = Subject {
        kind: FormatKind::Pdf,
        mime: None,
    };
    assert_eq!(plugins.route(Capability::Probe, &pdf), Route::Unserved);
}

#[test]
fn a_served_kind_is_routed_to_its_plugin() {
    let plugins = Plugins::resolve(vec![candidate("tool", 1, Origin::User, "\"video\"", "")]);
    match plugins.route(Capability::Probe, &video()) {
        Route::Served(plugin) => assert_eq!(plugin.manifest.id.as_str(), "tool"),
        Route::Missing(_) | Route::Unserved => panic!("served"),
    }
    // Served for probe, not for thumbnail: the plugin does not provide it.
    assert!(matches!(
        plugins.route(Capability::Thumbnail, &video()),
        Route::Missing(_)
    ));
}

#[test]
fn the_export_sheet_lists_each_target_once_for_the_first_plugin() {
    let export = |id: &str, targets: &str| Candidate {
        manifest: Manifest::parse(&format!(
            "id = \"{id}\"\nname = \"{id}\"\nprotocol = 1\n[program]\npath = \"/bin/{id}\"\n\
             [[provides]]\ncapability = \"export\"\nkinds = [\"audio\"]\ntargets = [{targets}]\n"
        ))
        .unwrap(),
        origin: Origin::System,
        readiness: Readiness::Ready,
    };
    let plugins = Plugins::resolve(vec![
        export("b", "\"mp3\", \"ogg\""),
        export("a", "\"mp3\", \"flac\""),
    ]);
    let audio = Subject {
        kind: FormatKind::Audio,
        mime: None,
    };
    let offered: Vec<_> = plugins
        .export_targets(&audio)
        .iter()
        .map(|(plugin, target)| (plugin.manifest.id.as_str(), target.as_str()))
        .collect();
    assert_eq!(offered, [("a", "mp3"), ("a", "flac"), ("b", "ogg")]);
    assert!(plugins.export_targets(&video()).is_empty());
}
