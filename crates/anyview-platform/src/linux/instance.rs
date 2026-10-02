//! Single instance over the session bus: the first viewer owns `org.quire.Anyview1`, and a
//! later launch calls `Open`, `Peek` or `Play` on it. The `.service` file in `dist/` lets the
//! bus start the viewer when someone calls the name while none runs.

use crate::env::Env;
use crate::error::PlatformError;
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
            .and_then(|builder| builder.serve_at(OBJECT_PATH, Service { sender }))
            .map_err(|error| PlatformError::bus("register the viewer's name", error))?
            .build()
            .await;
        match built {
            Ok(connection) => Ok(Claim::Primary(Primary::new(requests, Box::new(connection)))),
            Err(zbus::Error::NameTaken) => {
                forward(&self.env, request).await?;
                Ok(Claim::Forwarded)
            }
            Err(error) => Err(PlatformError::bus("register the viewer's name", error)),
        }
    }
}

/// Call the running viewer with `request`.
async fn forward(env: &Env, request: &Request) -> Result<(), PlatformError> {
    let connection = env
        .session_builder()?
        .build()
        .await
        .map_err(|error| PlatformError::bus("connect to the session bus", error))?;
    let to = |method: &'static str| (Some(BUS_NAME), OBJECT_PATH, Some(BUS_NAME), method);
    let reply = match request {
        Request::Open(files) => {
            let (name, path, interface, method) = to("Open");
            connection
                .call_method(name, path, interface, method, &(paths(files),))
                .await
        }
        Request::Peek(file) => {
            let (name, path, interface, method) = to("Peek");
            connection
                .call_method(name, path, interface, method, &(text(file),))
                .await
        }
        Request::Play(file) => {
            let (name, path, interface, method) = to("Play");
            connection
                .call_method(name, path, interface, method, &(text(file),))
                .await
        }
    };
    reply
        .map(drop)
        .map_err(|error| PlatformError::bus("forward the request to the running viewer", error))
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
}
