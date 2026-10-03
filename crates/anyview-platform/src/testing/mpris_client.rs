//! The desktop's end of the MPRIS player, for tests: what the control center reads and presses.

use super::PrivateBus;
use crate::linux::MPRIS_NAME;
use zbus::zvariant::OwnedValue;

const PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";

/// A client of the viewer's MPRIS player on a private bus.
#[derive(Debug, Clone)]
pub struct MprisClient {
    connection: zbus::Connection,
}

impl MprisClient {
    /// Connect to `bus` as the control center would.
    pub async fn connect(bus: &PrivateBus) -> Result<MprisClient, zbus::Error> {
        let connection = zbus::connection::Builder::address(bus.address())?
            .build()
            .await?;
        Ok(MprisClient { connection })
    }

    /// The player's `PlaybackStatus`: `Playing`, `Paused` or `Stopped`.
    pub async fn status(&self) -> Result<String, zbus::Error> {
        let reply = self
            .connection
            .call_method(
                Some(MPRIS_NAME),
                PATH,
                Some(PROPERTIES),
                "Get",
                &(PLAYER, "PlaybackStatus"),
            )
            .await?;
        let value: OwnedValue = reply.body().deserialize()?;
        Ok(String::try_from(value)?)
    }

    /// Call a method of the player that takes nothing (`Pause`, `PlayPause`, `Stop`, …).
    pub async fn press(&self, method: &str) -> Result<(), zbus::Error> {
        self.connection
            .call_method(Some(MPRIS_NAME), PATH, Some(PLAYER), method, &())
            .await
            .map(drop)
    }
}
