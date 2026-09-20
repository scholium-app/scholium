//! 夹具子模块。

use super::*;

// ---- 1 表格 ----

pub(crate) fn make_longtable(host: Dialect) -> Project {
    project(
        "longtable",
        host,
        vec![
            heading("跨页表格"),
            para("下表列出全部测量点，用于验证正文与跨页表格的混用。"),
            long_table("tab:long"),
            para("见表"),
            reference("tab:long", false),
            para("（第"),
            reference("tab:long", true),
            para("页）。"),
        ],
    )
}

pub(crate) fn make_bad_columns(host: Dialect) -> Project {
    let mut table = long_table("tab:bad");
    if let Block::Table(spec) = &mut table {
        spec.rows.push(vec!["缺列".to_string()]);
    }
    project(
        "bad-columns",
        host,
        vec![
            heading("列数不匹配"),
            table,
            para("该表应当在计划阶段被拒绝。"),
        ],
    )
}

// ---- 2 公式 ----

pub(crate) fn mass_energy() -> Math {
    seq(vec![
        ident("E"),
        sym("approx"),
        sup(ident("m"), num(2)),
        sym("cdot"),
        sup(ident("c"), num(2)),
    ])
}

pub(crate) fn integral() -> Math {
    seq(vec![
        sup(sub(sym("int"), num(0)), num(1)),
        sup(ident("x"), num(2)),
        ident("d"),
        ident("x"),
        sym("to"),
        frac(num(1), num(3)),
    ])
}

pub(crate) fn make_equations(host: Dialect) -> Project {
    project(
        "equations",
        host,
        vec![
            heading("编号公式"),
            para("行内公式"),
            Block::Equation {
                label: "eq:inline".to_string(),
                math: sqrt(frac(num(1), num(2))),
                display: false,
            },
            para("与独立公式："),
            equation("eq:mass", mass_energy()),
            para("式"),
            reference("eq:mass", false),
            para("给出质能关系；第二个公式见式"),
            reference("eq:int", false),
            para("（第"),
            reference("eq:int", true),
            para("页）："),
            equation("eq:int", integral()),
        ],
    )
}

pub(crate) fn make_unsupported_math(host: Dialect) -> Project {
    project(
        "unsupported-math",
        host,
        vec![
            heading("不受支持的公式命令"),
            equation("eq:bad", call("notacommand", vec![frac(num(1), num(2))])),
            para("该公式应当在计划阶段被拒绝。"),
        ],
    )
}

// ---- 3 图表 ----

pub(crate) fn make_native_plot(host: Dialect) -> Project {
    project(
        "native-plot",
        host,
        vec![
            heading("宿主原生绘图"),
            Block::Figure {
                label: "fig:native".to_string(),
                caption: "宿主原生绘图".to_string(),
                plot: wave(),
            },
            para("图"),
            reference("fig:native", false),
            para("在第"),
            reference("fig:native", true),
            para("页。"),
        ],
    )
}

pub(crate) fn plot_component(host: Dialect) -> Component {
    Component {
        id: "g2-draw".to_string(),
        dialect: opposite(host),
        scope: "g2".to_string(),
        body: vec![Block::Figure {
            label: format!("{}:plot", opposite(host).name()),
            caption: "外语绘图".to_string(),
            plot: wave(),
        }],
        bridge: Bridge::Vector,
        placement: Placement::Block,
        depends_on: Vec::new(),
    }
}

pub(crate) fn make_foreign_plot(host: Dialect) -> Project {
    let mut project = project(
        "foreign-plot",
        host,
        vec![
            heading("跨引擎矢量嵌入"),
            Block::ForeignFigure {
                label: "fig:foreign".to_string(),
                caption: "外语组件绘图".to_string(),
                component: "g2-draw".to_string(),
            },
            para("图"),
            reference("fig:foreign", false),
            para("在第"),
            reference("fig:foreign", true),
            para("页。"),
        ],
    );
    project.components.push(plot_component(host));
    project
}

pub(crate) fn make_multipage_component(host: Dialect) -> Project {
    let mut component = plot_component(host);
    component.body = vec![
        Block::Para("第一页内容".repeat(40)),
        Block::PageBreak,
        Block::Para("第二页内容".repeat(40)),
    ];
    let mut project = project(
        "multipage-component",
        host,
        vec![
            heading("跨页外语组件"),
            Block::ForeignFigure {
                label: "fig:big".to_string(),
                caption: "跨页组件".to_string(),
                component: "g2-draw".to_string(),
            },
        ],
    );
    project.components.push(component);
    project
}
