//! The Extension ids of the plugins this repository ships. They are reverse DNS, as every
//! Extension's is; before they became Extensions the same plugins were flat manifests named by a
//! short id (`heif.toml`), which discovery still reads for one release and maps to these.

use crate::missing::Package;

/// The short id a flat manifest used, and the Extension id it is now.
const SHIPPED: [(&str, &str); 4] = [
    ("ffmpeg", "org.quire.anyview.Ffmpeg"),
    ("heif", "org.quire.anyview.Heif"),
    ("raw", "org.quire.anyview.Raw"),
    ("mpv", "org.quire.anyview.Mpv"),
];

/// The Extension id of the shipped plugin whose flat manifest was called `short`, or `None` for
/// any other id.
#[must_use]
pub fn shipped_extension_id(short: &str) -> Option<&'static str> {
    SHIPPED
        .iter()
        .find(|(old, _)| *old == short)
        .map(|(_, id)| *id)
}

/// The Extension id of the plugin `package` names (`anyview-heif`).
#[must_use]
pub fn package_extension_id(package: Package) -> Option<&'static str> {
    shipped_extension_id(package.name.strip_prefix("anyview-")?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handles::Subject;
    use crate::manifest::PluginId;
    use crate::missing::{Package, suggested_package};
    use anyview_core::{FormatKind, Mime};
    use anyview_plugin_protocol::Capability;

    #[test]
    fn every_package_the_viewer_suggests_has_an_extension_id_that_is_a_valid_folder_name() {
        let heic = Mime::parse("image/heic").unwrap();
        let raw = Mime::parse("image/x-dcraw").unwrap();
        // what is asked, and the Extension that would serve it
        let cases: [(Capability, FormatKind, Option<&Mime>, &str); 4] = [
            (
                Capability::Probe,
                FormatKind::Video,
                None,
                "org.quire.anyview.Ffmpeg",
            ),
            (
                Capability::Play,
                FormatKind::Audio,
                None,
                "org.quire.anyview.Mpv",
            ),
            (
                Capability::Decode,
                FormatKind::Raster,
                Some(&heic),
                "org.quire.anyview.Heif",
            ),
            (
                Capability::Decode,
                FormatKind::Raster,
                Some(&raw),
                "org.quire.anyview.Raw",
            ),
        ];
        for (capability, kind, mime, want) in cases {
            let package: Package = suggested_package(capability, &Subject { kind, mime }).unwrap();
            let id = package_extension_id(package).unwrap();
            assert_eq!(id, want);
            assert!(PluginId::parse_dotted(id).is_ok(), "{id}");
        }
    }
}
