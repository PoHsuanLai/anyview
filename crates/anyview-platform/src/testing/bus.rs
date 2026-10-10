//! A private session bus for the tests: a `dbus-daemon` with a configuration of its own, in a
//! scratch directory, with no service directories (so nothing on the machine is ever started
//! by activation) and no connection to the person's session.

use crate::{BusRoute, Env};
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};

const CONFIG: &str = r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:path=SOCKET</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"#;

/// A running private bus; the daemon stops when this is dropped.
#[derive(Debug)]
pub struct PrivateBus {
    daemon: Child,
    address: String,
    scratch: tempfile::TempDir,
}

impl PrivateBus {
    /// Start a bus, or `None` with a message on stderr when `dbus-daemon` is not installed.
    pub fn start() -> Option<PrivateBus> {
        let scratch = tempfile::tempdir().ok()?;
        let socket = scratch.path().join("bus");
        let config = scratch.path().join("bus.conf");
        std::fs::write(&config, CONFIG.replace("SOCKET", &socket.to_string_lossy())).ok()?;
        let spawned = Command::new("dbus-daemon")
            .arg("--nofork")
            .arg("--print-address")
            .arg(format!("--config-file={}", config.display()))
            .stdout(Stdio::piped())
            .spawn();
        let mut daemon = match spawned {
            Ok(daemon) => daemon,
            Err(error) => {
                eprintln!("SKIPPED: dbus-daemon cannot be started ({error}); no private bus");
                return None;
            }
        };
        let stdout = daemon.stdout.take()?;
        let mut address = String::new();
        BufReader::new(stdout).read_line(&mut address).ok()?;
        Some(PrivateBus {
            daemon,
            address: address.trim().to_owned(),
            scratch,
        })
    }

    /// An environment whose session bus is this one and whose directories are scratch.
    pub fn env(&self) -> Env {
        Env::isolated(self.scratch.path()).with_session(BusRoute::Address(self.address.clone()))
    }

    /// The bus address.
    pub fn address(&self) -> &str {
        &self.address
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}
