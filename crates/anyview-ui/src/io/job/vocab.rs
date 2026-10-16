//! The jobs a window hands its workers and the results that come back.

use super::super::error::OpenError;
use super::super::notice::Notice;
use super::super::seams::ResumeKeeper;
use super::open::{Keeping, OpenPort, Opened, Preloaded};
use crate::families::{
    FoundHits, LineWindow, LoadedDoc, PdfAnswer, PdfTask, SortedOrder, TableDoc, TextDoc, TreeDoc,
};
use crate::{ColumnSort, SaveEnd, SheetNo, TextView, Ticket, TypedText, VersionRow};
use anyview_core::{FilePath, FileStamp, FindOptions, LineIndex, Sequence};
use anyview_text::{EditRefusal, EditStamp, EditText, TableEdits};
use std::sync::Arc;

/// One unit of blocking work.
#[derive(Debug)]
pub enum Job {
    /// Find out what the file of this load is.
    Probe {
        /// The load this work belongs to.
        ticket: Ticket,
        /// The file.
        path: FilePath,
    },
    /// Make the cheap first frame of the probed file.
    Peek {
        /// The load this work belongs to.
        ticket: Ticket,
        /// What the probe found.
        probed: Opened,
        /// Where the finished open reports to.
        link: OpenPort,
    },
    /// Open the probed file in full.
    Open {
        /// The load this work belongs to.
        ticket: Ticket,
        /// What the probe found.
        probed: Opened,
        /// Where the finished open reports to.
        link: OpenPort,
    },
    /// Read and highlight `rows` lines of an open text file from `first`.
    Lines {
        /// The load this work belongs to.
        ticket: Ticket,
        /// The open text file.
        doc: Arc<TextDoc>,
        /// The first line of the window.
        first: LineIndex,
        /// How many lines to read.
        rows: u32,
    },
    /// Find every place `query` occurs in an open text file.
    Search {
        /// The load this work belongs to.
        ticket: Ticket,
        /// The open text file.
        doc: Arc<TextDoc>,
        /// What the person typed.
        query: TypedText,
        /// How it is matched.
        options: FindOptions,
        /// What the stage shows of the file: a Markdown file's page is searched as a page.
        view: TextView,
    },
    /// Find every cell of a sheet of an open table in which `query` occurs.
    SearchCells {
        /// The load this work belongs to.
        ticket: Ticket,
        /// The open table.
        doc: Arc<TableDoc>,
        /// The sheet to search.
        sheet: SheetNo,
        /// What the person typed.
        query: TypedText,
        /// How it is matched.
        options: FindOptions,
    },
    /// Find every key and value of an open tree in which `query` occurs.
    SearchNodes {
        /// The load this work belongs to.
        ticket: Ticket,
        /// The open tree.
        doc: Arc<TreeDoc>,
        /// What the person typed.
        query: TypedText,
        /// How it is matched.
        options: FindOptions,
    },
    /// Sort a sheet of an open table by a column.
    Sort {
        /// The load this work belongs to.
        ticket: Ticket,
        /// The open table.
        doc: Arc<TableDoc>,
        /// The sheet to sort.
        sheet: SheetNo,
        /// The column and the way it runs.
        sort: ColumnSort,
    },
    /// Read an open text file whole, to edit it in place.
    EditRead {
        /// The load this work belongs to.
        ticket: Ticket,
        /// The open text file.
        doc: Arc<TextDoc>,
    },
    /// Build the file an open table is with its edits made to it, to be written over the file.
    DataBytes {
        /// The load this work belongs to.
        ticket: Ticket,
        /// The open table.
        doc: Arc<TableDoc>,
        /// The edits to make, as they stood when the person saved.
        edits: Box<TableEdits>,
        /// The state of the edits the file will be in.
        stamp: EditStamp,
    },
    /// Probe and open the file `path`, for the person to arrive at it without waiting.
    Preload {
        /// The file.
        path: FilePath,
        /// Where the finished open reports to.
        link: OpenPort,
    },
    /// Read the stamp `path` has now.
    Stat {
        /// The file.
        path: FilePath,
    },
    /// List the folder `path` is in, as the sequence the arrow keys walk.
    Folder {
        /// A file in the folder.
        path: FilePath,
    },
    /// Draw tiles, a thumbnail or a search of an open PDF.
    Pdf(PdfTask),
    /// List the versions kept of the file `path`, for the Revert To sheet.
    Versions {
        /// The file.
        path: FilePath,
    },
    /// Write a view or a place to the host's store.
    Keep {
        /// Where the view or place is written.
        keeper: Arc<dyn ResumeKeeper>,
        /// What to write.
        keeping: Keeping,
    },
}

/// What a worker made of a job.
#[derive(Debug)]
pub enum Done {
    /// A tool a plugin runs is on the machine: the open file is opened again if it lacked it.
    Available(anyview_core::Helper),
    /// Asking the system to install this tool ended like this.
    Helped(anyview_core::Helper, crate::HelperEnd),
    /// The probe of `ticket`.
    Probed {
        /// The load this work belongs to.
        ticket: Ticket,
        /// What came of it.
        result: Result<Opened, OpenError>,
    },
    /// The first frame of `ticket`, or `None` when the file has no cheap one.
    Peeked {
        /// The load this work belongs to.
        ticket: Ticket,
        /// What came of it.
        result: Result<Option<LoadedDoc>, OpenError>,
    },
    /// The open of `ticket`.
    Opened {
        /// The load this work belongs to.
        ticket: Ticket,
        /// What came of it.
        result: Result<LoadedDoc, OpenError>,
    },
    /// A window of lines of the file of `ticket`.
    Lines {
        /// The load this work belongs to.
        ticket: Ticket,
        /// What came of it.
        result: Result<LineWindow, OpenError>,
    },
    /// The order a sort gave a sheet of the table of `ticket`; `None` when it was cut short.
    Sorted {
        /// The load this work belongs to.
        ticket: Ticket,
        /// What came of it.
        order: Option<SortedOrder>,
    },
    /// The text of the file of `ticket`, read for editing, or why it cannot be.
    EditText {
        /// The load this work belongs to.
        ticket: Ticket,
        /// What came of it.
        result: Result<EditText, EditRefusal>,
    },
    /// The file an open table makes with its edits, built to be written; or why it could not be.
    DataBytes {
        /// The load this work belongs to.
        ticket: Ticket,
        /// The state of the edits the file is in.
        stamp: EditStamp,
        /// What came of it.
        result: Result<Vec<u8>, OpenError>,
    },
    /// The host wrote the edited text over `path`, or could not.
    Saved {
        /// The file.
        path: FilePath,
        /// How the write ended.
        end: SaveEnd,
    },
    /// The hits of `query` in the file of `ticket`.
    Found {
        /// The load this work belongs to.
        ticket: Ticket,
        /// What the person typed.
        query: TypedText,
        /// How it was matched.
        options: FindOptions,
        /// What came of it.
        result: Result<FoundHits, OpenError>,
    },
    /// The file `path`, opened ahead; `None` when it could not be or was too large to be worth it.
    Preloaded {
        /// The file.
        path: FilePath,
        /// The file as opened ahead.
        loaded: Option<Preloaded>,
    },
    /// The stamp `path` has now; `None` when it cannot be read.
    Stamped {
        /// The file.
        path: FilePath,
        /// The size and time the file has now.
        stamp: Option<FileStamp>,
    },
    /// The folder of `path` as a sequence.
    Folder {
        /// A file in the folder.
        path: FilePath,
        /// What came of it.
        result: Result<Sequence, OpenError>,
    },
    /// The host saw `path` change on disk (`Edge::changed`).
    Changed {
        /// The file.
        path: FilePath,
    },
    /// What a PDF task made, for the load that asked.
    Pdf {
        /// The load this work belongs to.
        ticket: Ticket,
        /// What the task made.
        answer: PdfAnswer,
    },
    /// The player of the load `ticket` has news: the window drains its line.
    Media {
        /// The load this work belongs to.
        ticket: Ticket,
    },
    /// The versions kept of `path`, newest first.
    Versions {
        /// The file.
        path: FilePath,
        /// The versions, newest first.
        rows: Vec<VersionRow>,
    },
    /// A view or a place was written (or could not be, which the keeper has logged): nothing to do.
    Kept,
    /// The person chose these files in the host's file dialog.
    Chosen {
        /// The files chosen, in the dialog's order.
        files: Vec<FilePath>,
    },
    /// The host renamed the open file: it is `to` now.
    Moved {
        /// The new path.
        to: FilePath,
    },
    /// The host tells the person how a task ended (`Edge::notify`).
    Notice(Notice),
}
