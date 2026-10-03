use super::workforce::lane_of;
use super::*;
use crate::runtime::Lane;
use crate::runtime::{JobOutcome, JobPanic, PoolSize};
use anyview_core::work::{Ticket, Ticketed};
use anyview_ui::WorkLane;
use std::num::NonZeroUsize;

#[test]
fn a_job_that_ran_to_its_end_has_no_notice_and_the_others_name_their_ticket() {
    let ended = vec![
        Ticketed::new(Ticket(1), JobOutcome::Done(())),
        Ticketed::new(Ticket(2), JobOutcome::Skipped),
        Ticketed::new(
            Ticket(3),
            JobOutcome::Panicked(JobPanic {
                message: "boom".to_owned(),
            }),
        ),
    ];
    assert_eq!(
        notices(ended),
        vec![
            Notice::Skipped(Ticket(2)),
            Notice::Panicked(Ticket(3), "boom".to_owned()),
        ]
    );
}

#[test]
fn a_workforce_starts_its_pool_and_joins_it_on_drop() {
    let waker = NoticeWaker::default();
    let size = PoolSize::exactly(NonZeroUsize::MIN);
    let workforce = Workforce::start(size, waker).unwrap();
    assert!(workforce.settled().is_empty(), "nothing ran, nothing ended");
    drop(workforce);
}

#[test]
fn work_is_run_in_the_lane_its_view_wanted_it_in() {
    // name, how soon the view wants it, the pool's lane
    const CASES: &[(&str, WorkLane, Lane)] = &[
        ("what is on screen", WorkLane::Visible, Lane::Visible),
        ("what is read ahead", WorkLane::Preload, Lane::Preload),
    ];
    for (name, wanted, lane) in CASES {
        assert_eq!(lane_of(*wanted), *lane, "{name}");
    }
}
