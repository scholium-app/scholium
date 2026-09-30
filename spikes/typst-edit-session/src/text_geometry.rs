//! Assert leaf offsets, shaping clusters, direction, transforms and cache reuse.

use std::path::Path;
use typst::foundations::NativeElement;
use typst::layout::{Abs, Frame, Point, Ratio, Size, Transform};
use unicode_segmentation::UnicodeSegmentation;

use crate::cases;
use crate::geometry::{Affinity, Geometry, Position};
use crate::kernel::{ProbeWorld, layout, save_frame};

pub(crate) fn run(directory: Option<&Path>) -> Result<(), String> {
    let mut world = ProbeWorld::new();
    world.library.editing = true;
    for name in cases::NAMES {
        let content = cases::paragraph(name, Some(900));
        let frame = layout(&world, &content)?;
        let geometry = Geometry::from_frame(&frame).map_err(|e| e.to_string())?;
        check_boundaries(name, &geometry);
        let repeat = layout(&world, &content)?;
        assert_eq!(
            typst::utils::hash128(&frame),
            typst::utils::hash128(&repeat)
        );
        let other = layout(&world, &cases::paragraph(name, Some(901)))?;
        assert!(
            Geometry::from_frame(&other)
                .map_err(|e| e.to_string())?
                .carets
                .iter()
                .all(|c| c.position.leaf == 901)
        );
        check_transform(&frame, &geometry)?;
        if let Some(directory) = directory {
            save_frame(&frame, &directory.join(format!("fork-{name}.png")))?;
        }
        println!(
            "PASS text_{name} carets={} fallback={}",
            geometry.carets.len(),
            geometry.carets.iter().filter(|caret| !caret.exact).count()
        );
    }
    reject_derived_mapping(&world)?;
    Ok(())
}

fn check_boundaries(name: &str, geometry: &Geometry) {
    for bound in &geometry.bounds {
        assert_eq!(bound.origin.node, 900);
        assert!(!bound.decoration);
        assert!(
            bound
                .corners
                .iter()
                .flatten()
                .all(|value| value.is_finite())
        );
    }
    let text = cases::value(name);
    let valid: Vec<_> = text
        .grapheme_indices(true)
        .map(|(byte, _)| byte)
        .chain(std::iter::once(text.len()))
        .collect();
    assert!(!geometry.carets.is_empty(), "no carets for {name}");
    for caret in &geometry.carets {
        assert_eq!(caret.position.leaf, 900);
        assert!(valid.contains(&caret.position.byte), "{name}: {caret:?}");
        assert!(caret.bottom[1] > caret.top[1]);
        let center = [caret.top[0], (caret.top[1] + caret.bottom[1]) / 2.0];
        let hit = geometry.hit(center).expect("nonempty geometry");
        assert!((hit.top[0] - caret.top[0]).abs() < 1e-6);
        // Adjacent font runs can share an edge with different ascent/descent.
        assert!(center[1] >= hit.top[1] - 1e-6 && center[1] <= hit.bottom[1] + 1e-6);
    }
    for byte in valid {
        let position = Position {
            leaf: 900,
            byte,
            affinity: Affinity::Downstream,
        };
        assert!(
            geometry.caret(position).is_some(),
            "missing {name} byte {byte}"
        );
    }
    if name == "ligature" {
        assert!(
            geometry.carets.iter().any(|caret| !caret.exact),
            "fixture must exercise ligatures"
        );
    }
    check_direction(name, geometry, text.len());
}

fn check_direction(name: &str, geometry: &Geometry, length: usize) {
    if name == "rtl" {
        let start = geometry
            .caret(Position {
                leaf: 900,
                byte: 0,
                affinity: Affinity::Downstream,
            })
            .unwrap();
        let end = geometry
            .caret(Position {
                leaf: 900,
                byte: length,
                affinity: Affinity::Upstream,
            })
            .unwrap();
        assert!(start.top[0] > end.top[0]);
    }
}

fn check_transform(frame: &Frame, original: &Geometry) -> Result<(), String> {
    let mut moved = Frame::soft(frame.size() + Size::splat(Abs::pt(100.0)));
    moved.push_frame(Point::new(Abs::pt(30.0), Abs::pt(70.0)), frame.clone());
    moved.transform(Transform::scale(Ratio::new(1.5), Ratio::new(1.5)));
    let after = Geometry::from_frame(&moved).map_err(|e| e.to_string())?;
    assert_eq!(original.carets.len(), after.carets.len());
    for (before, after) in original.carets.iter().zip(after.carets.iter()) {
        assert_eq!(before.position, after.position);
        for axis in 0..2 {
            let offset = if axis == 0 { 30.0 } else { 70.0 };
            assert!((after.top[axis] - (before.top[axis] + offset) * 1.5).abs() < 1e-8);
            assert!((after.bottom[axis] - (before.bottom[axis] + offset) * 1.5).abs() < 1e-8);
        }
    }
    Ok(())
}

fn reject_derived_mapping(world: &ProbeWorld) -> Result<(), String> {
    use typst::text::{Case, TextElem};
    let content =
        cases::paragraph("ligature", Some(950)).styled(TextElem::case.set(Some(Case::Upper)));
    let frame = layout(world, &content)?;
    let geometry = Geometry::from_frame(&frame).map_err(|e| e.to_string())?;
    assert!(
        geometry.carets.is_empty(),
        "case-transformed text must not receive guessed writable offsets"
    );
    let origin = |node| typst::editor::EditOrigin {
        node,
        slot: None,
        hole: false,
    };
    let body = crate::kernel::text("of").with_edit_origin(origin(951))
        + crate::kernel::text("fice").with_edit_origin(origin(952));
    let frame = layout(world, &typst::model::ParElem::new(body).pack())?;
    let geometry = Geometry::from_frame(&frame).map_err(|e| e.to_string())?;
    // The ffi cluster straddles two leaves. Do not invent per-leaf subranges.
    assert!(
        geometry
            .carets
            .iter()
            .all(|c| c.position.leaf != 951 || c.position.byte < 2)
    );
    println!("PASS derived_case_and_cross_leaf_cluster_mapping_refused");
    Ok(())
}
