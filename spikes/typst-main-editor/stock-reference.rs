//! Independently construct complete stock Content, with no Scholium adapter or editor origins.
#[path = "../typst-edit-session/src/kernel.rs"]
#[allow(dead_code)]
mod kernel;
use typst::foundations::{Content, NativeElement};
use typst::layout::Abs;
use typst::math::{EquationElem, FracElem};
use typst::model::{EmphElem, ParElem, StrongElem};
use typst::text::{FontFamily, FontList, TextElem, TextSize};

fn fixture(name: &str) -> Content {
    let body = match name {
        "full" => Content::sequence([
            kernel::text("English 中文 空格"),
            EquationElem::new(
                FracElem::new(
                    kernel::text("12"),
                    FracElem::new(kernel::text("3"), kernel::text("4")).pack(),
                )
                .pack(),
            )
            .pack(),
            kernel::text(""),
        ]),
        "styles" => Content::sequence([
            StrongElem::new(kernel::text("中文 ")).pack(),
            EmphElem::new(kernel::text("English ")).pack(),
            kernel::text("#for [] ffi"),
        ]),
        "heading" => StrongElem::new(kernel::text("中文 Heading"))
            .pack()
            .set(TextElem::size, TextSize(Abs::pt(20.0).into())),
        _ => panic!("named static fixture"),
    };
    ParElem::new(body)
        .pack()
        .set(
            TextElem::font,
            FontList(vec![
                FontFamily::new("Times New Roman"),
                FontFamily::new("SimSun"),
            ]),
        )
        .set(TextElem::size, TextSize(Abs::pt(12.0).into()))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::path::PathBuf::from(std::env::args().nth(1).ok_or("output path required")?);
    std::fs::create_dir_all(&output)?;
    let world = kernel::ProbeWorld::new();
    for name in ["full", "styles", "heading"] {
        let frame = kernel::layout(&world, &fixture(name))?;
        let pixels = typst_render::render(&kernel::page(&frame), &kernel::render_options());
        std::fs::write(output.join(format!("{name}.rgba")), pixels.data())?;
        std::fs::write(
            output.join(format!("{name}.json")),
            serde_json::to_vec_pretty(
                &serde_json::json!({"width": pixels.width(), "height": pixels.height()}),
            )?,
        )?;
    }
    Ok(())
}
