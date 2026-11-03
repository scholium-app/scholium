//! Reproduce: cargo run --release -p scholium-sdg --example engine_bench -- loro|yrs
mod bench_support;
use bench_support::{ACTIONS, Backend, bootstrap, trace};
use serde_json::json;
use std::time::Instant;
fn run<T: Backend>(name: &str) {
    let rss_before = memory("VmRSS");
    let mut a = T::new(1);
    for edit in bootstrap() {
        a.apply(&edit);
    }
    let mut expected = a.state();
    // Each paragraph's text is moved out and appended back after its inline math.
    for p in 0..bench_support::PARAGRAPHS {
        expected
            .get_mut(&format!("p{p}"))
            .expect("paragraph")
            .3
            .reverse();
    }
    let mut b = T::new(2);
    b.import(&a.snapshot());
    let left = trace(0);
    let right = trace(1);
    assert_eq!(left.len() + right.len(), ACTIONS);
    let start = Instant::now();
    for edit in &left {
        a.apply(edit);
    }
    for edit in &right {
        b.apply(edit);
    }
    let edit_ms = start.elapsed().as_secs_f64() * 1000.0;
    let rss_after_edit = memory("VmRSS");
    let start = Instant::now();
    let ua = a.updates();
    let ub = b.updates();
    let update_export_ms = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    a.import(&ub);
    b.import(&ua);
    let merge_ms = start.elapsed().as_secs_f64() * 1000.0;
    let import_ms = a.last_import_ms().0 + b.last_import_ms().0;
    let index_ms = a.last_import_ms().1 + b.last_import_ms().1;
    let state = a.state();
    assert_eq!(state, b.state(), "two-replica convergence");
    assert_eq!(
        state, expected,
        "trace must restore the entire identified baseline"
    );
    let start = Instant::now();
    let snapshot = a.snapshot();
    let snapshot_export_ms = start.elapsed().as_secs_f64() * 1000.0;
    let mut restored = T::new(3);
    let start = Instant::now();
    restored.import(&snapshot);
    let snapshot_import_ms = start.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(state, restored.state(), "snapshot roundtrip");
    println!(
        "{}",
        json!({"engine":name,"actions":ACTIONS,"edit_ms":edit_ms,"actions_per_second":ACTIONS as f64/(edit_ms/1000.0),"update_bytes_total":ua.len()+ub.len(),"update_export_ms":update_export_ms,"snapshot_bytes":snapshot.len(),"snapshot_export_ms":snapshot_export_ms,"snapshot_import_ms":snapshot_import_ms,"two_way_merge_ms":merge_ms,"two_way_engine_import_ms":import_ms,"two_way_index_rebuild_ms":index_ms,"rss_before_kib":rss_before,"rss_after_edit_kib":rss_after_edit,"peak_rss_kib":memory("VmHWM"),"visible_nodes":state.len(),"converged":true})
    );
}
fn memory(field: &str) -> usize {
    std::fs::read_to_string("/proc/self/status")
        .unwrap_or_default()
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{field}:")))
        .and_then(|l| l.split_whitespace().next())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}
fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("loro") => run::<bench_support::loro::Loro>("loro-1.16.0"),
        Some("yrs") => run::<bench_support::yrs::Yrs>("yrs-0.27.4"),
        _ => panic!("expected loro or yrs"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bench_support::{Edit, State};

    fn exercise<T: Backend>() -> State {
        let mut doc = T::new(1);
        for edit in bootstrap() {
            doc.apply(&edit);
        }
        let original = doc.state();
        doc.apply(&Edit::Insert("t0".into()));
        assert_eq!(doc.state()["t0"].2, "éhello 中文 👩‍🔬");
        doc.apply(&Edit::Remove("t0".into()));
        assert_eq!(doc.state(), original);
        doc.apply(&Edit::Create {
            id: "wrapper".into(),
            parent: "r0".into(),
            kind: "root",
            text: "",
        });
        doc.apply(&Edit::Move {
            id: "x0".into(),
            parent: "wrapper".into(),
        });
        assert_eq!(doc.state()["x0"].0, "wrapper");
        doc.apply(&Edit::Move {
            id: "x0".into(),
            parent: "r0".into(),
        });
        doc.apply(&Edit::Delete("wrapper".into()));
        assert_eq!(doc.state(), original);
        let mut restored = T::new(2);
        restored.import(&doc.snapshot());
        assert_eq!(restored.state(), original);
        original
    }

    #[test]
    fn both_engines_execute_the_same_unicode_wrap_move_and_delete_semantics() {
        assert_eq!(
            exercise::<bench_support::loro::Loro>(),
            exercise::<bench_support::yrs::Yrs>()
        );
    }

    #[test]
    fn the_trace_contains_one_hundred_thousand_actions_on_disjoint_paragraphs() {
        assert_eq!(bootstrap().len(), 160);
        assert_eq!(trace(0).len() + trace(1).len(), ACTIONS);
    }
}
