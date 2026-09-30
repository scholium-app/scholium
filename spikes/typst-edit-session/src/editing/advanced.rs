//! Composite slot identity and explicit fraction-rule ownership assertions.

use super::*;

pub(super) fn run(world: &ProbeWorld, nested: &Frame) -> Result<(), String> {
    let geometry = bounds(nested);
    let inner = find(&geometry, 300, None);
    let outer_slot = find(&geometry, 400, Some(11));
    assert_eq!(inner.min, outer_slot.min);
    assert_eq!(inner.max, outer_slot.max);
    let rules: Vec<_> = geometry.iter().filter(|item| item.decoration).collect();
    assert_eq!(rules.len(), 2);
    for rule in &rules {
        assert!(rule.area() > 0.0 && rule.origin.slot.is_none());
        assert!(find(&geometry, rule.origin.node, None).contains(rule.center()));
    }
    assert!(rules.iter().any(|item| item.origin.node == 300));
    assert!(rules.iter().any(|item| item.origin.node == 400));
    check_slot_cache(world)?;
    println!("PASS composite_nested_slot_and_explicit_rule_owners");
    Ok(())
}

fn check_slot_cache(world: &ProbeWorld) -> Result<(), String> {
    let content = formula(500, Some("2"));
    let in_slot = content.clone().with_edit_slot(origin(600, Some(11), false));
    let other = content.clone().with_edit_slot(origin(700, Some(11), false));
    assert_ne!(in_slot, other);
    let first = layout(world, &fraction_paragraph(in_slot, "结束。"))?;
    let second = layout(world, &fraction_paragraph(other, "结束。"))?;
    let first = bounds(&first);
    let second = bounds(&second);
    assert_eq!(find(&first, 500, None).min, find(&first, 600, Some(11)).min);
    assert_eq!(
        find(&first, 600, Some(11)).min,
        find(&second, 700, Some(11)).min
    );
    assert!(!second.iter().any(|item| item.origin.node == 600));
    let row = (text("x") + text("2")).with_edit_slot(origin(800, Some(12), false));
    let frame = layout(
        world,
        &fraction_paragraph(FracElem::new(text("x"), row).pack(), "结束。"),
    )?;
    assert!(find(&bounds(&frame), 800, Some(12)).area() > 0.0);
    Ok(())
}
