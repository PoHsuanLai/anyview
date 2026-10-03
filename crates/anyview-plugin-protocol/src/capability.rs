//! The things a plugin can offer.

use serde::{Deserialize, Serialize};

/// One thing a plugin offers for some kinds of file. A manifest lists them with what each
/// handles, and a plugin's `Hello` lists the ones it answers on the wire.
///
/// `Peek` and `Play` are never requests: a peek is the host asking `Probe` and `Thumbnail` in
/// turn, and playback is a player the host starts from the manifest's paths, which this protocol
/// does not carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// What a file is: rows of facts, with no pixels.
    Probe,
    /// The small picture and facts a launcher pane shows; the host asks `Probe` then `Thumbnail`.
    Peek,
    /// A small picture of a file.
    Thumbnail,
    /// A picture of a file at up to a pixel budget.
    Decode,
    /// Writing a file as another format, with progress.
    Export,
    /// Playing a recording; the manifest carries the player's paths.
    Play,
}

impl Capability {
    /// Every capability, in the order a manifest lists them.
    pub const ALL: [Capability; 6] = [
        Capability::Probe,
        Capability::Peek,
        Capability::Thumbnail,
        Capability::Decode,
        Capability::Export,
        Capability::Play,
    ];

    /// The name a manifest and a message spell it with.
    pub fn name(self) -> &'static str {
        match self {
            Capability::Probe => "probe",
            Capability::Peek => "peek",
            Capability::Thumbnail => "thumbnail",
            Capability::Decode => "decode",
            Capability::Export => "export",
            Capability::Play => "play",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_the_serde_spelling() {
        for capability in Capability::ALL {
            let json = serde_json::to_string(&capability).unwrap();
            assert_eq!(json, format!("\"{}\"", capability.name()));
        }
    }
}
