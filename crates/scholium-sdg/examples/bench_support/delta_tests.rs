use super::*;

fn exchange<T: Backend>() {
    for batch in [1, 10, 100] {
        let mut a = T::new(1);
        for edit in bootstrap() {
            a.apply(&edit);
        }
        let mut b = T::new(2);
        b.import(&a.snapshot());
        let left = trace(0);
        let right = trace(1);
        for (left, right) in left[..100].chunks(batch).zip(right[..100].chunks(batch)) {
            let va = a.version();
            let vb = b.version();
            for edit in left {
                a.apply(edit);
            }
            for edit in right {
                b.apply(edit);
            }
            let ua = a.updates_since(&va);
            let ub = b.updates_since(&vb);
            a.import(&ub);
            b.import(&ua);
            assert_eq!(a.state(), b.state());
            a.import(&ub);
            assert_eq!(a.state(), b.state(), "duplicate delta is idempotent");
        }
        let mut restored = T::new(3);
        restored.import(&a.snapshot());
        assert_eq!(restored.state(), b.state());
    }
}

#[test]
fn incremental_deltas_preserve_unicode_tree_state_and_duplicate_import() {
    exchange::<loro::Loro>();
    exchange::<yrs::Yrs>();
}
