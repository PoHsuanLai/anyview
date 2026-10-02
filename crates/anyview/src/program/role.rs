//! Single instance: be the viewer, or hand the request to the one that is.

use anyview_platform::{Claim, Instance, PlatformError, Primary, Request};

/// What this process is, once it has asked for the viewer's name.
#[derive(Debug)]
pub enum Role {
    /// Another process is the viewer and has the request: nothing is left to do.
    Forwarded,
    /// This process is the viewer; requests other launches forward arrive at the `Primary`.
    Primary(Primary),
    /// The bus could not say (no session bus, a refused call): this process opens its own
    /// request and takes no others, rather than dropping the request.
    Alone(PlatformError),
}

/// Ask `instance` for the viewer's name, forwarding `request` if it is taken.
pub async fn claim_role<I: Instance>(instance: &I, request: &Request) -> Role {
    match instance.claim(request).await {
        Ok(Claim::Forwarded) => Role::Forwarded,
        Ok(Claim::Primary(primary)) => Role::Primary(primary),
        Err(error) => Role::Alone(error),
    }
}
