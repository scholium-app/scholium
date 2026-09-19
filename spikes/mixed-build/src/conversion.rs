//! Explicit semantic conversion. Never reinterpret a vector contract as source conversion.
use crate::{
    diag::Diagnostic,
    ir::{Block, Bridge, Project},
};

fn validate_blocks(project: &Project, problems: &mut Vec<Diagnostic>) {
    for (_, block) in project.all_blocks() {
        if let Block::Ref { target, .. } = block
            && project.all_blocks().any(|(_, b)| {
                matches!(b,
                Block::Equation { label, display: false, .. } if label == target)
            })
        {
            problems.push(Diagnostic::new(
                "inline-target-unsupported",
                format!("行内公式 `{target}` 未声明编号/页码目标，不能生成未解析引用"),
            ));
        }
        if let Some(label) = block.label()
            && (label.is_empty()
                || label.starts_with("sch:")
                || !label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b":._-".contains(&b)))
        {
            problems.push(Diagnostic::new(
                "symbol-invalid",
                format!("不安全或保留的符号 `{label}`"),
            ));
        }
        if let Block::IncludeSection { component } = block {
            let valid = project.component(component).is_some_and(|c| {
                matches!(c.bridge, Bridge::Include | Bridge::Convert)
                    || (matches!(c.bridge, Bridge::Vector)
                        && c.placement == crate::ir::Placement::Inline)
            });
            if !valid {
                problems.push(Diagnostic::new(
                    "include-bridge-invalid",
                    format!("组件 `{component}` 不是源码桥接"),
                ));
            }
        }
    }
}

pub(crate) fn validate(project: &Project, problems: &mut Vec<Diagnostic>) {
    validate_blocks(project, problems);
    validate_vector_placements(project, problems);
    for component in &project.components {
        if !matches!(component.bridge, Bridge::Convert) {
            continue;
        }
        if component.dialect == project.host {
            problems.push(Diagnostic::new(
                "bridge-dialect-mismatch",
                "同语言组件应使用 Include",
            ));
        }
        for (index, block) in component.body.iter().enumerate() {
            if !matches!(
                block,
                Block::Para(_)
                    | Block::Equation { .. }
                    | Block::Ref { .. }
                    | Block::Heading { .. }
                    | Block::PageBreak
                    | Block::Table(_)
            ) {
                problems.push(Diagnostic::new(
                    "conversion-unsupported",
                    format!(
                        "组件 `{}` 第 {} 个块不在显式转换子集中",
                        component.id,
                        index + 1
                    ),
                ));
            }
        }
        if project.macros.iter().any(|decl| decl.owner == component.id) {
            problems.push(Diagnostic::new(
                "conversion-unsupported",
                format!("组件 `{}` 含未验证宏声明", component.id),
            ));
        }
    }
}

fn validate_vector_placements(project: &Project, problems: &mut Vec<Diagnostic>) {
    for c in &project.components {
        if !matches!(c.bridge, Bridge::Vector) {
            continue;
        }
        let placements: Vec<_> = project
            .all_blocks()
            .filter_map(|(owner, b)| match b {
                Block::ForeignFigure { component, .. } | Block::IncludeSection { component }
                    if component == &c.id =>
                {
                    Some((owner, b))
                }
                _ => None,
            })
            .collect();
        let valid = placements.len() == 1
            && placements.iter().all(|(owner, block)| {
                crate::generate::native_owner(project, owner)
                    && (matches!(block, Block::ForeignFigure { .. })
                        == (c.placement == crate::ir::Placement::Block))
            });
        if !valid {
            problems.push(Diagnostic::new(
                "vector-placement-unsupported",
                format!(
                    "组件 `{}` 必须在宿主源码空间按声明的行内/块级方式放置一次",
                    c.id
                ),
            ));
        }
    }
}
