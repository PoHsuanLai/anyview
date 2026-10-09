//! What the export sheet is told of the open file, so it can offer only options that mean
//! something for it and check the ones typed against it.

use anyview_core::{PageCount, PageIndex, PixelSize, TimeRange};

/// The open file as the export sheet needs to know it. The default is a file that says nothing:
/// no page count, no picture size, no trim marks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExportFacts {
    /// How many pages the document has; `None` for a file that is not paged.
    pub pages: Option<PageCount>,
    /// The page the reader is on.
    pub page: PageIndex,
    /// The picture's size in pixels, when it is a picture.
    pub image: Option<PixelSize>,
    /// What the person marked to keep of a recording; `None` while no mark is set.
    pub marks: Option<TimeRange>,
}

impl ExportFacts {
    /// The page count when the document has more than one page: a range of a one-page document
    /// is not a choice.
    pub fn several_pages(&self) -> Option<PageCount> {
        self.pages.filter(|count| count.get() > 1)
    }
}
