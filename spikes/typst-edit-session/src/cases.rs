//! Identical text fixtures compiled by the fork and independent stock reference.

use typst::foundations::{Content, NativeElement};
use typst::layout::Dir;
use typst::model::ParElem;
use typst::text::TextElem;

pub(crate) const NAMES: &[&str] = &["mixed", "ligature", "rtl", "bidi", "long", "empty", "lines"];

pub(crate) fn value(name: &str) -> String {
    match name {
        "mixed" => "中文 English e\u{301} 👩\u{200d}🔬，空格与字素。".into(),
        "ligature" => "office affine ffi fi fl".into(),
        "rtl" => "אבגדה וזחטי".into(),
        "bidi" => "English אבגדה 中文 123".into(),
        "long" => "中文 English office e\u{301}，连续换行测试。 ".repeat(80),
        "empty" => String::new(),
        "lines" => "第一行\n\n第三行 English".into(),
        _ => panic!("unknown text fixture: {name}"),
    }
}

pub(crate) fn paragraph(name: &str, leaf: Option<u128>) -> Content {
    let content = crate::kernel::text(&value(name));
    #[cfg(feature = "editor")]
    let content = if let Some(node) = leaf {
        content.with_edit_origin(typst::editor::EditOrigin {
            node,
            slot: None,
            hole: false,
        })
    } else {
        content
    };
    #[cfg(not(feature = "editor"))]
    let _ = leaf;
    let paragraph = ParElem::new(content).pack();
    if name == "rtl" {
        paragraph.styled(TextElem::dir.set(typst::text::TextDir(
            typst::foundations::Smart::Custom(Dir::RTL),
        )))
    } else {
        paragraph
    }
}
