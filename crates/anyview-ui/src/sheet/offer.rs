//! Which media exports a recording offers right now. What can be written depends on the programs
//! the person has installed, which the host knows and the machines do not: it hands the window
//! this value with the recording, and the sheet lists only what it holds.

use anyview_core::{ExportChoice, Fact, Helper, MediaExport, MediaExportKind};

/// The kinds of media export on offer for one recording, and what is missing for the rest.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MediaOffer {
    kinds: Vec<MediaExportKind>,
    needs: Option<Fact>,
    helper: Option<Helper>,
}

impl MediaOffer {
    /// `kinds` on offer; `needs` is the package that would add the exports that are not.
    pub fn new(kinds: Vec<MediaExportKind>, needs: Option<Fact>) -> MediaOffer {
        MediaOffer {
            kinds,
            needs,
            helper: None,
        }
    }

    /// The same offer, whose missing exports `helper` would add: the sheet that names what is
    /// missing then offers to install it.
    pub fn installable(self, helper: Helper) -> MediaOffer {
        MediaOffer {
            helper: Some(helper),
            ..self
        }
    }

    /// The tool to offer to install for the exports that are not on offer, when the host can.
    pub fn helper(&self) -> Option<Helper> {
        self.helper
    }

    /// The kinds on offer, in the order the pop-up lists them.
    pub fn kinds(&self) -> &[MediaExportKind] {
        &self.kinds
    }

    /// The row that says which package adds what is not on offer, when something is missing.
    pub fn needs(&self) -> Option<&Fact> {
        self.needs.as_ref()
    }

    /// The export the sheet opens on: the first kind on offer with its default options.
    pub fn first(&self) -> Option<MediaExport> {
        self.kinds
            .first()
            .map(|kind| MediaExport::default_for(*kind))
    }

    /// Whether `kind` is on offer.
    pub fn offers(&self, kind: MediaExportKind) -> bool {
        self.kinds.contains(&kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::{FactLabel, FactValue, RasterTarget};

    #[test]
    fn the_sheet_opens_on_the_first_kind_offered_and_lists_only_those() {
        let offer = MediaOffer::new(vec![MediaExportKind::ToMp3, MediaExportKind::ToWav], None);
        assert_eq!(
            offer.first().map(|choice| choice.kind()),
            Some(MediaExportKind::ToMp3)
        );
        assert!(offer.offers(MediaExportKind::ToWav));
        assert!(!offer.offers(MediaExportKind::FramePng));
        assert_eq!(MediaOffer::default().first(), None, "nothing offered");
    }

    #[test]
    fn a_frame_offer_opens_on_a_frame_and_names_what_is_missing() {
        let needs = Fact::new(
            FactLabel::Needs,
            FactValue::text("anyview-ffmpeg (to convert it)"),
        );
        let offer = MediaOffer::new(vec![MediaExportKind::FramePng], Some(needs.clone()));
        assert_eq!(
            offer.first(),
            Some(MediaExport::CurrentFrame(RasterTarget::Png))
        );
        assert_eq!(offer.needs(), Some(&needs));
    }
}
