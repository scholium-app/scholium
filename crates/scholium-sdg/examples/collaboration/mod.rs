use crate::bench_support::{Backend, Edit, State, bootstrap, trace};
use serde_json::json;
use std::time::Instant;
mod workload;

pub fn run<T: Backend>(engine: &str, mode: &str, workload: &str, count: usize, batch: usize) {
    assert!(count > 0 && batch > 0);
    let (mut a, mut b, left, right, expected) = prepare::<T>(workload, count);
    let mut receives = Vec::new();
    let mut imports = Vec::new();
    let mut indexes = Vec::new();
    let mut bytes = 0;
    let batch = if mode == "offline" {
        count
    } else {
        assert_eq!(mode, "incremental");
        batch
    };
    let start = Instant::now();
    for (left, right) in left.chunks(batch).zip(right.chunks(batch)) {
        let va = a.version();
        let vb = b.version();
        for edit in left {
            a.apply(edit);
        }
        for edit in right {
            b.apply(edit);
        }
        // Encode both before either import; each replica has diverged concurrently.
        let ua = a.updates_since(&vb);
        let ub = b.updates_since(&va);
        bytes += ua.len() + ub.len();
        for (doc, update) in [(&mut a, ub), (&mut b, ua)] {
            let start = Instant::now();
            doc.import(&update);
            receives.push(start.elapsed().as_secs_f64() * 1000.0);
            let (import, index) = doc.last_import_ms();
            imports.push(import);
            indexes.push(index);
        }
    }
    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(a.state(), b.state(), "convergence");
    assert_eq!(a.state(), expected, "serial identified-state oracle");
    let snapshot = a.snapshot();
    let mut restored = T::new(3);
    restored.import(&snapshot);
    assert_eq!(restored.state(), expected, "snapshot roundtrip");
    println!(
        "{}",
        json!({
            "engine":engine,"mode":mode,"workload":workload,"actions_per_side":count,
            "batch_actions":batch,"receive_ms":receives,"engine_import_ms":imports,
            "index_rebuild_ms":indexes,"update_bytes_total":bytes,"snapshot_bytes":snapshot.len(),
            "elapsed_ms":elapsed_ms,"converged":true,
        })
    );
}

type Prepared<T> = (T, T, Vec<Edit>, Vec<Edit>, State);
fn prepare<T: Backend>(workload: &str, count: usize) -> Prepared<T> {
    let mut a = T::new(1);
    for edit in bootstrap() {
        a.apply(&edit);
    }
    for peer in 0..2 {
        for edit in workload::setup(workload, peer, count) {
            a.apply(&edit);
        }
    }
    let mut b = T::new(2);
    b.import(&a.snapshot());
    let left = workload::edits(workload, 0, count);
    let right = workload::edits(workload, 1, count);
    // A serial execution is valid for this disjoint trace, and catches lost edits.
    let mut oracle = T::new(4);
    oracle.import(&a.snapshot());
    for edit in left.iter().chain(&right) {
        oracle.apply(edit);
    }
    let expected = oracle.state();
    (a, b, left, right, expected)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_workload_converges_to_its_serial_oracle() {
        for workload in workload::NAMES {
            run::<crate::bench_support::loro::Loro>("loro", "offline", workload, 20, 20);
            run::<crate::bench_support::yrs::Yrs>("yrs", "offline", workload, 20, 20);
        }
    }
}
