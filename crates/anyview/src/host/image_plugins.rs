//! The pictures the viewer cannot decode itself, from the plugins that can: HEIC through the HEIF
//! plugin, a raw file in full through the RAW plugin. The registry says which plugin serves a file;
//! asking it runs a program of the person's, so a worker calls this and never the UI thread.

use anyview_core::{Fact, FactLabel, FactValue, PixelArea, Sniffed, Source};
use anyview_image::Rgba8;
use anyview_platform::{PlatformError, PluginRunner};
use anyview_plugin::{MissingPlugin, Plugins, Route, Subject};
use anyview_plugin_protocol::Capability;
use anyview_ui::{ImagePlugins, PluginPicture};

/// The media type every camera raw file is sniffed as.
const RAW: &str = "image/x-dcraw";

/// The plugins of this run as the views' source of decoded pictures.
#[derive(Debug, Clone)]
pub struct ImageHost {
    plugins: Plugins,
    runner: PluginRunner,
}

impl ImageHost {
    /// The plugins `plugins` holds, run by `runner`.
    pub fn new(plugins: Plugins, runner: PluginRunner) -> ImageHost {
        ImageHost { plugins, runner }
    }

    /// A host with no plugin installed: every kind a plugin serves says which package would.
    pub fn without_plugins() -> ImageHost {
        ImageHost::new(Plugins::none(), PluginRunner::default())
    }
}

/// What a plugin adds for this media type, in the words of the `Needs` row.
fn purpose(sniffed: &Sniffed) -> &'static str {
    if sniffed.mime().as_str() == RAW {
        "show it in full quality"
    } else {
        "show it"
    }
}

fn missing_package(missing: &MissingPlugin, sniffed: &Sniffed) -> Fact {
    missing.fact_for(purpose(sniffed))
}

/// The row for a plugin that is installed and whose tools are not: it names what to install.
fn missing_tools(sniffed: &Sniffed) -> Fact {
    let text = if sniffed.mime().as_str() == RAW {
        "LibRaw's dcraw_emu (or dcraw) for the RAW plugin (to show it in full quality)"
    } else {
        "libheif's heif-dec (or heif-convert) for the HEIF plugin (to show it)"
    };
    Fact {
        label: FactLabel::Needs,
        value: FactValue::text(text),
    }
}

impl ImagePlugins for ImageHost {
    fn decode(&self, source: &Source, sniffed: &Sniffed, max_area: PixelArea) -> PluginPicture {
        let subject = Subject {
            kind: sniffed.kind(),
            mime: Some(sniffed.mime()),
        };
        match self.plugins.route(Capability::Decode, &subject) {
            Route::Served(plugin) => {
                match self.runner.decode(plugin, source.path(), max_area) {
                    Ok(pixels) => match Rgba8::new(pixels.size(), pixels.rgba().to_vec()) {
                        Ok(picture) => PluginPicture::Pixels(picture),
                        Err(error) => PluginPicture::Failed(error.to_string()),
                    },
                    // The plugin is there and its greeting offers no decode: its tools are not.
                    Err(PlatformError::PluginLacks { .. }) => {
                        PluginPicture::Missing(missing_tools(sniffed))
                    }
                    Err(error) => PluginPicture::Failed(error.to_string()),
                }
            }
            Route::Missing(missing) => PluginPicture::Missing(missing_package(&missing, sniffed)),
            Route::Unserved => PluginPicture::Unserved,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::{
        ByteLen, FileHead, FileName, FilePath, FileStamp, ModTime, SniffStep, sniff,
    };

    fn sniffed(name: &str, head: &[u8]) -> Sniffed {
        match sniff(&FileHead::new(head), &FileName::new(name).unwrap()) {
            SniffStep::Done(sniffed) => sniffed,
            SniffStep::LookInside(_) => panic!("not a zip"),
        }
    }

    fn source(name: &str) -> Source {
        let stamp = FileStamp {
            len: ByteLen(10),
            modified: ModTime(0),
        };
        Source::new(FilePath::new(format!("/nowhere/{name}")).unwrap(), stamp)
    }

    #[test]
    fn with_no_plugin_installed_heic_and_raw_name_their_packages_and_a_png_is_unserved() {
        const HEIC: &[u8] = b"\0\0\0\x18ftypheic\0\0\0\0mif1heic";
        const TIFF: &[u8] = b"II*\0\x08\0\0\0\0\0\0\0";
        const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0";
        let host = ImageHost::without_plugins();
        let want = |name: &str, head: &[u8]| {
            host.decode(&source(name), &sniffed(name, head), PixelArea(1_000))
        };
        assert_eq!(
            want("a.heic", HEIC),
            PluginPicture::Missing(Fact {
                label: FactLabel::Needs,
                value: FactValue::text("anyview-heif (to show it)"),
            })
        );
        assert_eq!(
            want("a.nef", TIFF),
            PluginPicture::Missing(Fact {
                label: FactLabel::Needs,
                value: FactValue::text("anyview-raw (to show it in full quality)"),
            })
        );
        assert_eq!(want("a.png", PNG), PluginPicture::Unserved);
    }
}
