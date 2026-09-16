//! 夹具子模块。

use super::*;

// ---- 6 引用 ----

pub(crate) fn make_refs(host: Dialect) -> Project {
    let mut project = project(
        "refs",
        host,
        vec![
            heading("双向引用"),
            equation("eq:host", mass_energy()),
            para("宿主公式编号"),
            reference("eq:host", false),
            para("，位于第"),
            reference("eq:host", true),
            para("页；外语组件中的公式编号"),
            reference("eq:foreign", false),
            para("，位于第"),
            reference("eq:foreign", true),
            para("页。"),
            Block::ForeignFigure {
                label: "fig:bridge".to_string(),
                caption: "承载外语公式的组件".to_string(),
                component: "r-lib".to_string(),
            },
            para("外语组件内部引用宿主公式编号"),
            reference("eq:host", false),
            para("。"),
        ],
    );
    project.components.push(Component {
        id: "r-lib".to_string(),
        dialect: opposite(host),
        scope: "r-lib".to_string(),
        body: vec![
            equation("eq:foreign", frac(num(1), num(7))),
            para("外语组件引用宿主："),
            reference("eq:host", false),
        ],
        bridge: Bridge::Vector,
        placement: Placement::Block,
        depends_on: Vec::new(),
    });
    project
}

pub(crate) fn make_dangling_ref(host: Dialect) -> Project {
    project(
        "dangling-ref",
        host,
        vec![
            heading("悬空引用"),
            para("引用一个不存在的符号："),
            reference("eq:missing", false),
        ],
    )
}

pub(crate) fn make_raw_foreign(host: Dialect) -> Project {
    project(
        "raw-foreign",
        host,
        vec![
            heading("内联外语原文"),
            Block::Raw {
                dialect: opposite(host),
                text: match opposite(host) {
                    Dialect::Latex => "\\begin{equation}x=1\\end{equation}".to_string(),
                    Dialect::Typst => "$ x = 1 $".to_string(),
                },
            },
            para("该原文应当在计划阶段被拒绝。"),
        ],
    )
}

// ---- 附加：振荡 ----

pub(crate) fn make_oscillation(host: Dialect) -> Project {
    let mut project = project(
        "oscillation",
        host,
        vec![
            heading("引用反馈振荡"),
            para("本夹具故意让版面高度依赖上一轮解析出的页码，形成无不动点的反馈。"),
            Block::FeedbackSpace {
                probe: "pg:probe".to_string(),
                base: 900.0,
                slope: 300.0,
            },
            equation("pg:probe", num(1)),
        ],
    );
    project.page = (400.0, 596.0);
    project
}

pub(crate) fn make_engine_divergence(host: Dialect) -> Project {
    project(
        "engine-divergence",
        host,
        vec![
            Block::Raw {
                dialect: Dialect::Typst,
                text: "#set page(height: 100pt, margin: 10pt)\n#context {\n  \
                       let n = counter(page).final().first()\n  \
                       block(height: (150 - 50 * n) * 1pt)\n}"
                    .to_string(),
            },
            para("该文档应当由引擎的收敛上限兜住。"),
        ],
    )
}

