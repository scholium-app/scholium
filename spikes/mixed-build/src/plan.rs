//! 转换/依赖计划：把混合项目校验成一份可执行计划，或在动手前明确拒绝。
//!
//! 这一层对应 [混合源码与团队编辑](../../../../docs/MIXED_SOURCE_EDITING.md) 第 7 节的
//! `转换/依赖计划`：能力矩阵之外的组合必须在这里被挡住，不能进入编译后静默产出错误结果。

use std::collections::BTreeMap;

use crate::diag::Diagnostic;
use crate::ir::{Bridge, Block, Dialect, MacroDecl, Placement, Project, SymbolInfo, SymbolKind};

/// 校验通过后的可执行计划。
pub(crate) struct Plan {
    /// 宿主方言。
    pub(crate) host: Dialect,
    /// 符号表（引用目标 → 归属）。
    pub(crate) symbols: BTreeMap<String, SymbolInfo>,
    /// 宏表：宏名 → 各作用域下的声明（同名宏可以在不同作用域各自存在）。
    pub(crate) macros: BTreeMap<String, Vec<MacroDecl>>,
    /// 人类可读的构建步骤，作为证据打印。
    pub(crate) steps: Vec<String>,
}

/// 宿主正文所在作用域名。
pub(crate) const HOST_SCOPE: &str = "host";

/// 校验项目并产出计划；有任何不支持的组合就返回全部诊断。
#[allow(clippy::too_many_lines)] // 校验规则清单，拆开反而难与文档逐条对照
pub(crate) fn plan(project: &Project) -> Result<Plan, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let mut symbols: BTreeMap<String, SymbolInfo> = BTreeMap::new();
    let mut macros: BTreeMap<String, Vec<MacroDecl>> = BTreeMap::new();

    // ---- 作用域与组件结构 ----
    let mut scopes: BTreeMap<String, &str> = BTreeMap::new();
    for component in &project.components {
        if let Some(previous) = scopes.insert(component.scope.clone(), &component.id) {
            problems.push(Diagnostic::new(
                "scope-conflict",
                format!(
                    "组件 `{}` 与 `{}` 声明了同一个作用域 `{}`；同名作用域会让宏互相可见",
                    component.id, previous, component.scope
                ),
            ));
        }
        match (&component.bridge, component.dialect == project.host) {
            (Bridge::Include, false) => problems.push(Diagnostic::new(
                "bridge-dialect-mismatch",
                format!(
                    "组件 `{}` 是 {} 源码，却声明了 include 桥接；include 只用于同方言",
                    component.id,
                    component.dialect.name()
                ),
            )),
            (Bridge::Vector, true) => problems.push(Diagnostic::new(
                "bridge-dialect-mismatch",
                format!(
                    "组件 `{}` 与宿主同方言，却声明了 vector 桥接；这会无谓丢掉语义",
                    component.id
                ),
            )),
            (Bridge::Macro { .. }, true) => problems.push(Diagnostic::new(
                "bridge-dialect-mismatch",
                format!("组件 `{}` 与宿主同方言，不需要宏桥接", component.id),
            )),
            _ => {}
        }
        if component.placement == Placement::Inline
            && matches!(component.bridge, Bridge::Macro { .. })
        {
            problems.push(
                Diagnostic::new(
                    "inline-embed-unsupported",
                    format!(
                        "组件 `{}` 要求行内嵌入，但当前受支持载体只有块级矢量嵌入",
                        component.id
                    ),
                )
                .hint("行内混用需要转换 IR 或受支持的行内载体；本 spike 未实现"),
            );
        }
        for dependency in &component.depends_on {
            if project.component(dependency).is_none() {
                problems.push(Diagnostic::new(
                    "component-missing",
                    format!("组件 `{}` 依赖不存在的组件 `{dependency}`", component.id),
                ));
            }
        }
    }
    check_inline_content(project, &mut problems);
    detect_cycles(project, &mut problems);

    // ---- 符号表 ----
    for (owner, block) in project.all_blocks() {
        let Some(label) = block.label() else { continue };
        let kind = match block {
            Block::Table(_) => SymbolKind::Table,
            Block::Equation { .. } => SymbolKind::Equation,
            Block::Figure { .. } | Block::ForeignFigure { .. } => SymbolKind::Figure,
            _ => SymbolKind::Section,
        };
        let scope = match project.component(owner) {
            Some(component) => component.scope.clone(),
            None => HOST_SCOPE.to_string(),
        };
        let info = SymbolInfo {
            id: label.to_string(),
            kind,
            owner: owner.to_string(),
            scope,
        };
        if let Some(previous) = symbols.get(label) {
            problems.push(Diagnostic::new(
                "symbol-duplicate",
                format!(
                    "符号 `{label}` 在 `{}` 与 `{}` 中重复定义",
                    previous.owner, owner
                ),
            ));
        } else {
            symbols.insert(label.to_string(), info);
        }
    }

    // ---- 宏表：按 (名字, 作用域) 唯一 ----
    for decl in &project.macros {
        let bucket = macros.entry(decl.name.clone()).or_default();
        if let Some(previous) = bucket
            .iter()
            .find(|existing| existing.scope == decl.scope)
        {
            problems.push(Diagnostic::new(
                "scope-conflict",
                format!(
                    "作用域 `{}` 内宏 `{}` 被 `{}` 与 `{}` 重复定义，最后一个会静默覆盖",
                    decl.scope, decl.name, previous.owner, decl.owner
                ),
            ));
        } else {
            bucket.push(decl.clone());
        }
    }

    // ---- 逐文件检查 ----
    check_blocks(
        project,
        &symbols,
        &macros,
        "host",
        HOST_SCOPE,
        project.host,
        true,
        &project.body,
        &mut problems,
    );
    for component in &project.components {
        check_blocks(
            project,
            &symbols,
            &macros,
            &component.id,
            &component.scope,
            component.dialect,
            false,
            &component.body,
            &mut problems,
        );
    }

    if !problems.is_empty() {
        return Err(problems);
    }

    let mut steps = vec![format!("宿主 = {}（{}）", project.host.name(), project.name)];
    for component in &project.components {
        let bridge = match &component.bridge {
            Bridge::Macro { contract } => format!("macro（合约 {contract}）"),
            other => other.name().to_string(),
        };
        steps.push(format!(
            "组件 {} = {} 源码 / {} 桥接 / 作用域 {}",
            component.id,
            component.dialect.name(),
            bridge,
            component.scope
        ));
    }
    let symbol_list = symbols
        .values()
        .map(|symbol| format!("{}:{}@{}/{}", symbol.id, symbol.kind.name(), symbol.owner, symbol.scope))
        .collect::<Vec<_>>()
        .join(", ");
    steps.push(format!("符号表 {symbol_list}"));
    steps.push(format!(
        "符号 {} 个，宏 {} 个，引用轮数上限由构建循环控制",
        symbols.len(),
        macros.values().map(Vec::len).sum::<usize>()
    ));
    Ok(Plan {
        host: project.host,
        symbols,
        macros,
        steps,
    })
}

fn check_inline_content(project: &Project, problems: &mut Vec<Diagnostic>) {
    for component in &project.components {
        if component.placement != Placement::Inline
            || matches!(component.bridge, Bridge::Macro { .. })
        {
            continue;
        }
        let supported = component.body.iter().all(|block| matches!(block,
            Block::Equation { display: false, .. } | Block::Ref { .. })
            || matches!(block, Block::Para(text) if !text.contains(['\n', '\r'])));
        if !supported {
            problems.push(Diagnostic::new("inline-content-unsupported",
                format!("行内组件 `{}` 含块级或未验证内容；仅支持单行文本、行内公式与引用", component.id)));
        }
    }
}

/// 对全部块做逐条校验。
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)] // 逐类内容校验清单，保持一处可读
fn check_blocks(
    project: &Project,
    symbols: &BTreeMap<String, SymbolInfo>,
    macros: &BTreeMap<String, Vec<MacroDecl>>,
    owner: &str,
    scope: &str,
    dialect: Dialect,
    is_host: bool,
    blocks: &[Block],
    problems: &mut Vec<Diagnostic>,
) {
    for block in blocks {
        match block {
            Block::Table(spec) => {
                if spec.header.len() != spec.columns {
                    problems.push(Diagnostic::new(
                        "table-columns",
                        format!(
                            "`{}` 的表头有 {} 格，但声明了 {} 列",
                            spec.label,
                            spec.header.len(),
                            spec.columns
                        ),
                    ));
                }
                for (index, row) in spec.rows.iter().enumerate() {
                    if row.len() != spec.columns {
                        problems.push(
                            Diagnostic::new(
                                "table-columns",
                                format!(
                                    "`{}` 第 {} 行有 {} 格，声明列数为 {}",
                                    spec.label,
                                    index + 1,
                                    row.len(),
                                    spec.columns
                                ),
                            )
                            .hint("列数不匹配会让表格静默错位，必须在计划阶段拒绝"),
                        );
                    }
                }
            }
            Block::Equation { label, math, .. } => {
                if let Some(problem) = math.unsupported() {
                    problems.push(problem.hint(format!("公式 `{label}` 无法源生成为宿主源码")));
                }
            }
            Block::ForeignFigure { label, component, .. } => match project.component(component) {
                None => problems.push(Diagnostic::new(
                    "component-missing",
                    format!("`{label}` 要求嵌入不存在的组件 `{component}`"),
                )),
                Some(found) => {
                    if !matches!(found.bridge, Bridge::Vector) {
                        problems.push(Diagnostic::new(
                            "foreign-figure-bridge",
                            format!(
                                "`{label}` 要求矢量嵌入组件 `{component}`，但该组件声明的是 {} 桥接",
                                found.bridge.name()
                            ),
                        ));
                    }
                }
            },
            Block::MacroUse { name, args } => match resolve_macro(macros, name, scope) {
                None => problems.push(
                    Diagnostic::new(
                        "macro-undeclared",
                        format!("`{owner}` 使用了未声明的宏 `{name}`"),
                    )
                    .hint("宏必须先声明定义方与（跨语言时的）桥接合约"),
                ),
                Some(decl) => {
                    if args.len() != decl.params {
                        problems.push(Diagnostic::new(
                            "macro-arity",
                            format!(
                                "宏 `{name}` 需要 {} 个参数，`{owner}` 传了 {} 个",
                                decl.params,
                                args.len()
                            ),
                        ));
                    }
                    if decl.dialect != dialect && decl.contract.is_none() {
                        problems.push(
                            Diagnostic::new(
                                "macro-bridge-missing",
                                format!(
                                    "`{owner}` 是 {} 源码，却直接调用了 {} 宏 `{name}`，且没有声明桥接合约",
                                    dialect.name(),
                                    decl.dialect.name()
                                ),
                            )
                            .hint("跨引擎调用必须有声明的适配合约，宏不会自动互换"),
                        );
                    } else if !is_host
                        && decl.scope != scope
                        && decl.contract.is_none()
                        && !project.unscoped_control
                    {
                        problems.push(
                            Diagnostic::new(
                                "scope-leak",
                                format!(
                                    "组件作用域 `{scope}` 使用了作用域 `{}` 的宏 `{name}`；作用域之间不得互相泄漏",
                                    decl.scope
                                ),
                            )
                            .hint("要么把宏移入本作用域，要么声明跨语言/跨作用域桥接合约"),
                        );
                    } else if is_host
                        && let Some(component) = project.component(&decl.owner)
                        && matches!(component.bridge, Bridge::Vector)
                        && decl.contract.is_none()
                    {
                        problems.push(Diagnostic::new(
                            "macro-vector-boundary",
                            format!(
                                "宿主使用了组件 `{}` 的宏 `{name}`，但该组件是矢量嵌入，宏边界不可穿越",
                                component.id
                            ),
                        ));
                    }
                }
            },
            Block::Ref { target, .. } => {
                if !symbols.contains_key(target) {
                    problems.push(
                        Diagnostic::new(
                            "ref-dangling",
                            format!("`{owner}` 引用了未定义的符号 `{target}`"),
                        )
                        .hint("悬空引用会在产物里留下 `??`，必须在计划阶段拒绝"),
                    );
                }
            }
            Block::Raw { dialect: raw, text } => {
                if *raw != dialect {
                    let code = if is_host {
                        "raw-foreign-unrouted"
                    } else {
                        "raw-dialect-mismatch"
                    };
                    problems.push(
                        Diagnostic::new(
                            code,
                            format!(
                                "`{owner}` 是 {} 文件，却直接内联了 {} 原文：`{}`",
                                dialect.name(),
                                raw.name(),
                                text.chars().take(32).collect::<String>()
                            ),
                        )
                        .hint("外语原文必须提升为声明了桥接的组件，否则宿主编译器会拒绝或误解它"),
                    );
                }
            }
            Block::FeedbackSpace { probe, .. } => {
                if !symbols.contains_key(probe) {
                    problems.push(Diagnostic::new(
                        "ref-dangling",
                        format!("反馈块引用了未定义的探测符号 `{probe}`"),
                    ));
                }
            }
            Block::Figure { .. } | Block::IncludeSection { .. } => {}
            Block::Heading { .. } | Block::Para(_) | Block::PageBreak => {}
        }
    }
}

/// 解析宏调用：同作用域优先，其次带桥接合约的定义。
pub(crate) fn resolve_macro<'a>(
    macros: &'a BTreeMap<String, Vec<MacroDecl>>,
    name: &str,
    scope: &str,
) -> Option<&'a MacroDecl> {
    let candidates = macros.get(name)?;
    if let Some(found) = candidates.iter().find(|decl| decl.scope == scope) {
        return Some(found);
    }
    if let Some(found) = candidates.iter().find(|decl| decl.contract.is_some()) {
        return Some(found);
    }
    candidates.first()
}

/// 依赖图环路检测。
fn detect_cycles(project: &Project, problems: &mut Vec<Diagnostic>) {
    fn visit(
        project: &Project,
        id: &str,
        stack: &mut Vec<String>,
        done: &mut Vec<String>,
        problems: &mut Vec<Diagnostic>,
    ) {
        if done.iter().any(|seen| seen == id) {
            return;
        }
        if stack.iter().any(|seen| seen == id) {
            stack.push(id.to_string());
            problems.push(Diagnostic::new(
                "component-cycle",
                format!("组件依赖成环：{}", stack.join(" → ")),
            ));
            stack.pop();
            return;
        }
        stack.push(id.to_string());
        if let Some(component) = project.component(id) {
            for dependency in &component.depends_on {
                visit(project, dependency, stack, done, problems);
            }
        }
        stack.pop();
        done.push(id.to_string());
    }
    let mut done = Vec::new();
    for component in &project.components {
        visit(project, &component.id, &mut Vec::new(), &mut done, problems);
    }
}
