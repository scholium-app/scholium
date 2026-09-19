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
use serde::{Deserialize, Serialize};
mod observe;
pub(crate) mod worker;
use observe::*;

/// 一条链接记录。
#[derive(Serialize, Deserialize)]
pub(crate) struct LinkRecord {
    /// 所在页。
    pub(crate) page: u64,
    /// 目标页（文档内链接）。
    pub(crate) target_page: Option<u64>,
    /// 外部 URL。
    pub(crate) url: Option<String>,
}

/// 一次 Typst 编译的观测结果。
#[derive(Default, Serialize, Deserialize)]
pub(crate) struct TypstRun {
    /// First explicit line-frame baseline, converted to descent in PDF bp.
    pub(crate) inline_depth: Option<f64>,
    /// 编译产物。
    pub(crate) pdf: Vec<u8>,
    /// First-page SVG evidence, exported in the worker.
    pub(crate) svg: Option<String>,
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
    worker::compile(main, files, probes)
}

/// Worker-only compilation; never invoked by the parent build path.
fn compile_local(main: &str, files: &[(String, Entry)], probes: &[String]) -> TypstRun {
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
    let document = match warned.output {
        Ok(document) => document,
        Err(errors) => {
            run.errors = errors
                .iter()
                .map(|error| error.message.to_string())
                .collect();
            return run;
        }
    };
    run.ok = true;
    let document = &document;
    run.inline_depth = document
        .pages()
        .first()
        .and_then(|p| baseline(&p.frame).map(|b| p.frame.height().to_pt() - b));
    for (index, page) in document.pages().iter().enumerate() {
        run.text_pages.push(page_text(page));
        collect_links(&page.frame, (index + 1) as u64, document, &mut run.links);
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
    match pdf_bytes(document, probes) {
        Ok(bytes) => run.pdf = bytes,
        Err(error) => {
            run.ok = false;
            run.errors.push(error);
        }
    }
    run.svg = page_svg(document, 1);
    run
}

/// 把某页导出为 SVG（矢量载体）。
fn page_svg(document: &PagedDocument, page: usize) -> Option<String> {
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
fn pdf_bytes(document: &PagedDocument, probes: &[String]) -> Result<Vec<u8>, String> {
    let options = typst_pdf::PdfOptions {
        ident: typst::foundations::Smart::Auto,
        ..Default::default()
    };
    let introspector = document.introspector();
    let anchors: Vec<_> = probes
        .iter()
        .filter_map(|name| {
            let label = Label::new(PicoStr::intern(name))?;
            let content = introspector.query_label(label).ok().or_else(|| {
                let label = Label::new(PicoStr::intern(&format!("sch:{name}")))?;
                introspector.query_label(label).ok()
            })?;
            let location = content.location()?;
            Some((location, format!("sch:{name}").into()))
        })
        .collect();
    let resolver = typst::model::LateLinkResolver::new(None, introspector.as_ref());
    match typst_pdf::pdf_in_bundle(
        document,
        &options,
        &anchors,
        typst::comemo::Track::track(&resolver),
    ) {
        Ok(bytes) => Ok(bytes),
        Err(errors) => Err(errors
            .iter()
            .map(|error| error.message.to_string())
            .collect::<Vec<_>>()
            .join("；")),
    }
}

/// Save only the PDF already exported by the isolated worker.
pub(crate) fn export_pdf(run: &TypstRun, path: &std::path::Path) -> Result<usize, String> {
    if !run.ok || !run.pdf.starts_with(b"%PDF-") {
        return Err("missing worker PDF".into());
    }
    std::fs::write(path, &run.pdf).map_err(|error| error.to_string())?;
    Ok(run.pdf.len())
}
