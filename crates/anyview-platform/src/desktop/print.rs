//! The desktop portal's print dialog, as quire's `ds-blitz` print path does it: `PreparePrint`
//! shows the dialog and answers the chosen settings and a token, then `Print` hands over the PDF
//! as a file descriptor with that token. Each call answers on a Request object whose path is
//! known in advance from the `handle_token` option, so the answer is subscribed to before the
//! call is made and cannot be missed.

use super::portal::{DESKTOP, DESKTOP_PATH, Response, UNPARENTED, answers, unreached};
use crate::env::Env;
use crate::error::PlatformError;
use crate::printer::{JobTitle, PrintOutcome, Printer};
use std::collections::HashMap;
use std::io::{Seek, SeekFrom, Write};
use std::os::fd::OwnedFd;
use zbus::zvariant::{Fd, Value};
use zbus::{Connection, Proxy};

const PRINT: &str = "org.freedesktop.portal.Print";

/// [`Printer`] through `org.freedesktop.portal.Print`.
#[derive(Debug, Clone)]
pub struct PortalPrinter {
    env: Env,
}

impl PortalPrinter {
    /// A printer on `env`'s session bus.
    pub fn new(env: Env) -> Self {
        PortalPrinter { env }
    }
}

/// Where a portal call failed: before anything was shown because no portal is there (the caller
/// falls back to a viewer), or otherwise, when the portal is there and failed; the failure keeps
/// its cause.
enum Stage {
    Before,
    After(PlatformError),
}

impl Printer for PortalPrinter {
    async fn print(&self, pdf: &[u8], title: &JobTitle) -> Result<PrintOutcome, PlatformError> {
        let builder = match self.env.session_builder() {
            Ok(builder) => builder,
            Err(error) => return Ok(no_dialog("find the session bus", &error)),
        };
        let connection = match builder.build().await {
            Ok(connection) => connection,
            Err(error) => return Ok(no_dialog("connect to the session bus", &error)),
        };
        let token = match prepare(&connection, &title.0).await {
            Ok(Some(token)) => token,
            Ok(None) => return Ok(PrintOutcome::Cancelled),
            Err(Stage::Before) => return Ok(PrintOutcome::NoDialog),
            Err(Stage::After(error)) => return Err(error),
        };
        submit(&connection, &title.0, pdf, token).await
    }
}

/// There is no session bus to reach a portal on; the reason goes to the log, because the caller
/// falls back to a viewer without saying why.
fn no_dialog(step: &str, error: &dyn std::fmt::Display) -> PrintOutcome {
    log::warn!("no print dialog: cannot {step}: {error}");
    PrintOutcome::NoDialog
}

/// The stage a failed portal call `step` ends in: `Before` if no portal is there, else the
/// failure, which is logged with its cause.
fn reach(step: &'static str) -> impl FnOnce(zbus::Error) -> Stage {
    move |error| unreached(CALL, step, &error).map_or(Stage::Before, Stage::After)
}

/// Show the dialog; the token if the person accepted, `None` if they cancelled.
async fn prepare(connection: &Connection, title: &str) -> Result<Option<u32>, Stage> {
    let mut answers = answers(connection, "prepare", CALL)
        .await
        .map_err(Stage::After)?;
    let empty: HashMap<&str, Value<'_>> = HashMap::new();
    let options: HashMap<&str, Value<'_>> = HashMap::from([
        ("handle_token", Value::from("prepare")),
        ("modal", Value::from(true)),
    ]);
    portal(connection)
        .await
        .map_err(reach("reach the print portal"))?
        .call_method(
            "PreparePrint",
            &(UNPARENTED, title, &empty, &empty, options),
        )
        .await
        .map_err(reach("ask the print portal"))?;
    let (response, results) = answers.next_answer().await.map_err(Stage::After)?;
    match response {
        Response::Success => results
            .get("token")
            .and_then(|value| u32::try_from(value).ok())
            .map(Some)
            .ok_or_else(|| Stage::After(portal_error("no token in the answer"))),
        Response::Cancelled => Ok(None),
        Response::Ended => Err(Stage::After(portal_error(
            "the dialog ended without an answer",
        ))),
    }
}

/// Hand `pdf` to the portal with the dialog's token.
async fn submit(
    connection: &Connection,
    title: &str,
    pdf: &[u8],
    token: u32,
) -> Result<PrintOutcome, PlatformError> {
    let file = sealed_file(pdf)?;
    let mut answers = answers(connection, "print", CALL).await?;
    let options: HashMap<&str, Value<'_>> = HashMap::from([
        ("handle_token", Value::from("print")),
        ("token", Value::from(token)),
        ("modal", Value::from(true)),
    ]);
    portal(connection)
        .await
        .map_err(|error| PlatformError::bus("reach the print portal", error))?
        .call_method("Print", &(UNPARENTED, title, Fd::from(&file), options))
        .await
        .map_err(|error| PlatformError::bus("print through the portal", error))?;
    match answers.next_answer().await?.0 {
        Response::Success => Ok(PrintOutcome::Printed),
        Response::Cancelled => Ok(PrintOutcome::Cancelled),
        Response::Ended => Err(portal_error("printing ended without success")),
    }
}

const CALL: &str = "print through the portal";

fn portal_error(reason: &str) -> PlatformError {
    PlatformError::bus(CALL, reason)
}

/// `pdf` in an anonymous in-memory file, rewound and sealed against change: the portal reads it
/// from the start, and nothing can alter what it reads.
fn sealed_file(pdf: &[u8]) -> Result<OwnedFd, PlatformError> {
    let sealing = |error: &dyn std::fmt::Display| portal_error(&error.to_string());
    let memfd = memfd::MemfdOptions::default()
        .allow_sealing(true)
        .create("anyview-print")
        .map_err(|error| sealing(&error))?;
    let mut file = memfd.as_file();
    file.write_all(pdf)
        .and_then(|()| file.seek(SeekFrom::Start(0)).map(drop))
        .map_err(|error| sealing(&error))?;
    memfd
        .add_seals(&[
            memfd::FileSeal::SealShrink,
            memfd::FileSeal::SealGrow,
            memfd::FileSeal::SealWrite,
            memfd::FileSeal::SealSeal,
        ])
        .map_err(|error| sealing(&error))?;
    Ok(OwnedFd::from(memfd.into_file()))
}

async fn portal(connection: &Connection) -> Result<Proxy<'static>, zbus::Error> {
    Proxy::new(connection, DESKTOP, DESKTOP_PATH, PRINT).await
}
