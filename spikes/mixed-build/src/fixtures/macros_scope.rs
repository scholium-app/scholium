//! 夹具子模块。

use super::*;

// ---- 4 宏 ----

pub(crate) fn make_macros(host: Dialect) -> Project {
    let mut project = project(
        "macros",
        host,
        vec![
            heading("自定义宏"),
            para("宿主自有宏："),
            macro_use("doubles", &["21"]),
            para("；跨语言宏："),
            macro_use("triple", &["14"]),
            para("。"),
        ],
    );
    project.macros.push(MacroDecl {
        name: "doubles".to_string(),
        scope: "host".to_string(),
        owner: "host".to_string(),
        dialect: host,
        kind: MacroKind::Multiply { factor: 2 },
        params: 1,
        contract: None,
    });
    project.macros.push(MacroDecl {
        name: "triple".to_string(),
        scope: "m-lib".to_string(),
        owner: "m-lib".to_string(),
        dialect: opposite(host),
        kind: MacroKind::Multiply { factor: 3 },
        params: 1,
        contract: Some("int-multiply-v1".to_string()),
    });
    project.components.push(Component {
        id: "m-lib".to_string(),
        dialect: opposite(host),
        scope: "m-lib".to_string(),
        body: vec![
            heading("外语宏库"),
            para("外语侧结果："),
            macro_use("triple", &["14"]),
        ],
        bridge: Bridge::Macro {
            contract: "int-multiply-v1".to_string(),
        },
        placement: Placement::Block,
        depends_on: Vec::new(),
    });
    project
}

pub(crate) fn make_undeclared_macro(host: Dialect) -> Project {
    project(
        "undeclared-macro",
        host,
        vec![
            heading("未声明的宏"),
            macro_use("mystery", &["1"]),
            para("该调用应当在计划阶段被拒绝。"),
        ],
    )
}

pub(crate) fn make_recursive_macro(host: Dialect) -> Project {
    project(
        "recursive-macro",
        host,
        vec![
            heading("递归宏"),
            Block::Raw {
                dialect: host,
                text: "\\def\\loopmacro{\\loopmacro\\loopmacro}\\loopmacro".to_string(),
            },
            para("该文档应当由真实的 TeX 编译失败兜住。"),
        ],
    )
}

// ---- 5 作用域 ----

pub(crate) fn scope_component(id: &str) -> Component {
    Component {
        id: id.to_string(),
        dialect: Dialect::Latex,
        scope: id.to_string(),
        body: vec![para("作用域取值："), macro_use("mark", &[])],
        bridge: Bridge::Include,
        placement: Placement::Block,
        depends_on: Vec::new(),
    }
}

pub(crate) fn scope_body() -> Vec<Block> {
    vec![
        heading("模板作用域"),
        Block::IncludeSection {
            component: "s-a".to_string(),
        },
        para("分隔"),
        Block::IncludeSection {
            component: "s-b".to_string(),
        },
    ]
}

/// 两个作用域各自定义并使用同名宏。
pub(crate) fn make_scopes(host: Dialect) -> Project {
    let mut project = project("scopes", host, scope_body());
    for (id, value) in [("s-a", "取值-A"), ("s-b", "取值-B")] {
        let mut component = scope_component(id);
        component.dialect = host;
        component.body = vec![para("作用域取值："), macro_use("mark", &[])];
        project.components.push(component);
        project.macros.push(MacroDecl {
            name: "mark".to_string(),
            scope: id.to_string(),
            owner: id.to_string(),
            dialect: host,
            kind: MacroKind::Constant {
                value: value.to_string(),
            },
            params: 0,
            contract: None,
        });
    }
    project
}

/// 对照实现：`s-b` **不定义**自己的 `mark`，只使用。
/// 关闭作用域隔离后它会静默取到 `s-a` 的值——这就是必须被计划拦住的泄漏。
pub(crate) fn make_scopes_control(host: Dialect, unscoped: bool) -> Project {
    let mut project = project("scope-control", host, scope_body());
    project.unscoped_control = unscoped;
    let mut component_a = scope_component("s-a");
    component_a.dialect = host;
    component_a.body = vec![para("作用域取值："), macro_use("mark", &[])];
    project.components.push(component_a);
    let mut component_b = scope_component("s-b");
    component_b.dialect = host;
    component_b.body = vec![para("作用域取值："), macro_use("mark", &[])];
    project.components.push(component_b);
    project.macros.push(MacroDecl {
        name: "mark".to_string(),
        scope: "s-a".to_string(),
        owner: "s-a".to_string(),
        dialect: host,
        kind: MacroKind::Constant {
            value: "取值-A".to_string(),
        },
        params: 0,
        contract: None,
    });
    project
}

pub(crate) fn make_scopes_unscoped(host: Dialect) -> Project {
    make_scopes_control(host, true)
}

pub(crate) fn make_scope_leak(host: Dialect) -> Project {
    make_scopes_control(host, false)
}

pub(crate) fn make_scope_conflict(host: Dialect) -> Project {
    let mut project = make_scopes(host);
    project.macros.push(MacroDecl {
        name: "mark".to_string(),
        scope: "s-a".to_string(),
        owner: "s-a".to_string(),
        dialect: host,
        kind: MacroKind::Constant {
            value: "重复定义".to_string(),
        },
        params: 0,
        contract: None,
    });
    project
}
