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
