//! 宿主/组件的产物读回：LaTeX `.aux` 与 Typst 内省，矢量组件导出。
//!
//! 读回的是**引擎真实产出**：LaTeX 的标签编号与页码来自 `.aux`，Typst 的编号来自组件里的
//! 编号探针、页码来自内省，链接来自布局元素。跨引擎引用的最终页码取自宿主装配位置。

use std::collections::BTreeMap;
use std::path::Path;

use crate::diag::Diagnostic;
use crate::generate::RefTable;
use crate::generate::Resolved;
use crate::ir::Project;
use crate::latex;
use crate::plan::Plan;
use crate::typst_host::{self, TypstRun};
use crate::world::Entry;

use crate::build::Observation;

/// 读回 LaTeX 宿主的观测，并更新解析表。
pub(crate) fn observe_latex(
    dir: &Path,
    project: &Project,
    plan: &Plan,
    next: &mut RefTable,
) -> Result<Observation, Diagnostic> {
    let run = latex::compile(dir, "main");
    if !run.ok {
        return Err(Diagnostic::new(
            "host-compile-failed",
            format!("宿主 LaTeX 编译失败：{}", run.diagnostics.join("；")),
        ));
    }
    let mut observation = Observation {
        text_pages: run.text_pages.clone(),
        label_pages: BTreeMap::new(),
        label_numbers: BTreeMap::new(),
        links: run
            .link_targets
            .iter()
            .map(|(page, target)| {
                let target_page = target
                    .rsplit('#')
                    .next()
                    .and_then(|value| value.parse::<u64>().ok());
                (*page, target_page)
            })
            .collect(),
        external_links: Vec::new(),
        pages: run.pages,
        source_path: dir.join("main.tex"),
        pdf_path: dir.join("main.pdf"),
    };
    // 先取组件锚点所在页：跨引擎引用的"最终页码"必须来自宿主装配结果。
    let mut anchor_pages: BTreeMap<String, u64> = BTreeMap::new();
    for (label, (_, page)) in &run.labels {
        observation
            .label_pages
            .insert(label.clone(), *page);
        if let Some(id) = label.strip_prefix("comp:") {
            anchor_pages.insert(id.to_string(), *page);
        }
    }
    for symbol in plan.symbols.values() {
        let Some((number, _)) = run.labels.get(&symbol.id) else {
            continue;
        };
        observation
            .label_numbers
            .insert(symbol.id.clone(), number.clone());
        if symbol.owner == "host" {
            let page = run.labels.get(&symbol.id).map(|value| value.1);
            next.insert(
                symbol.id.clone(),
                Resolved {
                    number: number.clone(),
                    page,
                    owner: symbol.owner.clone(),
                },
            );
        }
    }
    finalize_component_symbols(project, plan, next, &anchor_pages);
    Ok(observation)
}

/// 读回 Typst 宿主的观测，并更新解析表；同时导出真实 PDF 作为最终件。
pub(crate) fn observe_typst(
    run: &TypstRun,
    dir: &Path,
    project: &Project,
    plan: &Plan,
    next: &mut RefTable,
) -> Result<Observation, Diagnostic> {
    let pdf_path = dir.join("main.pdf");
    if run.ok {
        typst_host::export_pdf(run, &pdf_path).map_err(|error| {
            Diagnostic::new("host-pdf-export", format!("宿主 Typst 导出 PDF 失败：{error}"))
        })?;
    }
    let mut observation = Observation {
        text_pages: run.text_pages.clone(),
        label_pages: run.label_pages.clone(),
        label_numbers: run.numbers.clone(),
        links: run
            .links
            .iter()
            .map(|link| (link.page, link.target_page))
            .collect(),
        external_links: run
            .links
            .iter()
            .filter_map(|link| link.url.clone().map(|url| (link.page, url)))
            .collect(),
        pages: run.text_pages.len(),
        source_path: dir.join("main.typ"),
        pdf_path,
    };
    let mut anchor_pages: BTreeMap<String, u64> = BTreeMap::new();
    for (label, page) in &run.label_pages {
        if let Some(id) = label.strip_prefix("comp:") {
            anchor_pages.insert(id.to_string(), *page);
        }
    }
    for symbol in plan.symbols.values() {
        if symbol.owner != "host" {
            continue;
        }
        let number = run
            .numbers
            .get(&symbol.id)
            .cloned()
            .unwrap_or_else(|| "?".to_string());
        let page = run.label_pages.get(&symbol.id).copied();
        observation
            .label_numbers
            .insert(symbol.id.clone(), number.clone());
        next.insert(
            symbol.id.clone(),
            Resolved {
                number,
                page,
                owner: symbol.owner.clone(),
            },
        );
    }
    finalize_component_symbols(project, plan, next, &anchor_pages);
    Ok(observation)
}

/// 组件拥有的符号：编号来自组件自身编译，页码来自宿主装配位置。
fn finalize_component_symbols(
    _project: &Project,
    plan: &Plan,
    next: &mut RefTable,
    anchor_pages: &BTreeMap<String, u64>,
) {
    for symbol in plan.symbols.values() {
        if symbol.owner == "host" {
            continue;
        }
        if let Some(resolved) = next.get_mut(&symbol.id)
            && let Some(page) = anchor_pages.get(&symbol.owner)
        {
            resolved.page = Some(*page);
        }
    }
}

/// 把 Typst 组件导出为矢量载体（SVG 取证 + PDF 嵌入），供 LaTeX 宿主 `\includegraphics`。
pub(crate) fn export_component(
    dir: &Path,
    component: &crate::ir::Component,
    run: &TypstRun,
) -> Result<String, Diagnostic> {
    if !run.ok {
        return Err(Diagnostic::new(
            "component-artifact",
            format!("组件 `{}` 没有可导出的文档", component.id),
        ));
    }
    if run.text_pages.len() > 1 {
        return Err(Diagnostic::new(
            "component-multipage-unsupported",
            format!(
                "组件 `{}` 编译出 {} 页；当前受支持载体只有单页矢量嵌入",
                component.id,
                run.text_pages.len()
            ),
        ));
    }
    let Some(svg) = run.svg.as_ref() else {
        return Err(Diagnostic::new(
            "component-artifact",
            format!("组件 `{}` 无法导出 SVG", component.id),
        ));
    };
    let svg_path = dir.join(format!("{}.svg", component.id));
    let pdf_path = dir.join(format!("{}.pdf", component.id));
    let _ = std::fs::write(&svg_path, svg);
    if !svg.contains("<path") {
        return Err(Diagnostic::new(
            "component-artifact",
            format!("组件 `{}` 的 SVG 不含路径数据，可能是位图载体", component.id),
        ));
    }
    let bytes = typst_host::export_pdf(run, &pdf_path).map_err(|error| {
        Diagnostic::new(
            "component-artifact",
            format!("组件 `{}` 导出 PDF 失败：{error}", component.id),
        )
    })?;
    Ok(format!(
        "矢量产物 {}.pdf（{} 字节 PDF，{} 字节 SVG，含 {} 条路径）",
        component.id,
        bytes,
        svg.len(),
        svg.matches("<path").count()
    ))
}

/// 把已经生成的组件产物读成虚拟文件（Typst 宿主需要）。
pub(crate) fn component_files(dir: &Path, project: &Project) -> Vec<(String, Entry)> {
    let mut files = Vec::new();
    for component in &project.components {
        let artifact = format!("{}.pdf", component.id);
        if let Ok(bytes) = std::fs::read(dir.join(&artifact)) {
            files.push((artifact, Entry::Bytes(bytes)));
        }
        let source = format!("{}.typ", component.id);
        if let Ok(text) = std::fs::read_to_string(dir.join(&source)) {
            // 同方言组件走 `#include`，必须由 World 提供的文本源文件。
            files.push((source, Entry::Text(text)));
        }
    }
    files
}

/// 解析表的规范签名。
pub(crate) fn signature(table: &RefTable) -> String {
    if table.is_empty() {
        return "(空)".to_string();
    }
    table
        .iter()
        .map(|(id, resolved)| {
            format!(
                "{id}={}/{}",
                resolved.number,
                resolved
                    .page
                    .map(|page| page.to_string())
                    .unwrap_or_else(|| "-".to_string())
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}
