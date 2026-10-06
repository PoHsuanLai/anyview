//! Opening a web or mail address with the program that handles it.

use crate::error::PlatformError;

/// Open an address.
pub trait OpenLink {
    /// Hand `uri` to the desktop's handler for its scheme. The caller has already refused the
    /// schemes it does not open.
    fn open(&self, uri: &str) -> Result<(), PlatformError>;
}
