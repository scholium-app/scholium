//! Read layout evidence inside the isolated worker.
use super::*;
pub(super) fn baseline(frame: &Frame) -> Option<f64> {
    if frame.has_baseline() {
        return Some(frame.baseline().to_pt());
    }
    frame.items().find_map(|(pos, item)| match item {
        FrameItem::Group(group) => baseline(&group.frame).map(|b| pos.y.to_pt() + b),
        _ => None,
    })
}
/// 某标签所在的物理页码。
pub(super) fn label_position(
    introspector: &typst_layout::PagedIntrospector,
    name: &str,
) -> Option<u64> {
    let label = Label::new(PicoStr::intern(name))?;
    introspector
        .query_label(label)
        .ok()?
        .location()
        .and_then(|location| introspector.position(location))
        .map(|position| position.page.get() as u64)
}

/// 从编号探针元数据里读回编号。
pub(super) fn label_value(
    introspector: &typst_layout::PagedIntrospector,
    name: &str,
) -> Option<String> {
    let label = Label::new(PicoStr::intern(name))?;
    let content = introspector.query_label(label).ok()?;
    match content.get_by_name("value").ok()? {
        Value::Str(text) => Some(text.to_string()),
        other => Some(format!("{other:?}")),
    }
}

/// 递归抽取一页的文本，链接以 `⟦LINK⟧` 标记。
pub(super) fn page_text(page: &Page) -> String {
    let mut out = String::new();
    collect_text(&page.frame, &mut out);
    out
}

fn collect_text(frame: &Frame, out: &mut String) {
    for (_, item) in frame.items() {
        match item {
            FrameItem::Group(group) => collect_text(&group.frame, out),
            FrameItem::Text(text) => out.push_str(&text.text),
            _ => {}
        }
    }
}

pub(super) fn collect_links(
    frame: &Frame,
    page: u64,
    document: &PagedDocument,
    out: &mut Vec<LinkRecord>,
) {
    for (_, item) in frame.items() {
        match item {
            FrameItem::Group(group) => collect_links(&group.frame, page, document, out),
            FrameItem::Link(destination, _) => {
                let introspector = document.introspector();
                let (target_page, url) = match destination {
                    Destination::Url(url) => (None, Some(url.as_str().to_string())),
                    Destination::Position(position) => (Some(position.page.get() as u64), None),
                    Destination::Location(location) => (
                        introspector
                            .position(*location)
                            .map(|position| position.page.get() as u64),
                        None,
                    ),
                };
                out.push(LinkRecord {
                    page,
                    target_page,
                    url,
                });
            }
            _ => {}
        }
    }
}
