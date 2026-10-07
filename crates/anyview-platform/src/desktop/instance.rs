//! Single instance over the session bus: the first viewer owns `org.quire.Anyview1`, and a
//! later launch calls `Open`, `Peek`, `Play` or `Handoff` on it. The `.service` file in `dist/` lets the
//! bus start the viewer when someone calls the name while none runs.

use super::intents;
use crate::env::Env;
use crate::error::PlatformError;
use crate::handoff;
use crate::instance::{Claim, Instance, Primary, Request};
use anyview_core::FilePath;
use std::path::PathBuf;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use zbus::{fdo, interface};

/// The well-known name the viewer owns.
pub const BUS_NAME: &str = "org.quire.Anyview1";
const OBJECT_PATH: &str = "/org/quire/Anyview1";

/// [`Instance`] on the session bus.
#[derive(Debug, Clone)]
pub struct DbusInstance {
    env: Env,
}

impl DbusInstance {
    /// An instance claim on `env`'s session bus.
    pub fn new(env: Env) -> Self {
        DbusInstance { env }
    }
}

impl Instance for DbusInstance {
    async fn claim(&self, request: &Request) -> Result<Claim, PlatformError> {
        let (sender, requests) = unbounded_channel();
        let built = self
            .env
            .session_builder()?
            .name(BUS_NAME)
            // The first viewer keeps the name: a later one must be told it is taken, not take it.
            .map(|builder| {
                builder
                    .allow_name_replacements(false)
                    .replace_existing_names(false)
            })
            .and_then(|builder| {
                builder.serve_at(
                    OBJECT_PATH,
                    Service {
                        sender: sender.clone(),
                    },
                )
            })
            .map_err(|error| PlatformError::bus("register the viewer's name", error))?
            .build()
            .await;
        match built {
            Ok(connection) => {
                // The agent layer is an extra: a viewer that cannot offer it (no docket on this
                // desktop, its name taken) still is the viewer.
                if let Err(error) = intents::serve(&connection, sender).await {
                    eprintln!("anyview: not offered to the agent layer: {error}");
                }
                Ok(Claim::Primary(Primary::new(requests, Box::new(connection))))
            }
            Err(zbus::Error::NameTaken) => {
                forward(&self.env, request).await?;
                Ok(Claim::Forwarded)
            }
            Err(error) => Err(PlatformError::bus("register the viewer's name", error)),
        }
    }
}

/// Call the running viewer with `request`, on a connection to the session bus the caller owns
/// (the launcher has its own). The bus starts the viewer for the call when none runs and the
/// activation file is installed.
pub async fn forward_over(
    connection: &zbus::Connection,
    request: &Request,
) -> Result<(), PlatformError> {
    let call = |method: &'static str| (Some(BUS_NAME), OBJECT_PATH, Some(BUS_NAME), method);
    let reply = match request {
        Request::Open(files) => {
            let (name, path, interface, method) = call("Open");
            connection
                .call_method(name, path, interface, method, &(paths(files),))
                .await
        }
        Request::Peek(file) => {
            let (name, path, interface, method) = call("Peek");
            connection
                .call_method(name, path, interface, method, &(text(file),))
                .await
        }
        Request::Play(file) => {
            let (name, path, interface, method) = call("Play");
            connection
                .call_method(name, path, interface, method, &(text(file),))
                .await
        }
        Request::Handoff(handed) => {
            let (name, path, interface, method) = call("Handoff");
            let wire = handoff::encode(handed);
            connection
                .call_method(
                    name,
                    path,
                    interface,
                    method,
                    &(wire.file, wire.resume, wire.results, wire.entries),
                )
                .await
        }
    };
    reply
        .map(drop)
        .map_err(|error| PlatformError::bus("forward the request to the running viewer", error))
}

/// Call the running viewer with `request` over a connection of `env`'s.
async fn forward(env: &Env, request: &Request) -> Result<(), PlatformError> {
    let connection = env
        .session_builder()?
        .build()
        .await
        .map_err(|error| PlatformError::bus("connect to the session bus", error))?;
    forward_over(&connection, request).await
}

fn text(file: &FilePath) -> String {
    file.as_path().to_string_lossy().into_owned()
}

fn paths(files: &[FilePath]) -> Vec<String> {
    files.iter().map(text).collect()
}

/// A path from the wire, or the reason it is not one a request can name.
fn parse_path(text: &str) -> fdo::Result<FilePath> {
    FilePath::new(PathBuf::from(text)).map_err(|error| fdo::Error::InvalidArgs(error.to_string()))
}

/// The object other launches call.
struct Service {
    sender: UnboundedSender<Request>,
}

impl Service {
    fn hand_over(&self, request: Request) -> fdo::Result<()> {
        self.sender
            .send(request)
            .map_err(|_| fdo::Error::Failed("the viewer is closing".to_owned()))
    }
}

#[interface(name = "org.quire.Anyview1")]
impl Service {
    /// Open files in the window; an empty list shows the window.
    fn open(&self, files: Vec<String>) -> fdo::Result<()> {
        let files = files
            .iter()
            .map(|file| parse_path(file))
            .collect::<fdo::Result<Vec<_>>>()?;
        self.hand_over(Request::Open(files))
    }

    /// Show a file in the quick-look window.
    fn peek(&self, file: String) -> fdo::Result<()> {
        self.hand_over(Request::Peek(parse_path(&file)?))
    }

    /// Open a file and start it playing.
    fn play(&self, file: String) -> fdo::Result<()> {
        self.hand_over(Request::Play(parse_path(&file)?))
    }

    /// Open a file as the launcher's pane left it: `resume` is the JSON of where the pane had
    /// it, `results` the id of the search whose result paths `entries` are (none: the file
    /// came from no list).
    fn handoff(
        &self,
        file: String,
        resume: String,
        results: u64,
        entries: Vec<String>,
    ) -> fdo::Result<()> {
        let wire = handoff::Wire {
            file,
            resume,
            results,
            entries,
        };
        let handed = handoff::decode(&wire).map_err(fdo::Error::InvalidArgs)?;
        self.hand_over(Request::Handoff(handed))
    }
}
