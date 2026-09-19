//! Stage-0 coordinator and native-window adapter; no network transport.
mod coordinator;
mod crdt;
mod error;
mod fixtures;
mod harness;
mod model;
mod scenarios;
mod timeline;
mod validate;
mod wal;
pub mod window;

/// Run the original positive, negative and broken-policy control fixtures.
pub fn run_validation() -> bool {
    let mut harness = harness::Harness::new();
    scenarios::c1_same_language::run(&mut harness);
    scenarios::c2_barrier::run(&mut harness);
    scenarios::c3_epoch::run(&mut harness);
    scenarios::c4_restart::run(&mut harness);
    scenarios::c5_partition::run(&mut harness);
    scenarios::c6_stale::run(&mut harness);
    scenarios::c7_malicious::run(&mut harness);
    harness.finish()
}
