use super::*;
use crate::editor::{content::ContentCache, kernel};
use scholium_model::{
    BlockKind, DocumentId, NodeId, Revision,
    structured::{InlineBody, StructuredBlock, StructuredDocument, StructuredInline, TextStyle},
};

const PAGE: [f64; 2] = [595.0, 1200.0];

fn spec(px_per_pt: f64) -> RasterSpec {
    RasterSpec {
        px_per_pt,
        max_side: 16384,
    }
}

#[test]
fn sharp_scale_is_only_limited_by_one_texture_width() {
    assert_eq!(spec(3.1).resolve(PAGE), 3.1);
    assert_eq!(spec(40.0).resolve([595.0, 90000.0]), 16384.0 / 595.0);
    let narrow = RasterSpec {
        px_per_pt: 8.0,
        max_side: 2048,
    };
    assert!((narrow.resolve(PAGE) * PAGE[0]).round() <= 2048.0);
}

#[test]
fn base_scale_is_at_most_one_and_stays_within_the_pixel_budget() {
    assert_eq!(spec(3.0).resolve_base(PAGE), 1.0);
    assert_eq!(spec(0.6).resolve_base(PAGE), 0.6);
    let huge = [20000.0, 20000.0];
    let resolved = spec(1.0).resolve_base(huge);
    let pixels = (resolved * huge[0]).round() * (resolved * huge[1]).round();
    assert!(pixels <= MAX_RASTER_PIXELS as f64 * 1.001, "{pixels}");
    assert!(resolved > 0.1, "capped scale, not nothing");
}

#[test]
fn base_strips_tile_the_page_exactly_within_the_texture_limit() {
    for (height, max) in [(1, 4096), (4096, 4096), (4097, 4096), (9000, 2048), (10, 3)] {
        let ranges = strip_ranges(height, max);
        assert!(ranges.iter().all(|&(_, rows)| rows >= 1 && rows <= max));
        let mut next = 0;
        for &(y, rows) in &ranges {
            assert_eq!(y, next);
            next = y + rows;
        }
        assert_eq!(next, height);
    }
}

#[test]
fn window_follows_the_viewport_with_one_viewport_of_margin() {
    // 3 px/pt: 1200 pt is 3600 rows = 8 strips (the last one partial).
    let height = 3600;
    assert_eq!(strip_count(height), 8);
    let at = |top, bottom| {
        strip_window(
            ViewRange {
                top_pt: top,
                bottom_pt: bottom,
            },
            3.0,
            height,
        )
    };
    // View 400..500 pt = rows 1200..1500 = strips 2 (rows 1024..1536) only.
    let window = at(400.0, 500.0);
    assert_eq!(window.visible, 2..3);
    // Margin of 100 pt each side: rows 900..1800 = strips 1..4.
    assert_eq!(window.keep, 1..4);
    // Clamped at both page ends.
    assert_eq!(at(0.0, 100.0).keep, 0..2);
    let end = at(1100.0, 1200.0);
    assert_eq!(end.visible, 6..8);
    assert_eq!(end.keep.end, 8);
    // A view that covers everything keeps everything; an off-page view keeps nothing visible.
    assert_eq!(at(0.0, 1.0e6).visible, 0..8);
    assert!(at(5000.0, 5100.0).visible.is_empty());
}

#[test]
fn scale_changes_the_strip_indices_for_the_same_view() {
    let view = ViewRange {
        top_pt: 400.0,
        bottom_pt: 500.0,
    };
    let at_two = strip_window(view, 2.0, 2400);
    let at_four = strip_window(view, 4.0, 4800);
    // 800..1000 rows at 2 px/pt is strip 1; 1600..2000 at 4 px/pt is strips 3 and 3 (1536..2048).
    assert_eq!(at_two.visible, 1..2);
    assert_eq!(at_four.visible, 3..4);
}

#[test]
fn render_order_is_visible_first_then_nearest() {
    let window = StripWindow {
        visible: 4..6,
        keep: 1..9,
    };
    assert_eq!(window.render_order(), vec![4, 5, 3, 6, 2, 7, 1, 8]);
}

fn paragraphs(count: usize) -> StructuredDocument {
    let sentence = "The quick brown fox jumps over the lazy dogs, 排版 stays crisp. ";
    StructuredDocument {
        document: DocumentId::fresh(),
        revision: Revision(1),
        blocks: (0..count)
            .map(|_| StructuredBlock {
                node: NodeId::fresh(),
                kind: BlockKind::Paragraph,
                content: vec![StructuredInline {
                    node: NodeId::fresh(),
                    body: InlineBody::Text {
                        text: sentence.repeat(8),
                        style: TextStyle::Plain,
                    },
                }],
            })
            .collect(),
    }
}

fn laid_page(count: usize) -> typst_layout::Page {
    let mut cache = ContentCache::default();
    let content = cache.document(&paragraphs(count)).expect("content");
    let frame = kernel::layout(&kernel::EditWorld::new(), &content).expect("layout");
    kernel::page(&frame)
}

fn max_difference(a: &[u8], b: &[u8]) -> u8 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .map(|(x, y)| x.abs_diff(*y))
        .max()
        .unwrap_or(0)
}

#[test]
fn every_strip_equals_the_matching_rows_of_a_full_render() {
    let page = laid_page(8);
    for px_per_pt in [2.0, 3.0003, 4.1] {
        let full = render_whole(&page, px_per_pt, 1 << 20);
        let rows = &full.strips[0];
        let row_bytes = full.width as usize * 4;
        let count = strip_count(full.height);
        assert!(count > 2, "document spans several strips");
        for index in 0..count {
            let strip = render_strip(&page, px_per_pt, index);
            let (y, height) = strip_rows(index, full.height);
            assert_eq!((strip.y, strip.height), (y, height));
            let expected = &rows.rgba[y as usize * row_bytes..(y + height) as usize * row_bytes];
            assert_eq!(strip.rgba.len(), expected.len());
            let worst = max_difference(&strip.rgba, expected);
            assert!(worst <= 1, "strip {index} @ {px_per_pt}: off by {worst}");
        }
    }
}

fn count_items(frame: &Frame) -> usize {
    frame
        .items()
        .map(|(_, item)| match item {
            FrameItem::Group(group) => count_items(&group.frame),
            _ => 1,
        })
        .sum()
}

#[test]
fn culling_drops_items_that_miss_the_strip_and_changes_no_pixel() {
    let page = laid_page(12);
    let all = count_items(&page.frame);
    let px_per_pt = 3.0;
    let height_pt = page.frame.height().to_pt();
    let top = height_pt * 0.5;
    let kept = cull(&page.frame, top, top + 512.0 / px_per_pt);
    let remaining = count_items(&kept);
    assert!(remaining * 4 < all, "kept {remaining} of {all}");
    // Rendering the whole page from the culled frame equals the original over that band.
    let strip = render_strip(&page, px_per_pt, (top * px_per_pt) as u32 / SHARP_STRIP_PX);
    let uncull = {
        let size = page.frame.size();
        let y = f64::from(strip.y) / px_per_pt;
        let mut frame = Frame::soft(Size::new(
            size.x,
            Abs::pt(f64::from(strip.height) / px_per_pt),
        ));
        frame.push_frame(Point::new(Abs::zero(), Abs::pt(-y)), page.frame.clone());
        typst_render::render(&kernel::sheet(frame), &kernel::render_options(px_per_pt))
    };
    assert_eq!(max_difference(&strip.rgba, uncull.data()), 0);
}

/// Timing probe, not a gate: `cargo test --release -p scholium-typst --features editor
/// -- --ignored --nocapture strip_cost`.
#[test]
#[ignore = "timing probe"]
fn strip_cost_follows_strip_area_not_page_length() {
    let median = |mut f: Vec<f64>| {
        f.sort_by(f64::total_cmp);
        f[f.len() / 2]
    };
    for (label, count) in [("1-page", 4), ("3-page", 16)] {
        let page = laid_page(count);
        let px_per_pt = 4.0;
        let middle = strip_count(page_px([595.0, page.frame.height().to_pt()], px_per_pt).1) / 2;
        let time = |f: &dyn Fn()| {
            median(
                (0..30)
                    .map(|_| {
                        let at = std::time::Instant::now();
                        f();
                        at.elapsed().as_secs_f64() * 1000.0
                    })
                    .collect(),
            )
        };
        let culled = time(&|| drop(render_strip(&page, px_per_pt, middle)));
        let unculled = time(&|| {
            let size = page.frame.size();
            let mut frame = Frame::soft(Size::new(size.x, Abs::pt(512.0 / px_per_pt)));
            let top = f64::from(middle * SHARP_STRIP_PX) / px_per_pt;
            frame.push_frame(Point::new(Abs::zero(), Abs::pt(-top)), page.frame.clone());
            drop(typst_render::render(
                &kernel::sheet(frame),
                &kernel::render_options(px_per_pt),
            ));
        });
        let whole = time(&|| drop(render_whole(&page, px_per_pt, 4096)));
        println!(
            "{label}: strip with culling {culled:.2} ms, without {unculled:.2} ms, whole page {whole:.2} ms, items {}",
            count_items(&page.frame)
        );
    }
}
