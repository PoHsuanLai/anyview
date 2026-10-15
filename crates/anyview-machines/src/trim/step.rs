//! The trim's transitions.

use super::model::{Choosing, MIN_KEPT, MediaTrim, TrimIn, TrimWindow};
use crate::stage::TrimEdge;
use anyview_core::MediaTime;

impl MediaTrim {
    /// The trim after `input`.
    #[must_use]
    pub fn step(self, input: TrimIn) -> MediaTrim {
        match input {
            TrimIn::Begin(whole) => {
                if self.choosing.is_some() || whole.0 <= MIN_KEPT {
                    return self;
                }
                let window = self.applied.unwrap_or(TrimWindow {
                    start: MediaTime::default(),
                    end: whole.0,
                });
                MediaTrim {
                    choosing: Some(Choosing { whole, window }),
                    ..self
                }
            }
            TrimIn::Move { edge, to } => {
                let Some(Choosing { whole, window }) = self.choosing else {
                    return self;
                };
                let window = match edge {
                    TrimEdge::Start => TrimWindow {
                        start: to.min(MediaTime(window.end.0.saturating_sub(MIN_KEPT.0))),
                        end: window.end,
                    },
                    TrimEdge::End => TrimWindow {
                        start: window.start,
                        end: to
                            .max(MediaTime(window.start.0.saturating_add(MIN_KEPT.0)))
                            .min(whole.0),
                    },
                };
                MediaTrim {
                    choosing: Some(Choosing { whole, window }),
                    ..self
                }
            }
            TrimIn::Apply => {
                let Some(Choosing { whole, window }) = self.choosing else {
                    return self;
                };
                let everything = window.start == MediaTime::default() && window.end == whole.0;
                MediaTrim {
                    choosing: None,
                    applied: (!everything).then_some(window),
                    undone: None,
                }
            }
            TrimIn::Cancel => MediaTrim {
                choosing: None,
                ..self
            },
            TrimIn::Undo if self.can_undo() => MediaTrim {
                applied: None,
                undone: self.applied,
                ..self
            },
            TrimIn::Redo if self.can_redo() => MediaTrim {
                applied: self.undone,
                undone: None,
                ..self
            },
            TrimIn::Undo | TrimIn::Redo => self,
            TrimIn::Discard => MediaTrim::default(),
        }
    }
}
