//! How the host routes HEIC: to the plugin when it is installed, to the package that would serve it
//! when it is not, and nowhere for a picture that needs no plugin.

use crate::support;

use anyview_core::{FormatKind, Mime};
use anyview_plugin::{Plugins, Route, Subject};
use anyview_plugin_protocol::Capability;
use support::Scratch;

fn subject<'a>(mime: &'a Mime) -> Subject<'a> {
    Subject {
        kind: FormatKind::Raster,
        mime: Some(mime),
    }
}

#[test]
fn raw_is_served_by_the_plugin_when_it_is_installed() {
    let plugins = Scratch::new().discover(&[]);
    let raw = Mime::parse("image/x-dcraw").unwrap();
    for capability in [Capability::Decode, Capability::Thumbnail] {
        match plugins.route(capability, &subject(&raw)) {
            Route::Served(plugin) => assert_eq!(plugin.manifest.id.as_str(), "raw"),
            Route::Missing(_) | Route::Unserved => panic!("{capability:?} is not served"),
        }
    }
    // HEIC is not this plugin's business: it is the HEIF plugin's, so with only this one installed
    // the package that would serve it is named instead.
    let heic = Mime::parse("image/heic").unwrap();
    assert!(matches!(
        plugins.route(Capability::Decode, &subject(&heic)),
        Route::Missing(missing) if missing.package.name() == "anyview-heif"
    ));
}

#[test]
fn without_the_plugin_raw_is_missing_and_names_the_package() {
    let plugins = Plugins::none();
    let raw = Mime::parse("image/x-dcraw").unwrap();
    let Route::Missing(missing) = plugins.route(Capability::Decode, &subject(&raw)) else {
        panic!("a missing plugin");
    };
    assert_eq!(missing.package.name(), "anyview-raw");
    assert_eq!(missing.fact().value.as_str(), "anyview-raw (to show it)");
}
