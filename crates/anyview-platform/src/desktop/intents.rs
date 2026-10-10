//! The viewer as an app of docket, the desktop's agent layer: `org.quire.IntentProvider1` served on
//! the app's own bus name, `org.quire.Anyview`, so the router can ask the viewer to open files.
//! The declared actions are in `dist/intents/org.quire.Anyview.toml`, which this module embeds, so
//! the file the installer ships and the one the viewer answers for are one file.
//!
//! Every action is a request the viewer already takes from another launch (`Request::Open`): a call
//! lands on the same channel the single-instance name feeds, and the program handles it like any
//! forwarded launch. Nothing here reads a file, and no action writes one.

use crate::error::PlatformError;
use crate::instance::Request;
use anyview_core::FilePath;
use docket_client::{ContextSource, IntentProvider, SummonTarget};
use docket_core::{
    AppRefusal, ContextScope, ContextSnapshot, EntityRef, FailText, FileRef, Follow, Here, Hit,
    Invocation, LabelText, Manifest, Outcome, Preview, Selection, SuggestAsk, SummonAnswer,
    SummonOrigin, SummonSerial, TargetValue, TextTarget, UndoFault, UndoToken, Undoable,
    ValidManifest, Visible, WindowPrivacy, validate,
};
use porter_core::Count;
use prov::{Actor, Confidentiality, EntityId, Integrity, Label, Labelled, Source};
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::mpsc::Sender;

/// The bus name docket's router calls: the viewer's app name, which docket's activation file starts.
pub const AGENT_BUS_NAME: &str = "org.quire.Anyview";

/// The action that opens files in the window.
pub const OPEN_FILES: &str = "anyview.file.open";

/// The most files one call may open: a window of tabs, not a folder's worth.
const MOST_FILES: usize = 100;

/// The manifest the installer ships, parsed where the viewer starts.
const MANIFEST: &str = include_str!("../../../../dist/intents/org.quire.Anyview.toml");

/// The viewer's declaration, read from the embedded file.
pub(super) fn manifest() -> Result<ValidManifest, PlatformError> {
    let parsed: Manifest = toml::from_str(MANIFEST).map_err(|error| intents_error(&error))?;
    validate(parsed).map_err(|error| intents_error(&error))
}

fn intents_error(reason: &dyn std::fmt::Display) -> PlatformError {
    PlatformError::Bus {
        call: "read the viewer's intents manifest",
        reason: reason.to_string(),
    }
}

/// Serve the viewer's intents on `connection`, which already owns the viewer's own name, and claim
/// the app name docket calls. Requests the router makes arrive on `sender`.
pub(super) async fn serve(
    connection: &zbus::Connection,
    sender: Sender<Request>,
) -> Result<(), PlatformError> {
    let manifest = manifest()?;
    let window = PrivateWindow::new(&manifest);
    let provider = ViewerIntents::new(manifest, sender);
    docket_client::serve_on(connection, provider, window, NoPrompt)
        .await
        .map_err(|error| PlatformError::Bus {
            call: "offer the viewer to the agent layer",
            reason: error.to_string(),
        })
}

/// Answers the router's calls by handing requests to the viewer.
pub(super) struct ViewerIntents {
    manifest: ValidManifest,
    sender: Sender<Request>,
}

impl ViewerIntents {
    pub(super) fn new(manifest: ValidManifest, sender: Sender<Request>) -> ViewerIntents {
        ViewerIntents { manifest, sender }
    }

    fn open(&self, target: &TargetValue) -> Result<Outcome, AppRefusal> {
        let TargetValue::Files(refs) = target else {
            return Err(AppRefusal::Unsupported);
        };
        let files = existing_files(refs)?;
        let count = files.len();
        self.sender
            .send(Request::Open(files))
            .map_err(|_| AppRefusal::Busy)?;
        Ok(Outcome {
            value: None,
            said: LabelText::parse(&said(count)).ok(),
            show: Preview::None,
            undo: Undoable::No,
            follow: Follow::Nothing,
        })
    }
}

/// "Opened 1 file in Viewer" (the program's visible name).
fn said(count: usize) -> String {
    let noun = if count == 1 { "file" } else { "files" };
    format!("Opened {count} {noun} in {}", anyview_core::APP_NAME)
}

/// The files a call names, each an absolute path to something that exists; one that is not refuses
/// the whole call before anything opens, so the person never sees half of a request.
fn existing_files(refs: &[FileRef]) -> Result<Vec<FilePath>, AppRefusal> {
    if refs.is_empty() || refs.len() > MOST_FILES {
        return Err(AppRefusal::Failed(FailText(
            "name between one and a hundred files".to_owned(),
        )));
    }
    refs.iter()
        .map(|file| {
            let path = FilePath::new(file.as_str()).map_err(|_| {
                AppRefusal::Failed(FailText("give each file as an absolute path".to_owned()))
            })?;
            match Path::new(file.as_str()).try_exists() {
                Ok(true) => Ok(path),
                Ok(false) | Err(_) => Err(AppRefusal::Failed(FailText(
                    "a file named does not exist".to_owned(),
                ))),
            }
        })
        .collect()
}

impl IntentProvider for ViewerIntents {
    fn manifest(&self) -> &ValidManifest {
        &self.manifest
    }

    async fn perform(&self, inv: Invocation) -> Result<Outcome, AppRefusal> {
        match inv.action.as_str() {
            OPEN_FILES => self.open(&inv.target),
            _ => Err(AppRefusal::Unsupported),
        }
    }

    async fn dry_run(&self, _inv: Invocation) -> Result<Preview, AppRefusal> {
        Err(AppRefusal::Unsupported)
    }

    async fn undo(&self, _token: UndoToken, _actor: Actor) -> Result<(), UndoFault> {
        Err(UndoFault::Gone)
    }

    async fn search(&self, _text: &str) -> Vec<Hit> {
        Vec::new()
    }

    async fn preview(&self, _id: &EntityId) -> Preview {
        Preview::None
    }

    async fn suggest(&self, _ask: SuggestAsk) -> Vec<EntityRef> {
        Vec::new()
    }
}

/// What the viewer tells the router about the window: nothing but that it is the viewer. The
/// viewer holds no entities and its documents are the person's, so the window is reported private.
pub(super) struct PrivateWindow {
    app: porter_core::AppName,
}

impl PrivateWindow {
    pub(super) fn new(manifest: &ValidManifest) -> PrivateWindow {
        PrivateWindow {
            app: manifest.manifest().app.clone(),
        }
    }
}

impl ContextSource for PrivateWindow {
    fn snapshot(&self, _scope: ContextScope) -> ContextSnapshot {
        ContextSnapshot {
            app: self.app.clone(),
            window: Labelled {
                value: String::new(),
                label: Label {
                    integrity: Integrity::Trusted,
                    confidentiality: Confidentiality::Public,
                    classes: BTreeSet::new(),
                    sources: BTreeSet::from([Source::App(self.app.clone())]),
                },
            },
            here: Here::Nowhere,
            selection: Selection::Nothing,
            visible: Visible {
                kind: None,
                items: Vec::new(),
                total: Count(0),
            },
            text_target: TextTarget::None,
            privacy: WindowPrivacy::Private,
        }
    }
}

/// The viewer has no prompt field to summon into.
pub(super) struct NoPrompt;

impl SummonTarget for NoPrompt {
    fn summon(&self, _serial: SummonSerial, _origin: SummonOrigin) -> SummonAnswer {
        SummonAnswer::Declined
    }
}
