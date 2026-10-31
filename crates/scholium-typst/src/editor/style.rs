//! Typst `Styles` for the editing surface, built from the same constants as
//! the A4 preview preamble (`crate::style`), so both lay out identically.

use std::num::NonZeroUsize;
use std::str::FromStr;

use typst::foundations::{Content, Dict, FromValue, IntoValue, NativeElement, Smart, Value};
use typst::layout::{Abs, Em, Length};
use typst::model::{FirstLineIndent, HeadingElem, ParElem};
use typst::text::{FontFamily, FontList, FontWeight, Lang, Region, TextElem, TextSize};

use crate::style::{
    BODY_FONTS, BODY_SIZE_PT, FIRST_LINE_INDENT_EM, HEADING_FONTS, LANG, LEADING_EM, REGION,
};

fn fonts(names: &[&str]) -> FontList {
    FontList(names.iter().map(|name| FontFamily::new(name)).collect())
}

/// Document-wide text and paragraph rules (the preamble's `set text`/`set par`).
pub(super) fn apply_document_style(content: Content, profile: crate::FontProfile) -> Content {
    content
        .styled(TextElem::font.set(fonts(profile.body(BODY_FONTS))))
        .styled(TextElem::size.set(TextSize(Abs::pt(BODY_SIZE_PT).into())))
        .styled(TextElem::lang.set(Lang::from_str(LANG).expect("fixed language code")))
        .styled(TextElem::region.set(Some(Region::from_str(REGION).expect("fixed region code"))))
        .styled(ParElem::justify.set(true))
        .styled(ParElem::leading.set(Em::new(LEADING_EM).into()))
        .styled(ParElem::first_line_indent.set(first_line_indent()))
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

/// A real heading element with the preview's `show heading: set text(...)` applied.
pub(super) fn heading(level: usize, body: Content, profile: crate::FontProfile) -> Content {
    let level = NonZeroUsize::new(level).expect("heading levels start at one");
    HeadingElem::new(body)
        .with_level(Smart::Custom(level))
        .pack()
        .styled(TextElem::font.set(fonts(profile.body(HEADING_FONTS))))
        .styled(TextElem::weight.set(FontWeight::BOLD))
}

/// Drops the first-line indent. The fork anchors the caret of an empty line on
/// the line's text, and the indent's placeholder character would hide it, so
/// empty blocks are laid out without indent.
pub(super) fn without_indent(content: Content) -> Content {
    content.styled(ParElem::first_line_indent.set(FirstLineIndent::default()))
}
