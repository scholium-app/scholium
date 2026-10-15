//! 格式 menu: text styles and the paragraph style submenu.
use super::{
    avail,
    spec::{Action, ChordKey, CommandSpec, FocusKind, IconId, KeyChord, Routing},
};
use crate::typst_editor::Command;
use scholium_model::{BlockKind, structured::TextStyle};

pub(super) fn specs() -> Vec<CommandSpec> {
    let mut specs = text();
    specs.extend(paragraph());
    specs
}

fn text() -> Vec<CommandSpec> {
    vec![
        command! {
            "format.bold",
            icon: IconId::Bold,
            shortcuts: keys![KeyChord::ctrl(ChordKey::Letter('B'))],
            routing: Routing::Editor,
            menu: menu!(Format, 0, 0),
            toolbar: toolbar!(Text, 0),
            focus: Some(FocusKind::Text),
            writes: true,
            availability: avail::page_write,
            checked: Some(avail::strong),
            action: Action::Editor(Command::Style(TextStyle::Strong)),
        },
        command! {
            "format.italic",
            icon: IconId::Italic,
            shortcuts: keys![KeyChord::ctrl(ChordKey::Letter('I'))],
            routing: Routing::Editor,
            menu: menu!(Format, 0, 1),
            toolbar: toolbar!(Text, 1),
            focus: Some(FocusKind::Text),
            writes: true,
            availability: avail::page_write,
            checked: Some(avail::emphasis),
            action: Action::Editor(Command::Style(TextStyle::Emphasis)),
        },
        command! {
            "format.underline",
            icon: IconId::Underline,
            menu: menu!(Format, 0, 2),
            toolbar: toolbar!(Text, 2),
            writes: true,
            availability: avail::unimplemented,
        },
        command! {
            "format.strike",
            icon: IconId::Strike,
            menu: menu!(Format, 0, 3),
            toolbar: toolbar!(Text, 3),
            writes: true,
            availability: avail::unimplemented,
        },
        command! {
            "format.clear",
            icon: IconId::ClearFormat,
            menu: menu!(Format, 1, 0),
            toolbar: toolbar!(Text, 4),
            focus: Some(FocusKind::Text),
            writes: true,
            availability: avail::page_write,
            action: Action::Editor(Command::Style(TextStyle::Plain)),
        },
    ]
}

/// Ctrl+0 / Ctrl+1 / Ctrl+2 set 正文 / 标题 1 / 标题 2 (also in the page event stream).
fn paragraph() -> Vec<CommandSpec> {
    vec![
        command! {
            "format.paragraph.body",
            icon: IconId::Paragraph,
            shortcuts: keys![KeyChord::ctrl(ChordKey::Digit(0))],
            routing: Routing::Editor,
            menu: menu!(Format, ParagraphStyle, 2, 0),
            toolbar: toolbar!(Paragraph, 0),
            focus: Some(FocusKind::Text),
            writes: true,
            availability: avail::page_write,
            action: Action::Editor(Command::Kind(BlockKind::Paragraph)),
        },
        command! {
            "format.paragraph.heading1",
            icon: IconId::Heading1,
            shortcuts: keys![KeyChord::ctrl(ChordKey::Digit(1))],
            routing: Routing::Editor,
            menu: menu!(Format, ParagraphStyle, 2, 1),
            toolbar: toolbar!(Paragraph, 1),
            focus: Some(FocusKind::Text),
            writes: true,
            availability: avail::page_write,
            action: Action::Editor(Command::Kind(BlockKind::Heading1)),
        },
        command! {
            "format.paragraph.heading2",
            icon: IconId::Heading2,
            shortcuts: keys![KeyChord::ctrl(ChordKey::Digit(2))],
            routing: Routing::Editor,
            menu: menu!(Format, ParagraphStyle, 2, 2),
            toolbar: toolbar!(Paragraph, 2),
            focus: Some(FocusKind::Text),
            writes: true,
            availability: avail::page_write,
            action: Action::Editor(Command::Kind(BlockKind::Heading2)),
        },
    ]
}
