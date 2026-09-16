//! 夹具集合：六类混用内容各有成功夹具与失败夹具，逐条对应报告里的判据。
//!
//! 每个夹具都能按宿主实例化（`make(host)`），因此"两种宿主都要做"不是靠两个手写项目，
//! 而是同一份 IR 在两个宿主上各自源生成为真实源码。

use crate::diag::Expect;
use crate::ir::{
    Block, Bridge, Component, Dialect, MacroDecl, MacroKind, Math, Placement, PlotSpec, Project,
    TableSpec,
};

mod content;
mod macros_scope;
mod refs;

pub(crate) use content::*;
pub(crate) use macros_scope::*;
pub(crate) use refs::*;

/// 一个夹具的定义。
pub(crate) struct Fixture {
    /// 夹具 id。
    pub(crate) id: &'static str,
    /// 类别（对应六类混用之一，或附加项）。
    pub(crate) category: &'static str,
    /// 期望结果。
    pub(crate) expect: Expect,
    /// 设计意图（打印用）。
    pub(crate) note: &'static str,
    /// 支持的宿主。
    pub(crate) hosts: &'static [Dialect],
    /// 按宿主构造项目。
    pub(crate) make: fn(Dialect) -> Project,
    /// 是否期望检测到作用域泄漏（对照实现专用）。
    pub(crate) leak_expected: bool,
    /// 失败夹具必须命中的诊断码（成功夹具为空）。
    pub(crate) expect_codes: &'static [&'static str],
}

/// 两种宿主。
const BOTH: &[Dialect] = &[Dialect::Latex, Dialect::Typst];
/// 只有 LaTeX 宿主的真实编译失败夹具。
const LATEX_ONLY: &[Dialect] = &[Dialect::Latex];
/// 只有 Typst 宿主的引擎级不收敛夹具。
const TYPST_ONLY: &[Dialect] = &[Dialect::Typst];

/// 默认页面尺寸。
const PAGE: (f64, f64) = (420.0, 720.0);

fn project(name: &str, host: Dialect, body: Vec<Block>) -> Project {
    Project {
        name: name.to_string(),
        host,
        body,
        macros: Vec::new(),
        components: Vec::new(),
        page: PAGE,
        unscoped_control: false,
    }
}

fn para(text: &str) -> Block {
    Block::Para(text.to_string())
}

fn heading(text: &str) -> Block {
    Block::Heading {
        level: 1,
        text: text.to_string(),
    }
}

fn equation(label: &str, math: Math) -> Block {
    Block::Equation {
        label: label.to_string(),
        math,
        display: true,
    }
}

fn reference(target: &str, page: bool) -> Block {
    Block::Ref {
        target: target.to_string(),
        page,
    }
}

fn macro_use(name: &str, args: &[&str]) -> Block {
    Block::MacroUse {
        name: name.to_string(),
        args: args.iter().map(|arg| arg.to_string()).collect(),
    }
}

fn frac(a: Math, b: Math) -> Math {
    Math::Frac(Box::new(a), Box::new(b))
}

fn sup(a: Math, b: Math) -> Math {
    Math::Sup(Box::new(a), Box::new(b))
}

fn sub(a: Math, b: Math) -> Math {
    Math::Sub(Box::new(a), Box::new(b))
}

fn sqrt(a: Math) -> Math {
    Math::Sqrt(Box::new(a))
}

fn sym(name: &'static str) -> Math {
    Math::Sym(name)
}

fn ident(name: &str) -> Math {
    Math::Ident(name.to_string())
}

fn num(value: u32) -> Math {
    Math::Num(value)
}

fn call(name: &str, args: Vec<Math>) -> Math {
    Math::Call {
        name: name.to_string(),
        args,
    }
}

fn seq(items: Vec<Math>) -> Math {
    Math::Seq(items)
}

/// 折线图数据（可复现）。
fn wave() -> PlotSpec {
    PlotSpec {
        points: (0..=10)
            .map(|index| {
                let x = f64::from(index);
                (x, (x * 0.6).sin() * 2.0 + 3.0)
            })
            .collect(),
    }
}

/// 跨页表格：表头 + 数据行 + 填充行。
fn long_table(label: &str) -> Block {
    Block::Table(TableSpec {
        label: label.to_string(),
        caption: "跨页测量表".to_string(),
        columns: 3,
        header: vec!["编号".to_string(), "名称".to_string(), "数值".to_string()],
        rows: (1..=8)
            .map(|index| {
                vec![
                    format!("{index}"),
                    format!("样本-{index}"),
                    format!("{}", index * 7),
                ]
            })
            .collect(),
        filler: 80,
    })
}

/// 全部夹具。
#[allow(clippy::too_many_lines)] // 夹具清单是数据表
pub(crate) fn all() -> Vec<Fixture> {
    vec![
        Fixture {
            id: "T1-longtable",
            category: "1 正文+跨页表格",
            expect: Expect::Success,
            note: "同一份 IR 在两种宿主各自生成跨页表格，表头在续页重复",
            hosts: BOTH,
            make: make_longtable,
            leak_expected: false,
            expect_codes: &[],
        },
        Fixture {
            id: "T1-bad-cols",
            category: "1 正文+跨页表格",
            expect: Expect::Rejected,
            note: "行格数与声明列数不符，必须在计划阶段拒绝而不是静默错位",
            hosts: BOTH,
            make: make_bad_columns,
            leak_expected: false,
            expect_codes: &["table-columns"],
        },
        Fixture {
            id: "E1-equation",
            category: "2 公式",
            expect: Expect::Success,
            note: "行内公式 + 两个编号公式 + 编号引用与页码引用",
            hosts: BOTH,
            make: make_equations,
            leak_expected: false,
            expect_codes: &[],
        },
        Fixture {
            id: "E1-unsupported",
            category: "2 公式",
            expect: Expect::Rejected,
            note: "公式含受支持子集之外的命令，翻译器必须拒绝",
            hosts: BOTH,
            make: make_unsupported_math,
            leak_expected: false,
            expect_codes: &["math-call-unsupported"],
        },
        Fixture {
            id: "G1-plot-native",
            category: "3 图表",
            expect: Expect::Success,
            note: "宿主原生绘图：LaTeX 用 pgfplots，Typst 用 curve",
            hosts: BOTH,
            make: make_native_plot,
            leak_expected: false,
            expect_codes: &[],
        },
        Fixture {
            id: "G2-plot-foreign",
            category: "3 图表",
            expect: Expect::Success,
            note: "跨引擎矢量嵌入：LaTeX 宿主嵌 Typst 绘图，反之嵌 pgfplots",
            hosts: BOTH,
            make: make_foreign_plot,
            leak_expected: false,
            expect_codes: &[],
        },
        Fixture {
            id: "G3-foreign-multipage",
            category: "3 图表",
            expect: Expect::Rejected,
            note: "外语组件跨页，超出现有单页矢量嵌入载体能力，必须阻断",
            hosts: BOTH,
            make: make_multipage_component,
            leak_expected: false,
            expect_codes: &["component-multipage-unsupported"],
        },
        Fixture {
            id: "M1-macro",
            category: "4 自定义宏",
            expect: Expect::Success,
            note: "宿主自有宏 + 跨语言宏按声明合约在宿主侧重实现",
            hosts: BOTH,
            make: make_macros,
            leak_expected: false,
            expect_codes: &[],
        },
        Fixture {
            id: "M1-undeclared",
            category: "4 自定义宏",
            expect: Expect::Rejected,
            note: "使用未声明的宏必须被计划拒绝",
            hosts: BOTH,
            make: make_undeclared_macro,
            leak_expected: false,
            expect_codes: &["macro-undeclared"],
        },
        Fixture {
            id: "M2-recursive",
            category: "4 自定义宏",
            expect: Expect::Rejected,
            note: "递归宏由真实 TeX 编译失败兜住（TeX capacity exceeded）",
            hosts: LATEX_ONLY,
            make: make_recursive_macro,
            leak_expected: false,
            expect_codes: &["host-compile-failed"],
        },
        Fixture {
            id: "S1-scope",
            category: "5 模板作用域",
            expect: Expect::Success,
            note: "两个组件作用域定义同名宏且取值不同，互不泄漏",
            hosts: BOTH,
            make: make_scopes,
            leak_expected: false,
            expect_codes: &[],
        },
        Fixture {
            id: "S1-scope-control",
            category: "5 模板作用域",
            expect: Expect::Success,
            note: "对照实现：关闭作用域隔离后同名宏互相覆盖，应当检测到泄漏",
            hosts: BOTH,
            make: make_scopes_unscoped,
            leak_expected: true,
            expect_codes: &[],
        },
        Fixture {
            id: "S1-leak",
            category: "5 模板作用域",
            expect: Expect::Rejected,
            note: "跨作用域使用他人宏且无合约，必须被计划拒绝",
            hosts: BOTH,
            make: make_scope_leak,
            leak_expected: false,
            expect_codes: &["scope-leak"],
        },
        Fixture {
            id: "S1-conflict",
            category: "5 模板作用域",
            expect: Expect::Rejected,
            note: "同一作用域内同名宏重复定义必须被拒绝",
            hosts: BOTH,
            make: make_scope_conflict,
            leak_expected: false,
            expect_codes: &["scope-conflict"],
        },
        Fixture {
            id: "R1-refs",
            category: "6 双向引用/页码/链接",
            expect: Expect::Success,
            note: "宿主↔外语组件双向引用、最终页码与链接（多轮解析）",
            hosts: BOTH,
            make: make_refs,
            leak_expected: false,
            expect_codes: &[],
        },
        Fixture {
            id: "R1-dangling",
            category: "6 双向引用/页码/链接",
            expect: Expect::Rejected,
            note: "悬空引用（未定义符号）必须被拒绝，不允许留下 ??",
            hosts: BOTH,
            make: make_dangling_ref,
            leak_expected: false,
            expect_codes: &["ref-dangling"],
        },
        Fixture {
            id: "R1-raw-foreign",
            category: "6 双向引用/页码/链接",
            expect: Expect::Rejected,
            note: "宿主里内联外语原文而未提升为组件，必须被拒绝",
            hosts: BOTH,
            make: make_raw_foreign,
            leak_expected: false,
            expect_codes: &["raw-foreign-unrouted"],
        },
        Fixture {
            id: "OSC-1-feedback",
            category: "附加：引用振荡上限",
            expect: Expect::Rejected,
            note: "页码反馈改变版面高度，形成无不动点的振荡；必须检测并停止",
            hosts: BOTH,
            make: make_oscillation,
            leak_expected: false,
            expect_codes: &["reference-oscillation"],
        },
        Fixture {
            id: "OSC-2-engine",
            category: "附加：引用振荡上限",
            expect: Expect::Rejected,
            note: "Typst 引擎自身 5 次内省后不收敛，必须当作硬失败",
            hosts: TYPST_ONLY,
            make: make_engine_divergence,
            leak_expected: false,
            expect_codes: &["engine-non-convergence"],
        },
    ]
}

fn opposite(dialect: Dialect) -> Dialect {
    match dialect {
        Dialect::Latex => Dialect::Typst,
        Dialect::Typst => Dialect::Latex,
    }
}
