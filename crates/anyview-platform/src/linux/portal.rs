//! What every call to a desktop portal shares: the portal's bus name, how a request ends, and the
//! answer that comes back on a Request object. Each call answers on a path known in advance from
//! its `handle_token` option, so the answer is subscribed to before the call is made and cannot be
//! missed.

use crate::error::PlatformError;
use futures_util::StreamExt;
use std::collections::HashMap;
use zbus::zvariant::OwnedValue;
use zbus::{Connection, Proxy};

pub(super) const DESKTOP: &str = "org.freedesktop.portal.Desktop";
pub(super) const DESKTOP_PATH: &str = "/org/freedesktop/portal/desktop";
const REQUEST: &str = "org.freedesktop.portal.Request";
/// No parent window: winit exports no xdg-foreign handle to name one with.
pub(super) const UNPARENTED: &str = "";

/// How a portal request ended (its `Response` code).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Response {
    Success,
    Cancelled,
    Ended,
}

impl Response {
    pub(super) fn of(code: u32) -> Self {
        match code {
            0 => Response::Success,
            1 => Response::Cancelled,
            _ => Response::Ended,
        }
    }
}

/// An answer's results, by key.
pub(super) type Results = HashMap<String, OwnedValue>;

/// A failed portal call named `call`, for `reason`.
pub(super) fn failure(call: &'static str, reason: &str) -> PlatformError {
    PlatformError::bus(call, reason)
}

/// The answers to the request that `handle` will name, subscribed before the call.
pub(super) async fn answers(
    connection: &Connection,
    handle: &str,
    call: &'static str,
) -> Result<Answers, PlatformError> {
    let sender = connection
        .unique_name()
        .map(|name| name.as_str().to_owned())
        .unwrap_or_default();
    let subscribe = |error| PlatformError::bus("subscribe to the portal's answer", error);
    let request = Proxy::new(connection, DESKTOP, request_path(&sender, handle), REQUEST)
        .await
        .map_err(subscribe)?;
    let signals = request
        .receive_signal("Response")
        .await
        .map_err(subscribe)?;
    Ok(Answers { signals, call })
}

/// A request's `Response` signals.
pub(super) struct Answers {
    signals: zbus::proxy::SignalStream<'static>,
    call: &'static str,
}

impl Answers {
    /// The next answer: its code and results.
    pub(super) async fn next_answer(&mut self) -> Result<(Response, Results), PlatformError> {
        let message = self
            .signals
            .next()
            .await
            .ok_or_else(|| failure(self.call, "the portal went away before answering"))?;
        let (code, results): (u32, Results) = message
            .body()
            .deserialize()
            .map_err(|error| failure(self.call, &error.to_string()))?;
        Ok((Response::of(code), results))
    }
}

/// The Request object a call with `handle_token` = `handle` answers on: the caller's unique
/// name without its colon, dots as underscores (the portal's documented rule).
pub(super) fn request_path(sender: &str, handle: &str) -> String {
    let sender = sender.trim_start_matches(':').replace('.', "_");
    format!("{DESKTOP_PATH}/request/{sender}/{handle}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_request_path_follows_the_portals_rule() {
        assert_eq!(
            request_path(":1.42", "print"),
            "/org/freedesktop/portal/desktop/request/1_42/print"
        );
    }

    #[test]
    fn response_codes_map_to_how_the_request_ended() {
        const CASES: &[(&str, u32, Response)] = &[
            ("success", 0, Response::Success),
            ("cancelled", 1, Response::Cancelled),
            ("ended", 2, Response::Ended),
            ("unknown", 9, Response::Ended),
        ];
        for (name, code, want) in CASES {
            assert_eq!(Response::of(*code), *want, "{name}");
        }
    }
}
