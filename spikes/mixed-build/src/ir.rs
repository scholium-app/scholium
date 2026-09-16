//! 语义 IR：与引擎无关的正文/结构描述，以及双端源码生成器共同消费的模型。
//!
//! 混合构建的"源码级重建"就是从这份 IR 生成真实 `.tex` / `.typ`，由各自工具链编译；
//! 只有无法原生表达的内容才落到组件桥接（矢量嵌入）。

use crate::diag::Diagnostic;

/// 源码方言（同时也是排版宿主/工具链）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Dialect {
    /// LaTeX（XeTeX 工具链）。
    Latex,
    /// Typst（crate 内编译）。
    Typst,
}

impl Dialect {
    /// 打印用名字。
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Latex => "latex",
            Self::Typst => "typst",
        }
    }
}

/// 语言无关的数学树。支持子集之外的命令必须显式失败，不得静默丢弃。
#[derive(Clone, Debug)]
pub(crate) enum Math {
    /// 命名字符（希腊字母、运算符），由内置表映射到双端写法。
    Sym(&'static str),
    /// 整数常量。
    Num(u32),
    /// 普通标识符（变量名）。
    Ident(String),
    /// 分数。
    Frac(Box<Math>, Box<Math>),
    /// 根式。
    Sqrt(Box<Math>),
    /// 上标。
    Sup(Box<Math>, Box<Math>),
    /// 下标。
    Sub(Box<Math>, Box<Math>),
    /// 序列。
    Seq(Vec<Math>),
    /// 函数调用（仅限受支持函数名）。
    Call { name: String, args: Vec<Math> },
}

/// 数学符号映射表的项。
struct SymMap {
    name: &'static str,
    latex: &'static str,
    typst: &'static str,
}

/// 受支持的符号表。
const SYMBOLS: &[SymMap] = &[
    SymMap { name: "alpha", latex: "\\alpha", typst: "alpha" },
    SymMap { name: "beta", latex: "\\beta", typst: "beta" },
    SymMap { name: "gamma", latex: "\\gamma", typst: "gamma" },
    SymMap { name: "delta", latex: "\\delta", typst: "delta" },
    SymMap { name: "lambda", latex: "\\lambda", typst: "lambda" },
    SymMap { name: "mu", latex: "\\mu", typst: "mu" },
    SymMap { name: "pi", latex: "\\pi", typst: "pi" },
    SymMap { name: "sigma", latex: "\\sigma", typst: "sigma" },
    SymMap { name: "theta", latex: "\\theta", typst: "theta" },
    SymMap { name: "omega", latex: "\\omega", typst: "omega" },
    SymMap { name: "infty", latex: "\\infty", typst: "infinity" },
    SymMap { name: "cdot", latex: "\\cdot", typst: "dot" },
    SymMap { name: "pm", latex: "\\pm", typst: "plus.minus" },
    SymMap { name: "times", latex: "\\times", typst: "times" },
    SymMap { name: "le", latex: "\\le", typst: "lt.eq" },
    SymMap { name: "ge", latex: "\\ge", typst: "gt.eq" },
    SymMap { name: "ne", latex: "\\ne", typst: "eq.not" },
    SymMap { name: "approx", latex: "\\approx", typst: "approx" },
    SymMap { name: "to", latex: "\\to", typst: "arrow.r" },
    SymMap { name: "partial", latex: "\\partial", typst: "diff" },
    SymMap { name: "sum", latex: "\\sum", typst: "sum" },
    SymMap { name: "prod", latex: "\\prod", typst: "product" },
    SymMap { name: "int", latex: "\\int", typst: "integral" },
];

/// 受支持的函数名。
const FUNCTIONS: &[&str] = &["sin", "cos", "tan", "log", "ln", "exp", "lim", "det", "max", "min"];

impl Math {
    /// 检查是否落在受支持子集内；返回第一条不支持项的诊断。
    pub(crate) fn unsupported(&self) -> Option<Diagnostic> {
        match self {
            Self::Sym(name) => {
                if SYMBOLS.iter().any(|entry| entry.name == *name) {
                    None
                } else {
                    Some(Diagnostic::new(
                        "math-symbol-unsupported",
                        format!("数学符号 `{name}` 不在受支持符号表内"),
                    ))
                }
            }
            Self::Num(_) | Self::Ident(_) => None,
            Self::Frac(a, b) | Self::Sup(a, b) | Self::Sub(a, b) => {
                a.unsupported().or_else(|| b.unsupported())
            }
            Self::Sqrt(inner) => inner.unsupported(),
            Self::Seq(items) => items.iter().find_map(Self::unsupported),
            Self::Call { name, args } => {
                if !FUNCTIONS.contains(&name.as_str()) {
                    return Some(
                        Diagnostic::new(
                            "math-call-unsupported",
                            format!("数学命令 `{name}` 不在受支持命令表内"),
                        )
                        .hint("复杂公式应改为外语组件走矢量嵌入，不能静默丢弃"),
                    );
                }
                args.iter().find_map(Self::unsupported)
            }
        }
    }

    /// 生成 LaTeX 数学源码。
    pub(crate) fn to_latex(&self) -> Result<String, Diagnostic> {
        if let Some(problem) = self.unsupported() {
            return Err(problem);
        }
        Ok(self.render_latex())
    }

    /// 生成 Typst 数学源码。
    pub(crate) fn to_typst(&self) -> Result<String, Diagnostic> {
        if let Some(problem) = self.unsupported() {
            return Err(problem);
        }
        Ok(self.render_typst())
    }

    fn render_latex(&self) -> String {
        match self {
            Self::Sym(name) => SYMBOLS
                .iter()
                .find(|entry| entry.name == *name)
                .map(|entry| entry.latex.to_string())
                .unwrap_or_default(),
            Self::Num(value) => value.to_string(),
            Self::Ident(name) => name.clone(),
            Self::Frac(a, b) => format!("\\frac{{{}}}{{{}}}", a.render_latex(), b.render_latex()),
            Self::Sqrt(inner) => format!("\\sqrt{{{}}}", inner.render_latex()),
            Self::Sup(base, exp) => {
                format!("{}^{{{}}}", base.render_latex(), exp.render_latex())
            }
            Self::Sub(base, sub) => {
                format!("{}_{{{}}}", base.render_latex(), sub.render_latex())
            }
            Self::Seq(items) => items
                .iter()
                .map(Self::render_latex)
                .collect::<Vec<_>>()
                .join(" "),
            Self::Call { name, args } => {
                let head = if FUNCTIONS.contains(&name.as_str()) {
                    format!("\\{name}")
                } else {
                    name.clone()
                };
                let rendered = args
                    .iter()
                    .map(Self::render_latex)
                    .collect::<Vec<_>>()
                    .join("}{");
                if args.is_empty() {
                    head
                } else {
                    format!("{head}{{{rendered}}}")
                }
            }
        }
    }

    fn render_typst(&self) -> String {
        match self {
            Self::Sym(name) => SYMBOLS
                .iter()
                .find(|entry| entry.name == *name)
                .map(|entry| entry.typst.to_string())
                .unwrap_or_default(),
            Self::Num(value) => value.to_string(),
            Self::Ident(name) => name.clone(),
            Self::Frac(a, b) => format!("frac({}, {})", a.render_typst(), b.render_typst()),
            Self::Sqrt(inner) => format!("sqrt({})", inner.render_typst()),
            Self::Sup(base, exp) => format!("{}^({})", base.render_typst(), exp.render_typst()),
            Self::Sub(base, sub) => format!("{}_({})", base.render_typst(), sub.render_typst()),
            Self::Seq(items) => items
                .iter()
                .map(Self::render_typst)
                .collect::<Vec<_>>()
                .join(" "),
            Self::Call { name, args } => {
                let rendered = args
                    .iter()
                    .map(Self::render_typst)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{name}({rendered})")
            }
        }
    }
}

/// 表格规格。
#[derive(Clone, Debug)]
pub(crate) struct TableSpec {
    /// 标签（同时是符号 id）。
    pub(crate) label: String,
    /// 表题。
    pub(crate) caption: String,
    /// 声明列数。
    pub(crate) columns: usize,
    /// 表头单元。
    pub(crate) header: Vec<String>,
    /// 数据行。
    pub(crate) rows: Vec<Vec<String>>,
    /// 追加的填充行数（用于把表推到多页）。
    pub(crate) filler: usize,
}

/// 可复现绘图规格（折线）。
#[derive(Clone, Debug)]
pub(crate) struct PlotSpec {
    /// 数据点。
    pub(crate) points: Vec<(f64, f64)>,
}

/// 块级内容。
#[derive(Clone, Debug)]
pub(crate) enum Block {
    /// 标题。
    Heading { level: u8, text: String },
    /// 段落。
    Para(String),
    /// 跨页表格。
    Table(TableSpec),
    /// 公式。
    Equation {
        /// 符号 id。
        label: String,
        /// 数学树。
        math: Math,
        /// 是否独立成行。
        display: bool,
    },
    /// 宿主原生绘图。
    Figure {
        /// 符号 id。
        label: String,
        /// 图题。
        caption: String,
        /// 绘图数据。
        plot: PlotSpec,
    },
    /// 嵌入外语矢量组件的图。
    ForeignFigure {
        /// 符号 id（宿主侧标签）。
        label: String,
        /// 图题。
        caption: String,
        /// 组件 id。
        component: String,
    },
    /// 以真实源码方式纳入同方言组件（作用域隔离）。
    IncludeSection {
        /// 组件 id。
        component: String,
    },
    /// 使用宏。
    MacroUse {
        /// 宏名。
        name: String,
        /// 字符串实参。
        args: Vec<String>,
    },
    /// 引用。
    Ref {
        /// 目标符号 id。
        target: String,
        /// 是否引用页码（否则引用编号）。
        page: bool,
    },
    /// 原始外语语法（不理解时必须显式失败，不得静默产出）。
    Raw {
        /// 该原文所属方言。
        dialect: Dialect,
        /// 原文。
        text: String,
    },
    /// 分页。
    PageBreak,
    /// 反馈空间：高度由某个符号的**上一轮解析页码**决定（振荡夹具专用）。
    FeedbackSpace {
        /// 探测符号 id。
        probe: String,
        /// 基准高度（pt）。
        base: f64,
        /// 每页扣减（pt）。
        slope: f64,
    },
}

impl Block {
    /// 该块定义的符号 id（若有）。
    pub(crate) fn label(&self) -> Option<&str> {
        match self {
            Self::Table(spec) => Some(&spec.label),
            Self::Equation { label, .. } => Some(label),
            Self::Figure { label, .. } => Some(label),
            Self::ForeignFigure { label, .. } => Some(label),
            _ => None,
        }
    }
}

/// 符号种类。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SymbolKind {
    /// 表格。
    Table,
    /// 公式。
    Equation,
    /// 图。
    Figure,
    /// 章节。
    Section,
}

impl SymbolKind {
    /// 打印名。
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Table => "table",
            Self::Equation => "equation",
            Self::Figure => "figure",
            Self::Section => "section",
        }
    }
}

/// 宏体：语言无关，双端各自生成，因此可以断言"语义一致"而不只是文本抄写。
#[derive(Clone, Debug)]
pub(crate) enum MacroKind {
    /// `name(v) = v * factor`。
    Multiply { factor: i64 },
    /// `name() = 固定文本`。
    Constant { value: String },
}

/// 宏声明。
#[derive(Clone, Debug)]
pub(crate) struct MacroDecl {
    /// 宏名（双端同名，便于按名查找宿主侧定义）。
    pub(crate) name: String,
    /// 定义所属作用域。
    pub(crate) scope: String,
    /// 定义所属组件（`"host"` 表示宿主正文）。
    pub(crate) owner: String,
    /// 定义所在方言。
    pub(crate) dialect: Dialect,
    /// 宏体。
    pub(crate) kind: MacroKind,
    /// 形参个数。
    pub(crate) params: usize,
    /// 跨语言桥接合约 id（跨语言使用时必填）。
    pub(crate) contract: Option<String>,
}

/// 组件桥接方式。
#[derive(Clone, Debug)]
pub(crate) enum Bridge {
    /// 同方言：作为真实源码 `\input` / `#include`，语义完整。
    Include,
    /// 异方言：编译为矢量载体后嵌入，布局保真但语义冻结。
    Vector,
    /// 异方言宏：按声明合约在宿主侧重实现。
    Macro { contract: String },
}

impl Bridge {
    /// 打印名。
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Include => "include",
            Self::Vector => "vector",
            Self::Macro { .. } => "macro",
        }
    }
}

/// 组件放置位置。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Placement {
    /// 块级。
    Block,
    /// 行内。
    Inline,
}

/// 一个混用组件：有独立权威源文件，由自己的工具链编译。
#[derive(Clone, Debug)]
pub(crate) struct Component {
    /// 组件 id。
    pub(crate) id: String,
    /// 组件方言。
    pub(crate) dialect: Dialect,
    /// 组件所属作用域。
    pub(crate) scope: String,
    /// 组件正文（IR）。
    pub(crate) body: Vec<Block>,
    /// 桥接方式。
    pub(crate) bridge: Bridge,
    /// 放置位置。
    pub(crate) placement: Placement,
    /// 依赖的组件 id。
    pub(crate) depends_on: Vec<String>,
}

/// 一个混合项目：一个宿主 + 若干组件。
#[derive(Clone, Debug)]
pub(crate) struct Project {
    /// 项目名。
    pub(crate) name: String,
    /// 宿主方言。
    pub(crate) host: Dialect,
    /// 宿主正文。
    pub(crate) body: Vec<Block>,
    /// 宏声明。
    pub(crate) macros: Vec<MacroDecl>,
    /// 组件。
    pub(crate) components: Vec<Component>,
    /// 宿主页面尺寸（pt）。
    pub(crate) page: (f64, f64),
    /// 禁用作用域隔离（对照实现，用于证明隔离确实在起作用）。
    pub(crate) unscoped_control: bool,
}

/// 符号的解析归属。
#[derive(Clone, Debug)]
pub(crate) struct SymbolInfo {
    /// 符号 id。
    pub(crate) id: String,
    /// 种类。
    pub(crate) kind: SymbolKind,
    /// 定义所在（`"host"` 或组件 id）。
    pub(crate) owner: String,
    /// 定义所在作用域。
    pub(crate) scope: String,
}

impl Project {
    /// 遍历宿主与全部组件里的块。
    pub(crate) fn all_blocks(&self) -> impl Iterator<Item = (&str, &Block)> {
        let host = self.body.iter().map(|block| ("host", block));
        let components = self
            .components
            .iter()
            .flat_map(|component| component.body.iter().map(move |block| (component.id.as_str(), block)));
        host.chain(components)
    }

    /// 组件查找。
    pub(crate) fn component(&self, id: &str) -> Option<&Component> {
        self.components.iter().find(|component| component.id == id)
    }
}
