//! The viewer's protocol, as bayonet runs it: the messages of protocol version 1 and which one is
//! the greeting.

use anyview_plugin_protocol::{Capability, HostMessage, PROTOCOL_VERSION, PluginMessage};
use bayonet::run::{Greeting, Protocol};

/// Protocol version 1 between the viewer and its plugins.
#[derive(Debug, Clone, Copy)]
pub(super) struct Wire;

impl Protocol for Wire {
    type Capability = Capability;
    type Request = HostMessage;
    type Message = PluginMessage;
    const VERSION: u32 = PROTOCOL_VERSION;

    fn greeting(message: &PluginMessage) -> Option<Greeting<'_, Capability>> {
        match message {
            PluginMessage::Hello(hello) => Some(Greeting {
                protocol: hello.protocol,
                provides: &hello.provides,
            }),
            PluginMessage::Facts(_)
            | PluginMessage::Image(_)
            | PluginMessage::Progress(_)
            | PluginMessage::Done(_)
            | PluginMessage::Error(_) => None,
        }
    }
}
