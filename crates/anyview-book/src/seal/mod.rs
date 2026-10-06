//! A chapter made safe to show: its markup rebuilt from an allowlist of elements and attributes,
//! its stylesheets inlined and sealed, its images inlined as `data:` URLs. No script, no frame, no
//! form and no address that would load from the network survives, because only what is known is
//! written and everything else is left out.

mod assets;
mod css;
mod element;
mod entities;
mod escapes;
mod tokens;
mod walk;

pub(crate) use assets::Assets;

/// A chapter ready for a sealed frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chapter {
    /// The chapter's stylesheets, sealed, in the order the chapter names them.
    pub styles: String,
    /// The chapter's content: markup with no script, frame or remote address.
    pub body: String,
}

/// `html`, a chapter in `directory` (the entry name of its folder, with the trailing slash), as a
/// [`Chapter`] whose files are read through `assets`.
pub(crate) fn seal(html: &str, directory: &str, assets: &dyn Assets) -> Chapter {
    walk::seal(html, directory, assets)
}

#[cfg(test)]
mod tests;
