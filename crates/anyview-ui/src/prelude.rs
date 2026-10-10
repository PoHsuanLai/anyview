//! The names a host uses to put a viewer window on screen: the roots it mounts, the one value it
//! wires a window from, the requests the window sends back and the pool it brings. `use
//! anyview_ui::prelude::*;` brings them in.
//!
//! What belongs here is what a binary or a test harness names to build and run a window
//! ([`Services`], [`Edge`], [`Launch`], [`ViewerApp`]) and the seams it most often fills in
//! ([`Workers`], [`HostRequest`], [`PlatformAbilities`]). The state machines, the stages and the
//! views are reached from the crate root by the code that drives them.

pub use crate::{
    DesktopService, Edge, HostRequest, Launch, Opened, PlatformAbilities, Services, ViewerApp,
    WelcomeApp, Work, Workers,
};
