//! Typst 宿主/组件驱动：crate 内编译，从真实布局读回文本、编号、页码与链接。
//!
//! 核验不依赖我们自己维护的表格：文本来自 `FrameItem::Text`，编号来自组件里的编号探针，
//! 页码来自 `Introspector::position`，链接来自 `FrameItem::Link`。

use std::collections::BTreeMap;

use typst::foundations::{Label, Value};
use typst::introspection::Introspector;
use typst::layout::{Frame, FrameItem};
use typst::model::Destination;
use typst::utils::PicoStr;
use typst_layout::{Page, PagedDocument};
use typst_svg::SvgOptions;

use crate::world::{Entry, World};

/// 一条链接记录。
pub(crate) struct LinkRecord {
    /// 所在页。
    pub(crate) page: u64,
    /// 目标页（文档内链接）。
    pub(crate) target_page: Option<u64>,
    /// 外部 URL。
    pub(crate) url: Option<String>,
}

/// 一次 Typst 编译的观测结果。
#[derive(Default)]
pub(crate) struct TypstRun {
    /// 编译产物。
    pub(crate) document: Option<PagedDocument>,
    /// 是否编译成功。
    pub(crate) ok: bool,
    /// 错误信息。
    pub(crate) errors: Vec<String>,
    /// 警告信息。
    pub(crate) warnings: Vec<String>,
    /// 编号探针读回的编号。
    pub(crate) numbers: BTreeMap<String, String>,
    /// 标签所在页。
    pub(crate) label_pages: BTreeMap<String, u64>,
    /// 逐页文本。
    pub(crate) text_pages: Vec<String>,
    /// 链接。
    pub(crate) links: Vec<LinkRecord>,
}

impl TypstRun {
    /// 是否存在引擎级不收敛警告。
    pub(crate) fn non_convergent(&self) -> bool {
        self.warnings
            .iter()
            .any(|warning| warning.contains("did not converge"))
    }

    /// 整篇文本。
    pub(crate) fn all_text(&self) -> String {
        self.text_pages.join("\n")
    }
}

/// 用给定虚拟文件编译主文件；`probes` 是要读回编号的符号 id。
pub(crate) fn compile(main: &str, files: &[(String, Entry)], probes: &[String]) -> TypstRun {
    let mut run = TypstRun::default();
    let world = World::new(main.to_string());
    for (path, entry) in files {
        match entry {
            Entry::Text(text) => world.write_text(path, text.clone()),
            Entry::Bytes(bytes) => world.write_bytes(path, bytes.clone()),
        }
    }
    let warned = typst::compile::<PagedDocument>(&world);
    run.warnings = warned
        .warnings
        .iter()
        .map(|warning| warning.message.to_string())
        .collect();
    match warned.output {
        Ok(document) => {
            run.ok = true;
            run.document = Some(document);
        }
        Err(errors) => {
            run.errors = errors
                .iter()
                .map(|error| error.message.to_string())
                .collect();
            return run;
        }
    }
    let Some(document) = run.document.as_ref() else {
        return run;
    };
    for (index, page) in document.pages().iter().enumerate() {
        run.text_pages.push(page_text(page));
        collect_links(
            &page.frame,
            (index + 1) as u64,
            document,
            &mut run.links,
        );
    }
    let introspector = document.introspector();
    for probe in probes {
        let page = label_position(introspector, probe);
        if let Some(page) = page {
            run.label_pages.insert(probe.clone(), page);
        }
        let number = label_value(introspector, &format!("num:{probe}"));
        if let Some(number) = number {
            run.numbers.insert(probe.clone(), number);
        }
    }
    run
}

/// 某标签所在的物理页码。
pub(crate) fn label_position(
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
pub(crate) fn label_value(
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
pub(crate) fn page_text(page: &Page) -> String {
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

fn collect_links(frame: &Frame, page: u64, document: &PagedDocument, out: &mut Vec<LinkRecord>) {
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

/// 把某页导出为 SVG（矢量载体）。
pub(crate) fn page_svg(document: &PagedDocument, page: usize) -> Option<String> {
    let page = document.pages().get(page.saturating_sub(1))?;
    Some(typst_svg::svg(
        page,
        &SvgOptions {
            render_bleed: false,
            pretty: true,
        },
    ))
}

/// 把文档导出为真实 PDF。
pub(crate) fn export_pdf(document: &PagedDocument, path: &std::path::Path) -> Result<usize, String> {
    let options = typst_pdf::PdfOptions {
        ident: typst::foundations::Smart::Auto,
        ..Default::default()
    };
    match typst_pdf::pdf(document, &options) {
        Ok(bytes) => {
            let length = bytes.len();
            std::fs::write(path, bytes).map_err(|error| error.to_string())?;
            Ok(length)
        }
        Err(errors) => Err(errors
            .iter()
            .map(|error| error.message.to_string())
            .collect::<Vec<_>>()
            .join("；")),
    }
}
