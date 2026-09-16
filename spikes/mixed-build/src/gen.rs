//! LaTeX 侧源码生成：宿主入口与同方言组件的片段/独立文档。
//!
//! 生成的每一份 `.tex` 都写入 `out/<夹具>/` 并交给真实 `xelatex` 编译；
//! 引用值来自上一轮解析结果（`RefTable`），因此构建是**有界多轮**而不是一次成型的猜测。

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::diag::Diagnostic;
use crate::ir::{Block, Bridge, Component, Dialect, MacroKind, PlotSpec, Project, TableSpec};
use crate::plan::Plan;

/// 一个符号的已解析值。
#[derive(Clone, Debug)]
pub(crate) struct Resolved {
    /// 编号（如 "1"）。
    pub(crate) number: String,
    /// 物理页码。
    pub(crate) page: Option<u64>,
    /// 定义归属（`"host"` 或组件 id）。
    pub(crate) owner: String,
}

/// 引用解析表。
pub(crate) type RefTable = BTreeMap<String, Resolved>;

/// 生成上下文。
pub(crate) struct Ctx<'a> {
    /// 项目。
    pub(crate) project: &'a Project,
    /// 计划。
    pub(crate) plan: &'a Plan,
    /// 当前轮解析表。
    pub(crate) table: &'a RefTable,
    /// 目标方言。
    pub(crate) dialect: Dialect,
    /// 当前文件归属（`"host"` 或组件 id）。
    pub(crate) owner: &'a str,
    /// 当前作用域。
    pub(crate) scope: &'a str,
    /// 是否禁用作用域隔离（对照实现）。
    pub(crate) unscoped: bool,
}

impl Ctx<'_> {
    /// 某个符号的引用写法：同归属用原生引用，跨归属用上一轮解析值。
    fn reference(&self, target: &str, page: bool) -> String {
        let resolved = self.table.get(target);
        let native = self
            .plan
            .symbols
            .get(target)
            .map(|info| info.owner == self.owner)
            .unwrap_or(false);
        if native {
            return match self.dialect {
                Dialect::Latex => {
                    if page {
                        format!("\\pageref{{{target}}}")
                    } else {
                        format!("\\ref{{{target}}}")
                    }
                }
                Dialect::Typst => {
                    if page {
                        format!("#ref(<{target}>, form: \"page\", supplement: [页])")
                    } else {
                        format!("@{target}")
                    }
                }
            };
        }
        let value = match resolved {
            Some(resolved) if page => resolved
                .page
                .map(|page| page.to_string())
                .unwrap_or_else(|| "?".to_string()),
            Some(resolved) => resolved.number.clone(),
            None => "?".to_string(),
        };
        // 跨引擎目标只能落到"组件所在位置"这一粒度上，链接指向组件锚点。
        let anchor = resolved.map(|resolved| resolved.owner.clone());
        match (self.dialect, anchor) {
            (Dialect::Latex, Some(owner)) if self.owner == "host" => {
                format!("\\hyperref[comp:{owner}]{{{value}}}")
            }
            (Dialect::Typst, Some(owner)) if self.owner == "host" => {
                format!("#link(<comp:{owner}>)[{value}]")
            }
            _ => value,
        }
    }

    /// 反馈高度：由探测符号上一轮解析页码决定。
    fn feedback_height(&self, probe: &str, base: f64, slope: f64) -> f64 {
        let page = self
            .table
            .get(probe)
            .and_then(|resolved| resolved.page)
            .unwrap_or(1) as f64;
        (base - slope * page).max(1.0)
    }
}

/// 生成 LaTeX 宿主文档。
pub(crate) fn host_source(ctx: &Ctx<'_>) -> Result<String, Diagnostic> {
    let mut out = String::new();
    out.push_str(&preamble(ctx.project, ctx.dialect));
    out.push_str("\\begin{document}\n");
    for block in &ctx.project.body {
        latex_block(block, ctx, &mut out)?;
    }
    out.push_str("\\end{document}\n");
    Ok(out)
}

/// 生成同方言组件的片段（被宿主 `\input`）。
pub(crate) fn include_fragment(
    ctx: &Ctx<'_>,
    component: &Component,
) -> Result<String, Diagnostic> {
    let mut out = String::new();
    let mut defs = String::new();
    for decl in ctx.plan.macros.values() {
        if decl.owner == component.id && decl.scope == component.scope {
            latex_macro(&mut defs, decl);
        }
    }
    out.push_str(&defs);
    for block in &component.body {
        latex_block(block, ctx, &mut out)?;
    }
    Ok(out)
}

/// 生成独立组件文档（矢量嵌入或宏桥接的权威原文）。
pub(crate) fn standalone_source(
    ctx: &Ctx<'_>,
    component: &Component,
) -> Result<String, Diagnostic> {
    let mut out = String::new();
    out.push_str(&preamble(ctx.project, component.dialect));
    out.push_str("\\begin{document}\n");
    let mut defs = String::new();
    for decl in ctx.plan.macros.values() {
        if decl.owner == component.id {
            latex_macro(&mut defs, decl);
        }
    }
    out.push_str(&defs);
    for block in &component.body {
        latex_block(block, ctx, &mut out)?;
    }
    out.push_str("\\end{document}\n");
    Ok(out)
}

/// LaTeX 导言区。
pub(crate) fn preamble(project: &Project, _dialect: Dialect) -> String {
    let (width, height) = project.page;
    format!(
        "\\documentclass[11pt]{{article}}\n\
         \\usepackage[paperwidth={width}pt,paperheight={height}pt,margin=48pt]{{geometry}}\n\
         \\usepackage{{longtable}}\n\
         \\usepackage{{booktabs}}\n\
         \\usepackage{{pgfplots}}\n\
         \\pgfplotsset{{compat=1.18}}\n\
         \\usepackage{{graphicx}}\n\
         \\usepackage[colorlinks=true,linkcolor=blue]{{hyperref}}\n\
         \\setlength{{\\parindent}}{{0pt}}\n"
    )
}

/// 生成一个块的 LaTeX 源码。
fn latex_block(block: &Block, ctx: &Ctx<'_>, out: &mut String) -> Result<(), Diagnostic> {
    match block {
        Block::Heading { level, text } => {
            let command = if *level <= 1 { "section" } else { "subsection" };
            let _ = writeln!(out, "\\{command}{{{}}}", escape_latex(text));
        }
        Block::Para(text) => {
            let _ = writeln!(out, "{}\n", escape_latex(text));
        }
        Block::PageBreak => out.push_str("\\newpage\n"),
        Block::Table(spec) => latex_longtable(spec, out),
        Block::Equation { label, math, display } => {
            let body = math.to_latex()?;
            if *display {
                let _ = write!(
                    out,
                    "\\begin{{equation}}\n{body}\\label{{{label}}}\n\\end{{equation}}\n"
                );
            } else {
                let _ = write!(out, "${body}$\n");
            }
        }
        Block::Figure { label, caption, plot } => latex_plot(label, caption, plot, out),
        Block::ForeignFigure {
            label,
            caption,
            component,
        } => {
            let width = 0.55;
            let _ = write!(
                out,
                "\\begin{{figure}}[htbp]\n\\centering\n\
                 \\phantomsection\\label{{comp:{component}}}%\n\
                 \\includegraphics[width={width}\\linewidth]{{{component}.pdf}}\n\
                 \\caption{{{}}}\n\\label{{{label}}}\n\\end{{figure}}\n",
                escape_latex(caption)
            );
        }
        Block::IncludeBlock { component } => {
            let Some(found) = ctx.project.component(component) else {
                return Ok(());
            };
            if ctx.unscoped {
                let _ = write!(out, "\\input{{{}}}\n", found.id);
            } else {
                let _ = write!(out, "\\begingroup\n\\input{{{}}}\n\\endgroup\n", found.id);
            }
        }
        Block::MacroUse { name, args } => {
            let rendered = args
                .iter()
                .map(|arg| escape_latex(arg))
                .collect::<Vec<_>>()
                .join("}{");
            let _ = write!(out, "\\{name}{{{rendered}}}\n");
        }
        Block::Ref { target, page } => {
            let _ = write!(out, "{}\n", ctx.reference(target, *page));
        }
        Block::Raw { text, .. } => {
            let _ = writeln!(out, "{text}");
        }
        Block::FeedbackSpace { probe, base, slope } => {
            let height = ctx.feedback_height(probe, *base, *slope);
            let _ = write!(out, "\\vspace*{{{height:.2}pt}}\n");
        }
    }
    Ok(())
}

/// 跨页表格。
fn latex_longtable(spec: &TableSpec, out: &mut String) {
    let columns = "l".repeat(spec.columns.max(1));
    let header = spec
        .header
        .iter()
        .map(|cell| escape_latex(cell))
        .collect::<Vec<_>>()
        .join(" & ");
    let _ = write!(
        out,
        "\\begin{{longtable}}{{{columns}}}\n\
         \\caption{{{}}}\n\\label{{{}}}\\\\\n\
         \\toprule\n{header} \\\\\n\\midrule\n\\endfirsthead\n\
         \\multicolumn{{{}}}{{l}}{{\\small 续表}}\\\\\n\
         \\toprule\n{header} \\\\\n\\midrule\n\\endhead\n\
         \\bottomrule\n\\endlastfoot\n",
        escape_latex(&spec.caption),
        spec.label,
        spec.columns
    );
    for row in &spec.rows {
        let cells = row
            .iter()
            .map(|cell| escape_latex(cell))
            .collect::<Vec<_>>()
            .join(" & ");
        let _ = write!(out, "{cells} \\\\\n");
    }
    for index in 0..spec.filler {
        let cells = (0..spec.columns.max(1))
            .map(|column| format!("填充 {}·{}", index + 1, column + 1))
            .collect::<Vec<_>>()
            .join(" & ");
        let _ = write!(out, "{cells} \\\\\n");
    }
    out.push_str("\\end{longtable}\n");
}

/// pgfplots 折线图。
fn latex_plot(label: &str, caption: &str, plot: &PlotSpec, out: &mut String) {
    let coordinates = plot
        .points
        .iter()
        .map(|(x, y)| format!("({x},{y})"))
        .collect::<Vec<_>>()
        .join(" ");
    let _ = write!(
        out,
        "\\begin{{figure}}[htbp]\n\\centering\n\\begin{{tikzpicture}}\n\
         \\begin{{axis}}[width=7cm,height=4.2cm,xlabel={{x}},ylabel={{y}},grid=major]\n\
         \\addplot[mark=*,thick,blue] coordinates {{{coordinates}}};\n\
         \\end{{axis}}\n\\end{{tikzpicture}}\n\
         \\caption{{{}}}\n\\label{{{label}}}\n\\end{{figure}}\n",
        escape_latex(caption)
    );
}

/// 宏定义：语言无关的宏体在宿主侧的具体形态。
fn latex_macro(out: &mut String, decl: &crate::ir::MacroDecl) {
    match &decl.kind {
        MacroKind::Multiply { factor } => {
            let _ = write!(
                out,
                "\\newcommand{{\\{}}}[1]{{\\the\\numexpr#1*{factor}\\relax}}\n",
                decl.name
            );
        }
        MacroKind::Constant { value } => {
            let _ = write!(
                out,
                "\\newcommand{{\\{}}}[0]{{{}}}\n",
                decl.name,
                escape_latex(value)
            );
        }
    }
}

/// LaTeX 特殊字符转义。
pub(crate) fn escape_latex(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '\\' => out.push_str("\\textbackslash{}"),
            '&' | '%' | '$' | '#' | '_' | '{' | '}' => {
                out.push('\\');
                out.push(character);
            }
            '~' => out.push_str("\\textasciitilde{}"),
            '^' => out.push_str("\\textasciicircum{}"),
            other => out.push(other),
        }
    }
    out
}

/// 组件是否有可被宿主 `\input` 的权威源文件。
pub(crate) fn is_include(component: &Component) -> bool {
    matches!(component.bridge, Bridge::Include)
}
