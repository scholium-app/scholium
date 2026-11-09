use super::*;
use serde_json::json;

fn layout(x: i64) -> serde_json::Value {
    json!({"pages_total":1,"pages_changed":[{"page":1,"exact":true,"dl":{"page_width":4736287,"page_height":4736287,"lines":[{"x":655360,"y":1310720,"items":[["r",x,655360,655360,655360]]},{"x":2621440,"y":1310720,"items":[["r",2621440,655360,655360,655360]]}]}}],"placements":[{"par_id":1,"fragments":[{"page":1,"first_line":1,"last_line":1,"xs":[655360],"baselines":[1310720]}]}]})
}
fn paragraph(bad: bool) -> Drawing {
    Drawing::decode(
        &json!({"dl":{"lines":[{"x":0,"y":0,"items":[["r",0,-655360,1310720,655360]]}]},"fragments":[{"page":1,"first_line":1,"last_line":if bad {2}else{1},"xs":[655360],"baselines":[1310720]}]}),
        None,
    )
}
#[test]
fn paragraph_replaces_only_owned_column_and_failed_update_is_transactional() {
    let mut renderer = Renderer::default();
    let initial = Drawing::decode(&layout(655360), None);
    renderer.apply(&initial, None).expect("initial");
    assert!(renderer.apply(&paragraph(true), Some(1)).is_err());
    let after = renderer
        .apply(&paragraph(false), Some(1))
        .expect("live update");
    assert_eq!(
        after[0].rgba[(55 * after[0].width as usize + 180) * 4],
        0,
        "other column survives"
    );
    assert!(after[0].rgba.as_chunks::<4>().0.iter().any(|p| p[0] == 0));
}
#[test]
fn coalesced_layout_retains_unchanged_page_payloads() {
    let first = Drawing::decode(&layout(655360), None);
    let next = Drawing::decode(
        &json!({"pages_total":1,"pages_changed":[],"placements":[]}),
        None,
    )
    .complete(&first);
    assert_eq!(
        Renderer::default()
            .apply(&next, None)
            .expect("complete")
            .len(),
        1
    );
}
#[test]
fn display_list_matches_independent_reference_png_with_e4_threshold() {
    let drawing = Drawing::decode(&layout(655360), None);
    let page = Renderer::default()
        .apply(&drawing, None)
        .expect("drawing")
        .remove(0);
    let reference = tiny_skia::Pixmap::decode_png(include_bytes!("../../tests/fixtures/rules.png"))
        .expect("reference PNG");
    let a = crate::tests::pixels::gray(&page);
    let b =
        crate::tests::pixels::gray_bytes(reference.width(), reference.height(), reference.data());
    let diff = crate::tests::pixels::compare(&a, &b);
    assert!(diff.unmatched_fraction <= 0.01, "{diff:?}");
    assert_eq!(page.origin, RasterOrigin::DisplayList);
}
#[test]
fn excessive_page_and_pdf_dimensions_fail_before_allocation() {
    let mut value = layout(655360);
    value["pages_changed"][0]["dl"]["page_width"] = json!(i64::MAX);
    assert!(
        Renderer::default()
            .apply(&Drawing::decode(&value, None), None)
            .is_err()
    );
    assert!(render_pdf(b"not a PDF").is_err());
}

#[test]
fn math_boundaries_are_metadata_but_unknown_drawing_requires_pdf() {
    let mut value = layout(655360);
    value["pages_changed"][0]["dl"]["lines"][0]["items"]
        .as_array_mut()
        .expect("items")
        .push(json!(["m", "on", 0]));
    assert_eq!(
        Renderer::default()
            .apply(&Drawing::decode(&value, None), None)
            .expect("math metadata")[0]
            .origin,
        RasterOrigin::DisplayList
    );
    value["pages_changed"][0]["dl"]["lines"][0]["items"]
        .as_array_mut()
        .expect("items")
        .push(json!(["unknown_ink"]));
    assert!(
        Renderer::default()
            .apply(&Drawing::decode(&value, None), None)
            .is_err()
    );
}

#[test]
fn hundred_page_vector_scene_rasterizes_only_the_requested_page_within_budget() {
    let mut value = layout(655360);
    let mut page = value["pages_changed"][0].clone();
    // A4 in TeX scaled points; 100 pages cannot fit the all-page raster budget.
    page["dl"]["page_width"] = json!(39_158_276);
    page["dl"]["page_height"] = json!(55_380_990);
    value["pages_total"] = json!(100);
    value["pages_changed"] = (1..=100)
        .map(|n| {
            let mut next = page.clone();
            next["page"] = json!(n);
            next
        })
        .collect();
    let drawing = Drawing::decode(&value, None);
    assert!(Renderer::default().apply(&drawing, None).is_err());
    let mut renderer = Renderer::default();
    for selected in [1, 100, 50] {
        let pages = renderer
            .apply_page(&drawing, None, selected)
            .expect("visible page");
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].page, selected);
        assert_eq!(pages[0].origin, RasterOrigin::DisplayList);
    }
}
