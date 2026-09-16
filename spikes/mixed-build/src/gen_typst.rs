//! Typst 侧源码生成：宿主入口与同方言组件片段/独立文档。
//!
//! 编号探针写法：`#context [#metadata(str(counter(...).get().first())) <num:ID>]`。
//! 它在布局期把引擎分配的编号写进元数据，构建循环据此读回真实编号做验证。

use std::fmt::Write as _;

use crate::diag::Diagnostic;
use crate::gen::Ctx;
use crate::ir::{Block, Component, MacroKind, PlotSpec, Project, TableSpec};

/// 独立组件的页面尺寸（pt）。
pub(crate) const COMPONENT_PAGE: (f64, f64) = (320.0, 200.0);

/// 生成 Typst 宿主文档。
pub(crate) fn host_source(ctx: &Ctx<'_>) -> Result<String, Diagnostic> {
    let mut out = preamble(ctx.project, true);
    for block in &ctx.project.body {
        typst_block(block, ctx, &mut out)?;
    }
    Ok(out)
}

/// 生成同方言组件片段（被宿主 `#include`）。
pub(crate) fn include_fragment(
    ctx: &Ctx<'_>,
    component: &Component,
) -> Result<String, Diagnostic> {
    let mut out = String::new();
    for decl in ctx.plan.macros.values() {
        if decl.owner == component.id && decl.scope == component.scope {
            typst_macro(&mut out, decl);
        }
    }
    for block in &component.body {
        typst_block(block, ctx, &mut out)?;
    }
    Ok(out)
}

/// 生成独立组件文档。
pub(crate) fn standalone_source(
    ctx: &Ctx<'_>,
    component: &Component,
) -> Result<String, Diagnostic> {
    let mut out = preamble(ctx.project, false);
    for decl in ctx.plan.macros.values() {
        if decl.owner == component.id {
            typst_macro(&mut out, decl);
        }
    }
    for block in &component.body {
        typst_block(block, ctx, &mut out)?;
    }
    Ok(out)
}

/// Typst 前言。
pub(crate) fn preamble(project: &Project, host: bool) -> String {
    let (width, height) = if host {
        project.page
    } else {
        COMPONENT_PAGE
    };
    let mut out = String::new();
    let _ = write!(
        out,
        "#set page(width: {width}pt, height: {height}pt, margin: {}pt, numbering: \"1\")\n\
         #set heading(numbering: \"1.\")\n\
         #set math.equation(numbering: \"(1)\")\n\
         #set figure(numbering: \"1\")\n\
         #set par(justify: false)\n",
        if host { 48 } else { 8 }
    );
    out
}

/// 生成一个块的 Typst 源码。
fn typst_block(block: &Block, ctx: &Ctx<'_>, out: &mut String) -> Result<(), Diagnostic> {
    match block {
        Block::Heading { level, text } => {
            let marker = if *level <= 1 { "=" } else { "==" };
            let _ = writeln!(out, "{marker} {}\n", escape_typst(text));
        }
        Block::Para(text) => {
            let _ = writeln!(out, "{}\n", escape_typst(text));
        }
        Block::PageBreak => out.push_str("#pagebreak()\n"),
        Block::Table(spec) => {
            let _ = write!(
                out,
                "#figure(\n  table(\n    columns: {},\n    table.header({}),\n{},\n  ),\n  \
                 kind: table,\n  caption: [{}],\n) <{}>\n{}\n",
                spec.columns.max(1),
                spec.header
                    .iter()
                    .map(|cell| format!("[{}]", escape_typst(cell)))
                    .collect::<Vec<_>>()
                    .join(", "),
                table_rows(spec),
                escape_typst(&spec.caption),
                spec.label,
                number_probe("table", &spec.label)
            );
        }
        Block::Equation { label, math, display } => {
            let body = math.to_typst()?;
            if *display {
                let _ = write!(
                    out,
                    "$ {body} $ <{label}>\n{}\n",
                    number_probe("math.equation", label)
                );
            } else {
                let _ = write!(out, "$ {body} $\n");
            }
        }
        Block::Figure { label, caption, plot } => {
            let _ = write!(
                out,
                "#figure(\n{},\n  kind: \"figure\",\n  supplement: [图],\n  caption: [{}],\n) <{}>\n{}\n",
                plot_box(plot),
                escape_typst(caption),
                label,
                number_probe("figure", label)
            );
        }
        Block::ForeignFigure {
            label,
            caption,
            component,
        } => {
            let _ = write!(
                out,
                "#metadata(none) <comp:{component}>\n\
                 #figure(\n  image(\"{component}.pdf\", width: 55%),\n  \
                 kind: \"figure\",\n  supplement: [图],\n  caption: [{}],\n) <{}>\n{}\n",
                escape_typst(caption),
                label,
                number_probe("figure", label)
            );
        }
        Block::IncludeBlock { component } => {
            if ctx.unscoped {
                let _ = write!(out, "#include \"{component}.typ\"\n");
            } else {
                // 代码块形成作用域：组件里的定义不会泄漏到宿主。
                let _ = write!(out, "#{{\n  include \"{component}.typ\"\n}}\n");
            }
        }
        Block::MacroUse { name, args } => {
            let rendered = args
                .iter()
                .map(|arg| format!("\"{}\"", arg.replace('"', "\\\"")))
                .collect::<Vec<_>>()
                .join(", ");
            let _ = write!(out, "#{name}({rendered})\n");
        }
        Block::Ref { target, page } => {
            let _ = write!(out, "{}\n", ctx.reference(target, *page));
        }
        Block::Raw { text, .. } => {
            let _ = writeln!(out, "{text}");
        }
        Block::FeedbackSpace { probe, base, slope } => {
            let height = ctx.feedback_height(probe, *base, *slope);
            let _ = write!(out, "#block(height: {height:.2}pt)\n");
        }
    }
    Ok(())
}

/// 表格数据行。
fn table_rows(spec: &TableSpec) -> String {
    let mut rows = Vec::new();
    for row in &spec.rows {
        let cells = row
            .iter()
            .map(|cell| format!("[{}]", escape_typst(cell)))
            .collect::<Vec<_>>()
            .join(", ");
        rows.push(format!("    {cells},"));
    }
    for index in 0..spec.filler {
        let cells = (0..spec.columns.max(1))
            .map(|column| format!("[填充 {}·{}]", index + 1, column + 1))
            .collect::<Vec<_>>()
            .join(", ");
        rows.push(format!("    {cells},"));
    }
    rows.join("\n")
}

/// 原生折线图：用 `curve` 画坐标轴与数据折线。
fn plot_box(plot: &PlotSpec) -> String {
    let (box_w, box_h) = (200.0_f64, 120.0_f64);
    let (pad_x, pad_y) = (10.0_f64, 10.0_f64);
    let max_x = plot
        .points
        .iter()
        .map(|(x, _)| *x)
        .fold(f64::MIN, f64::max)
        .max(1.0);
    let max_y = plot
        .points
        .iter()
        .map(|(_, y)| *y)
        .fold(f64::MIN, f64::max)
        .max(1.0);
    let map = |x: f64, y: f64| {
        (
            pad_x + x / max_x * (box_w - 2.0 * pad_x),
            box_h - pad_y - y / max_y * (box_h - 2.0 * pad_y),
        )
    };
    let axes = format!(
        "#place(top + left, curve(\n      curve.move(({:.1}pt, {:.1}pt)),\n      \
         curve.line(({:.1}pt, {:.1}pt)),\n      curve.line(({:.1}pt, {:.1}pt)),\n      \
         stroke: 0.6pt,\n    ))",
        pad_x,
        box_h - pad_y,
        pad_x,
        pad_y,
        box_w - pad_x,
        pad_y
    );
    let mut curve = String::from("#place(top + left, curve(\n");
    for (index, (x, y)) in plot.points.iter().enumerate() {
        let (px, py) = map(*x, *y);
        let call = if index == 0 { "curve.move" } else { "curve.line" };
        let _ = write!(curve, "      {call}(({px:.1}pt, {py:.1}pt)),\n");
    }
    curve.push_str("      stroke: 1pt + blue,\n    ))");
    format!(
        "  box(width: {box_w:.1}pt, height: {box_h:.1}pt, stroke: 0.3pt + luma(60))[\n    \
         {axes}\n    {curve}\n  ]"
    )
}

/// 编号探针：把引擎分配的编号写进带标签的元数据，构建循环据此读回。
fn number_probe(counter: &str, label: &str) -> String {
    format!(
        "#context [#metadata(str(counter({counter}).get().first())) <num:{label}>]"
    )
}

/// 宏定义。
fn typst_macro(out: &mut String, decl: &crate::ir::MacroDecl) {
    match &decl.kind {
        MacroKind::Multiply { factor } => {
            let _ = writeln!(out, "#let {}(v) = v * {factor}", decl.name);
        }
        MacroKind::Constant { value } => {
            let _ = writeln!(out, "#let {}() = [{}]", decl.name, escape_typst(value));
        }
    }
}

/// Typst 标记模式转义。
pub(crate) fn escape_typst(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '#' | '$' | '@' | '<' | '[' | ']' | '*' | '_' | '`' | '\\' => {
                out.push('\\');
                out.push(character);
            }
            other => out.push(other),
        }
    }
    out
}
