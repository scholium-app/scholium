//! Visual verification of the extracted scene against Typst's original frame.

use std::error::Error;
use typst::layout::Point;
use typst_layout::PagedDocument;

pub(crate) fn run() -> Result<(), Box<dyn Error>> {
    let source = concat!(
        "#set page(width: 360pt, height: auto, margin: 20pt)\n",
        "#set text(font: \"Libertinus Serif\", size: 20pt)\n",
        "#show math.equation: set text(font: \"New Computer Modern Math\")\n",
        "Inline: $frac(a, b) x^2_1 mat(1, 2; 3, 4) sqrt(y)$\n\n",
        "$ frac(a, b) x^2_1 mat(1, 2; 3, 4) sqrt(y) $\n",
        "$ sqrt(frac(a + b, c)) + (frac(1, x^2)) $\n",
        "#rotate(17deg)[$frac(u, v) + sqrt(z)$]\n",
    );
    let world = crate::world::SpikeWorld::new(source.into());
    let compiled = typst::compile::<PagedDocument>(&world);
    if !compiled.warnings.is_empty() {
        return Err(format!("font/compile warnings: {:?}", compiled.warnings).into());
    }
    let document = compiled
        .output
        .map_err(|e| format!("compile errors: {e:?}"))?;
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/spikes/evidence/SPK-0027/scene");
    std::fs::create_dir_all(&output)?;
    std::fs::write(output.join("fixture.typ"), source)?;
    for (index, page) in document.pages().iter().enumerate() {
        let scene = crate::scene::Scene::from_frame(&page.frame, &world)?;
        let options = typst_render::RenderOptions {
            pixel_per_pt: 2.0.into(),
            render_bleed: false,
        };
        let reference = typst_render::render(page, &options);
        let mut rebuilt = page.clone();
        rebuilt.frame = scene.frame(&page.frame);
        let rendered = typst_render::render(&rebuilt, &options);
        reference.save_png(output.join(format!("reference-{index}.png")))?;
        rendered.save_png(output.join(format!("scene-{index}.png")))?;
        let changed = reference
            .pixels()
            .iter()
            .zip(rendered.pixels())
            .filter(|(a, b)| a != b)
            .count();
        println!(
            "page {}: {} ink boxes, {} changed pixels / {}",
            index + 1,
            scene.hits.len(),
            changed,
            reference.pixels().len()
        );
        assert_eq!(changed, 0, "scene must preserve the compiled picture");
        crate::scene_view::verify(&rendered, &scene);
        assert!(!scene.hits.is_empty());
        assert!(
            scene.hit(Point::zero()).is_none(),
            "margin must not select a nearby node"
        );
        let mut checked = 0;
        for expected in &scene.hits {
            assert!(!expected.text.is_empty());
            let Some(source_range) = expected.source.clone() else {
                continue;
            };
            let source_text = &source[source_range];
            if !matches!(source_text, "a" | "b" | "x" | "y" | "u" | "v" | "z") {
                continue;
            }
            let actual = scene.hit(expected.center()).ok_or("missing glyph hit")?;
            assert_eq!(actual.source, expected.source, "wrong source target");
            let range = actual.source.clone().ok_or("detached source span")?;
            assert_eq!(&source[range], source_text, "source text mismatch");
            checked += 1;
        }
        assert_eq!(checked, 14, "math targets missing");
        println!(
            "verified {checked} math ink hits against source; image: {}",
            output.display()
        );
    }
    Ok(())
}
