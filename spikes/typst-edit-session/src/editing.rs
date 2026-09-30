//! Kernel assertions: slot identity, invisible holes, cache isolation and transforms.

use std::path::Path;
use std::sync::atomic::Ordering;

use typst::editor::{EditHoleElem, EditOrigin};
use typst::foundations::{Content, NativeElement};
use typst::layout::{Abs, Frame, FrameItem, Point, Ratio, Size, Transform};
use typst::math::FracElem;

use super::{Counts, ProbeWorld, count, fraction_paragraph, layout, save_frame, text};

#[derive(Debug, Clone)]
struct Placed {
    origin: EditOrigin,
    min: [f64; 2],
    max: [f64; 2],
}

impl Placed {
    fn area(&self) -> f64 {
        (self.max[0] - self.min[0]) * (self.max[1] - self.min[1])
    }

    fn center(&self) -> [f64; 2] {
        [
            (self.min[0] + self.max[0]) / 2.0,
            (self.min[1] + self.max[1]) / 2.0,
        ]
    }

    fn contains(&self, point: [f64; 2]) -> bool {
        (0..2).all(|axis| self.min[axis] <= point[axis] && point[axis] <= self.max[axis])
    }
}

fn origin(node: u128, slot: Option<u128>, hole: bool) -> EditOrigin {
    EditOrigin { node, slot, hole }
}

fn slot(node: u128, id: u128, value: Option<&str>) -> Content {
    match value {
        Some(value) => text(value).with_edit_origin(origin(node, Some(id), false)),
        None => EditHoleElem::new()
            .pack()
            .with_edit_origin(origin(node, Some(id), true)),
    }
}

fn formula(node: u128, value: Option<&str>) -> Content {
    FracElem::new(slot(node, 11, Some("x")), slot(node, 12, value))
        .pack()
        .with_edit_origin(origin(node, None, false))
}

fn fixture(node: u128, value: Option<&str>) -> Content {
    fraction_paragraph(formula(node, value), "结束。")
}

fn collect(frame: &Frame, transform: Transform, output: &mut Vec<Placed>) {
    // Compose frame-local coordinates into root coordinates, measured in points.
    for bounds in frame.edit_bounds() {
        let start = bounds.position;
        let end = start + bounds.size.to_point();
        let corners = [
            start,
            Point::new(start.x, end.y),
            end,
            Point::new(end.x, start.y),
        ];
        let transformed = corners.map(|point| point.transform(transform));
        output.push(Placed {
            origin: bounds.origin,
            min: [
                transformed
                    .iter()
                    .map(|p| p.x.to_pt())
                    .fold(f64::INFINITY, f64::min),
                transformed
                    .iter()
                    .map(|p| p.y.to_pt())
                    .fold(f64::INFINITY, f64::min),
            ],
            max: [
                transformed
                    .iter()
                    .map(|p| p.x.to_pt())
                    .fold(f64::NEG_INFINITY, f64::max),
                transformed
                    .iter()
                    .map(|p| p.y.to_pt())
                    .fold(f64::NEG_INFINITY, f64::max),
            ],
        });
    }
    for (point, item) in frame.items() {
        if let FrameItem::Group(group) = item {
            assert!(
                group.clip.is_none(),
                "clipped hit geometry is outside this probe"
            );
            let transform = transform
                .pre_concat(Transform::translate(point.x, point.y))
                .pre_concat(group.transform);
            collect(&group.frame, transform, output);
        }
    }
}

fn bounds(frame: &Frame) -> Vec<Placed> {
    let mut result = Vec::new();
    collect(frame, Transform::identity(), &mut result);
    result
}

fn find(geometry: &[Placed], node: u128, slot: Option<u128>) -> &Placed {
    geometry
        .iter()
        .find(|item| item.origin.node == node && item.origin.slot == slot)
        .unwrap_or_else(|| panic!("missing geometry node={node} slot={slot:?}: {geometry:?}"))
}

fn hit(geometry: &[Placed], point: [f64; 2]) -> Option<EditOrigin> {
    geometry
        .iter()
        .filter(|item| item.origin.slot.is_some() && item.contains(point))
        .min_by(|a, b| a.area().total_cmp(&b.area()))
        .map(|item| item.origin)
}

fn check_hole(world: &ProbeWorld) -> Result<Frame, String> {
    let frame = layout(world, &fixture(100, None))?;
    let geometry = bounds(&frame);
    let numerator = find(&geometry, 100, Some(11));
    let denominator = find(&geometry, 100, Some(12));
    let parent = find(&geometry, 100, None);
    assert!(denominator.area() > 0.0 && numerator.area() > 0.0);
    assert!(denominator.origin.hole);
    assert!(denominator.center()[1] > numerator.center()[1]);
    assert!(parent.contains(numerator.center()) && parent.contains(denominator.center()));
    assert_eq!(
        hit(&geometry, denominator.center()),
        Some(denominator.origin)
    );
    let mut counts = Counts::default();
    count(&frame, &mut counts);
    assert_eq!(counts.shapes, 1, "only the fraction bar is printed");
    println!(
        "PASS empty_slot_hit geometry={geometry:?} glyphs={} shapes={}",
        counts.glyphs, counts.shapes
    );
    Ok(frame)
}

fn check_cache(world: &ProbeWorld, empty: &Frame) -> Result<Frame, String> {
    let filled = layout(world, &fixture(100, Some("2")))?;
    let geometry = bounds(&filled);
    assert!(!find(&geometry, 100, Some(12)).origin.hole);
    let mut before = Counts::default();
    let mut after = Counts::default();
    count(empty, &mut before);
    count(&filled, &mut after);
    assert_eq!(after.glyphs, before.glyphs + 1);
    assert_eq!(after.shapes, before.shapes);
    let repeat = layout(world, &fixture(100, Some("2")))?;
    assert_eq!(
        typst::utils::hash128(&filled),
        typst::utils::hash128(&repeat)
    );
    let other_content = fixture(200, Some("2"));
    assert_ne!(fixture(100, Some("2")), other_content);
    let other = layout(world, &other_content)?;
    let other_geometry = bounds(&other);
    assert!(other_geometry.iter().all(|item| item.origin.node == 200));
    for slot in [None, Some(11), Some(12)] {
        assert_eq!(
            find(&geometry, 100, slot).min,
            find(&other_geometry, 200, slot).min
        );
        assert_eq!(
            find(&geometry, 100, slot).max,
            find(&other_geometry, 200, slot).max
        );
    }
    let changed = layout(world, &fixture(100, Some("22")))?;
    let changed_geometry = bounds(&changed);
    assert!(find(&changed_geometry, 100, Some(12)).area() > find(&geometry, 100, Some(12)).area());
    assert!(find(&changed_geometry, 100, Some(11)).area() > 0.0);
    println!("PASS fill_slot_repeat_cache_different_ids_and_changed_denominator");
    Ok(filled)
}

fn check_transforms(frame: &Frame) {
    let before = bounds(frame);
    let offset = Point::new(Abs::pt(30.0), Abs::pt(70.0));
    let mut shifted = Frame::soft(frame.size() + Size::splat(Abs::pt(100.0)));
    shifted.push_frame(offset, frame.clone());
    shifted.transform(Transform::scale(Ratio::new(2.0), Ratio::new(2.0)));
    let after = bounds(&shifted);
    for item in before {
        let moved = find(&after, item.origin.node, item.origin.slot);
        for axis in 0..2 {
            let shift = if axis == 0 { 30.0 } else { 70.0 };
            assert!((moved.min[axis] - (item.min[axis] + shift) * 2.0).abs() < 1e-8);
            assert!((moved.max[axis] - (item.max[axis] + shift) * 2.0).abs() < 1e-8);
        }
    }
    println!("PASS nested_group_translation_and_scale");
}

fn nested_fixture() -> Content {
    let inner = formula(300, Some("2"));
    let outer = FracElem::new(inner, slot(400, 12, Some("3")))
        .pack()
        .with_edit_origin(origin(400, None, false));
    fraction_paragraph(outer, "结束。")
}

pub(super) fn run(mut world: ProbeWorld, directory: Option<&Path>) -> Result<(), String> {
    // Changing the hashed Library mode also invalidates mode-dependent memoized work.
    world.library.editing = true;
    let empty = check_hole(&world)?;
    let filled = check_cache(&world, &empty)?;
    check_transforms(&empty);
    let nested = layout(&world, &nested_fixture())?;
    let nested_geometry = bounds(&nested);
    assert!(find(&nested_geometry, 300, Some(11)).area() > 0.0);
    assert!(find(&nested_geometry, 300, Some(12)).area() > 0.0);
    assert!(find(&nested_geometry, 400, None).contains(find(&nested_geometry, 300, None).center()));
    println!("PASS nested_fraction_ownership");
    world.library.editing = false;
    let strict_error = layout(&world, &fixture(100, None)).expect_err("strict mode rejects holes");
    assert!(strict_error.contains("unfilled structural math slot"));
    layout(&world, &fixture(100, Some("2")))?;
    println!("PASS strict_mode_rejects_holes_and_accepts_complete_content");
    if let Some(directory) = directory {
        std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        save_frame(&empty, &directory.join("empty-slot.png"))?;
        save_frame(&filled, &directory.join("fork-full.png"))?;
        save_frame(&nested, &directory.join("fork-nested.png"))?;
    }
    assert_eq!(world.source_reads.load(Ordering::Relaxed), 0);
    println!("PASS K1_kernel_subset source_reads=0");
    Ok(())
}
