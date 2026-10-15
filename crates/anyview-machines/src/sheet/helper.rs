//! The install sheet's phases and how an install ended, as plain data the machine holds and the
//! view draws.

/// Where an install stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HelperPhase {
    /// The question: install it now, or not.
    Ask,
    /// The system is installing it, and may ask for the password itself.
    Installing,
    /// The install failed; the package manager's own words say why.
    Failed(String),
    /// The software sources have no package for it; the package to look for by hand.
    NotFound(String),
    /// This system cannot install it from here (no PackageKit, a sandbox, an unknown distribution);
    /// the program a package must provide, and the command that installs it by hand when the
    /// host can name the one for the person's system.
    Unsupported {
        /// The program a package must provide.
        program: String,
        /// The install command to copy, when there is one.
        command: Option<String>,
    },
}

/// How asking the system to install a tool ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HelperEnd {
    /// The tool is on the machine now.
    Installed,
    /// The person said no at the system's password prompt.
    Declined,
    /// The software sources have no package for it; the package to look for by hand.
    NotFound(String),
    /// This system cannot install it from here; the program a package must provide, and the
    /// command that installs it by hand when the host can name the one for the person's system.
    Unsupported {
        /// The program a package must provide.
        program: String,
        /// The install command to copy, when there is one.
        command: Option<String>,
    },
    /// The install failed; the package manager's own words say why.
    Failed(String),
}
