//! `Viewed`: when a file was last viewed, handed in by the caller (the store itself reads no clock;
//! [`system_clock`] is the one a program hands in).

/// Seconds since the Unix epoch, the unit the launcher's own clock counts in, so the launcher
/// reads a history row's time without converting.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct Viewed(pub u64);

/// The time as a program reads it, in [`Viewed`] seconds: the one place a program asks the time,
/// so a test passes a fixed one.
pub type Clock = std::sync::Arc<dyn Fn() -> Viewed + Send + Sync>;

/// The wall clock in whole seconds since the epoch; a clock set before it reads as the epoch itself.
pub fn system_clock() -> Clock {
    std::sync::Arc::new(|| {
        Viewed(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_secs()),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_viewed_time_is_stored_as_bare_seconds() {
        let json = serde_json::to_string(&Viewed(1_700_000_000)).unwrap();
        assert_eq!(json, "1700000000");
        assert_eq!(
            serde_json::from_str::<Viewed>(&json).unwrap(),
            Viewed(1_700_000_000)
        );
    }
}
