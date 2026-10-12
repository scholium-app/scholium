//! Independently construct complete stock Content, with no Scholium adapter or editor origins.
#[path = "../typst-edit-session/src/kernel.rs"]
#[allow(dead_code)]
mod kernel;
use typst::foundations::{Content, NativeElement};
use typst::layout::Abs;
use typst::math::{AttachElem, ClassElem, EquationElem, FracElem};
use typst::model::{EmphElem, ParElem, StrongElem};
use typst::text::{FontFamily, FontList, TextElem, TextSize};

/// One math atom the way the editor shows it: a text element in its default math class.
/// Letters use the math-italic code points, since stock auto-italic only reaches symbol
/// elements. A native symbol element differs only in its script-size glyph forms (ssty),
/// by at most one pixel column inside scripts; that is why this fixture mirrors the atom.
fn symbol(value: &str) -> Content {
    let mut chars = value.chars();
    let shown = match (chars.next(), chars.next()) {
        (Some(c @ 'a'..='z'), None) => char::from_u32(0x1D44E + (c as u32 - 'a' as u32)),
        (Some(c @ 'A'..='Z'), None) => char::from_u32(0x1D434 + (c as u32 - 'A' as u32)),
        _ => None,
    };
    let text = kernel::text(&shown.map_or_else(|| value.to_owned(), String::from));
    let class = value
        .chars()
        .next()
        .and_then(typst::utils::default_math_class);
    class.map_or(text.clone(), |class| ClassElem::new(class, text).pack())
}

fn scripts(base: Content, sub: Option<Content>, sup: Option<Content>) -> Content {
    let mut elem = AttachElem::new(base);
    elem.b.set(sub);
    elem.t.set(sup);
    elem.pack()
}

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
        // `x^2 + y_i = a_n^23` as independent stock Content: atoms in math classes,
        // numbers as text, scripts as AttachElem.
        "scripts" => Content::sequence([
            kernel::text("E = "),
            EquationElem::new(Content::sequence([
                scripts(symbol("x"), None, Some(kernel::text("2"))),
                symbol("+"),
                scripts(symbol("y"), Some(symbol("i")), None),
                symbol("="),
                scripts(symbol("a"), Some(symbol("n")), Some(kernel::text("23"))),
            ]))
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
    for name in ["full", "scripts", "styles", "heading"] {
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
