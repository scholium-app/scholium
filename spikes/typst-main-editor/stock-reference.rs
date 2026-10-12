//! Independently construct complete stock Content, with no Scholium adapter or editor origins.
#[path = "../typst-edit-session/src/kernel.rs"]
#[allow(dead_code)]
mod kernel;
use std::num::NonZeroUsize;
use std::str::FromStr;
use typst::foundations::{Content, Dict, FromValue, IntoValue, NativeElement, Smart, Value};
use typst::layout::{Abs, Em, Frame, Length, Point, Sides, Size};
use typst::math::{EquationElem, FracElem};
use typst::model::{EmphElem, FirstLineIndent, HeadingElem, ParElem, StrongElem};
use typst::text::{FontFamily, FontList, FontWeight, Lang, Region, TextElem, TextSize};

// Document style, duplicated on purpose so this path stays independent of the adapter.
// Keep in sync with crates/scholium-typst/src/style.rs.
const BODY_FONTS: [&str; 2] = ["Times New Roman", "SimSun"];
const HEADING_FONTS: [&str; 2] = ["Times New Roman", "SimHei"];
const BODY_SIZE_PT: f64 = 12.0;
const LEADING_EM: f64 = 1.0;
const FIRST_LINE_INDENT_EM: f64 = 2.0;
const PT_PER_CM: f64 = 72.0 / 2.54;
const MARGIN_X_PT: f64 = 2.5 * PT_PER_CM;
const MARGIN_Y_PT: f64 = 2.54 * PT_PER_CM;
const TEXT_WIDTH_PT: f64 = 21.0 * PT_PER_CM - 2.0 * MARGIN_X_PT;
const HEIGHT_PT: f64 = 2400.0;

fn fonts(names: &[&str]) -> FontList {
    FontList(names.iter().map(|name| FontFamily::new(name)).collect())
}

fn first_line_indent() -> FirstLineIndent {
    let mut dict = Dict::new();
    dict.insert(
        "amount".into(),
        Length::from(Em::new(FIRST_LINE_INDENT_EM)).into_value(),
    );
    dict.insert("all".into(), true.into_value());
    FirstLineIndent::from_value(Value::Dict(dict)).expect("fixed indent dictionary")
}

fn fixture(name: &str) -> Content {
    let paragraph = |body| ParElem::new(body).pack();
    let block = match name {
        "full" => paragraph(Content::sequence([
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
        ])),
        "styles" => paragraph(Content::sequence([
            StrongElem::new(kernel::text("中文 ")).pack(),
            EmphElem::new(kernel::text("English ")).pack(),
            kernel::text("#for [] ffi"),
        ])),
        "heading" => HeadingElem::new(kernel::text("中文 Heading"))
            .with_level(Smart::Custom(NonZeroUsize::MIN))
            .pack()
            .set(TextElem::font, fonts(&HEADING_FONTS))
            .set(TextElem::weight, FontWeight::BOLD),
        _ => panic!("named static fixture"),
    };
    block
        .set(TextElem::font, fonts(&BODY_FONTS))
        .set(TextElem::size, TextSize(Abs::pt(BODY_SIZE_PT).into()))
        .set(TextElem::lang, Lang::from_str("zh").expect("lang"))
        .set(
            TextElem::region,
            Some(Region::from_str("cn").expect("region")),
        )
        .set(ParElem::justify, true)
        .set(ParElem::leading, Em::new(LEADING_EM).into())
        .set(ParElem::first_line_indent, first_line_indent())
}

/// The editor sheet: the laid-out text column inside the A4 page margins.
fn sheet(frame: &Frame) -> typst_layout::Page {
    let margin = Point::new(Abs::pt(MARGIN_X_PT), Abs::pt(MARGIN_Y_PT));
    let mut padded = Frame::soft(frame.size() + Size::new(margin.x * 2.0, margin.y * 2.0));
    padded.push_frame(margin, frame.clone());
    typst_layout::Page {
        frame: padded,
        bleed: Sides::default(),
        fill: Smart::Auto,
        numbering: None,
        supplement: Content::empty(),
        number: 1,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::path::PathBuf::from(std::env::args().nth(1).ok_or("output path required")?);
    std::fs::create_dir_all(&output)?;
    let world = kernel::ProbeWorld::new();
    for name in ["full", "styles", "heading"] {
        let frame = kernel::layout_in(
            &world,
            &fixture(name),
            Size::new(Abs::pt(TEXT_WIDTH_PT), Abs::pt(HEIGHT_PT)),
        )?;
        let pixels = typst_render::render(&sheet(&frame), &kernel::render_options());
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
