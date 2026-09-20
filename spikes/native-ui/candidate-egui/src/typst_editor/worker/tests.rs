use super::*;

fn fixture() -> (projection::Projection, serde_json::Value) {
    let mut editor = scholium_spike_core::Editor::new();
    scholium_spike_core::fixture::build_text(&mut editor, "a");
    let projection = projection::Projection::new(editor.document());
    let span = projection
        .spans
        .iter()
        .find(|s| s.text == "a")
        .expect("text span");
    let cell = serde_json::json!({
        "start": span.start, "end": span.end, "shape": false,
        "ink": [0, 0, 10, 10], "x": 0, "y": 10, "size": 10,
        "advance": 10, "offset": 0, "length": 1, "text": "a"
    });
    (projection, cell)
}

#[test]
fn rejects_overflowing_or_invalid_text_ranges() {
    let (projection, good) = fixture();
    assert!(read_cell(&good, &projection).expect("valid cell").is_some());
    for (offset, length) in [(u64::MAX, 2), (0, u64::MAX), (1, 1)] {
        let mut cell = good.clone();
        cell["offset"] = offset.into();
        cell["length"] = length.into();
        assert!(
            read_cell(&cell, &projection).is_err(),
            "accepted {offset}+{length}"
        );
    }
}

#[test]
fn rejects_non_finite_geometry_after_narrowing() {
    let (projection, good) = fixture();
    for key in ["x", "y", "size", "advance"] {
        let mut cell = good.clone();
        cell[key] = serde_json::json!(1e100);
        assert!(read_cell(&cell, &projection).is_err(), "accepted {key}");
    }
    let mut cell = good;
    cell["start"] = cell["end"].as_u64().expect("end").saturating_add(1).into();
    assert!(read_cell(&cell, &projection).is_err());
}

#[test]
fn transformed_math_scalar_keeps_original_byte_range() {
    let cell = serde_json::json!({"offset": 0, "length": 4, "text": "\u{1d44e}"});
    assert_eq!(text_range(&cell, "a").expect("styled math letter"), (0, 1));
    assert!(text_range(&cell, "").is_err());
    assert!(text_range(&cell, "ab").is_err());
}
