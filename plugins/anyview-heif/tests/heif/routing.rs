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
fn heic_is_served_by_the_plugin_when_it_is_installed() {
    let plugins = Scratch::new().discover(&[]);
    let heic = Mime::parse("image/heic").unwrap();
    for capability in [Capability::Decode, Capability::Thumbnail] {
        match plugins.route(capability, &subject(&heic)) {
            Route::Served(plugin) => assert_eq!(plugin.manifest.id.as_str(), "heif"),
            Route::Missing(_) | Route::Unserved => panic!("{capability:?} is not served"),
        }
    }
    let avif = Mime::parse("image/avif").unwrap();
    assert!(matches!(
        plugins.route(Capability::Decode, &subject(&avif)),
        Route::Served(_)
    ));
}

#[test]
fn without_the_plugin_heic_is_missing_and_names_the_package() {
    let plugins = Plugins::none();
    let heic = Mime::parse("image/heic").unwrap();
    let Route::Missing(missing) = plugins.route(Capability::Decode, &subject(&heic)) else {
        panic!("a missing plugin");
    };
    assert_eq!(missing.package.name(), "anyview-heif");
    assert_eq!(missing.fact().value.as_str(), "anyview-heif (to show it)");
}

#[test]
fn a_picture_that_needs_no_plugin_is_unserved_either_way() {
    let png = Mime::parse("image/png").unwrap();
    assert!(matches!(
        Plugins::none().route(Capability::Decode, &subject(&png)),
        Route::Unserved
    ));
    let installed = Scratch::new().discover(&[]);
    assert!(matches!(
        installed.route(Capability::Decode, &subject(&png)),
        Route::Unserved
    ));
}
