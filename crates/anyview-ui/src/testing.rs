//! The virtual clock the timed machines are tested on: no clock is read, the test steps the
//! machine with `Elapsed` exactly where it asked to be woken, as `use_machine` does.

use ds_core::machine::{Elapsed, Machine};
use ds_core::time::stamp::Stamp;

/// Steps `machine` with `Elapsed` at each `wake()` while it has one (at most `limit` times),
/// returning where it came to rest and every output with the stamp it came out at.
pub(crate) fn settle<M: Machine>(
    mut machine: M,
    params: &M::Params,
    limit: usize,
) -> (M, Vec<(Stamp, M::Out)>) {
    let mut log = Vec::new();
    for _ in 0..limit {
        let Some(at) = machine.wake() else { break };
        let (next, outs) = machine.step(M::In::from(Elapsed), at, params);
        log.extend(outs.into_iter().map(|out| (at, out)));
        machine = next;
    }
    (machine, log)
}

/// Golden files for rendered markup: `tests/snapshots/<name>`, compared as text, or rewritten under
/// `DS_BLESS=1` (read the diff before committing a bless: a golden rewritten to match is not
/// evidence of anything).
pub(crate) mod golden {
    use std::path::PathBuf;

    fn path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/snapshots")
            .join(name)
    }

    /// `Ok` when `actual` is the golden `name`; the difference otherwise.
    pub(crate) fn check(name: &str, actual: &str) -> Result<(), String> {
        let file = path(name);
        let actual = format!("{actual}\n");
        if std::env::var("DS_BLESS").is_ok_and(|value| value == "1") {
            if let Some(dir) = file.parent() {
                std::fs::create_dir_all(dir).map_err(|e| format!("{name}: {e}"))?;
            }
            return std::fs::write(&file, actual).map_err(|e| format!("{name}: {e}"));
        }
        match std::fs::read_to_string(&file) {
            Ok(expected) if expected == actual => Ok(()),
            Ok(expected) => Err(format!(
                "{name} differs\n  golden: {}\n  actual: {}",
                expected.trim_end(),
                actual.trim_end()
            )),
            Err(e) => Err(format!("{name}: {e} (DS_BLESS=1 writes it; then read it)")),
        }
    }
}
