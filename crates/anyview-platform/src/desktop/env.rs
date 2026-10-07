//! The session bus an [`Env`] names, as a connection builder.

use crate::env::{BusRoute, Env};
use crate::error::PlatformError;

impl Env {
    /// A connection builder for the session bus, or [`PlatformError::NoBus`] when the route is
    /// [`BusRoute::Absent`].
    pub(crate) fn session_builder(
        &self,
    ) -> Result<zbus::connection::Builder<'static>, PlatformError> {
        match &self.session {
            BusRoute::Usual => zbus::connection::Builder::session(),
            BusRoute::Address(address) => zbus::connection::Builder::address(address.as_str()),
            BusRoute::Absent => return Err(PlatformError::NoBus),
        }
        .map_err(|error| PlatformError::bus("connect to the session bus", error))
    }
}
