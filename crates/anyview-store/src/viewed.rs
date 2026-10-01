//! `Viewed`: when a file was last viewed, handed in by the caller (this crate reads no clock).

/// Seconds since the Unix epoch, the unit the launcher's own clock counts in, so the launcher
/// reads a history row's time without converting.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct Viewed(pub u64);

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
