//! Spoken projection of structural math, beside (never inside) the editable text runs.
//!
//! Wording follows the Orca-validated stage-0 conventions (report 0042): formulas read as
//! `分数（分子：…；分母：…）`, slot context reads outside-in, and empty slots read as `空`.
use scholium_model::{
    NodeId,
    structured::{MathBody, MathNode},
};

/// Whole-formula description; empty slots are spoken, never skipped.
pub(super) fn describe(node: &MathNode) -> String {
    match &node.body {
        MathBody::Text { text } => text.clone(),
        MathBody::Symbol { symbol } => symbol.to_string(),
        MathBody::Hole => "空".into(),
        MathBody::Row { children } => children.iter().map(describe).collect::<Vec<_>>().join("，"),
        MathBody::Fraction {
            numerator,
            denominator,
        } => format!(
            "分数（分子：{}；分母：{}）",
            describe(numerator),
            describe(denominator)
        ),
        MathBody::Attach { base, sub, sup } => {
            let slots = [("下标", sub), ("上标", sup)]
                .into_iter()
                .filter_map(|(name, slot)| Some(format!("；{name}：{}", describe(slot.as_ref()?))));
            format!(
                "上下标（底数：{}{}）",
                describe(base),
                slots.collect::<String>()
            )
        }
    }
}

/// Outside-in slot path of `leaf` and its content, e.g. `分数分子，分数分母：空`.
///
/// A leaf directly in the formula (no slot ancestor) reads as `公式：…`; `None` when
/// `leaf` is not inside `root`.
pub(super) fn slot_context(root: &MathNode, leaf: NodeId) -> Option<String> {
    let mut path = Vec::new();
    let found = locate(root, leaf, &mut path)?;
    let content = describe(found);
    let content = if content.is_empty() {
        "空".into()
    } else {
        content
    };
    let path = if path.is_empty() {
        "公式".to_owned()
    } else {
        path.join("，")
    };
    Some(format!("{path}：{content}"))
}

/// The edge announcement is separate from editable Text offsets.
pub(super) fn caret_context(
    root: &MathNode,
    at: scholium_typst::editor::Position,
    locale: crate::i18n::Locale,
) -> Option<String> {
    let mut path = Vec::new();
    let node = locate(root, at.leaf, &mut path)?;
    let key = match (&node.body, at.byte == 0) {
        (MathBody::Fraction { .. }, true) => "a11y.math.edge.fraction.before",
        (MathBody::Fraction { .. }, false) => "a11y.math.edge.fraction.after",
        (MathBody::Attach { .. }, true) => "a11y.math.edge.attach.before",
        (MathBody::Attach { .. }, false) => "a11y.math.edge.attach.after",
        _ => return slot_context(root, at.leaf),
    };
    let edge = crate::i18n::text(locale, key);
    Some(if path.is_empty() {
        edge
    } else {
        crate::i18n::text_with(
            locale,
            "a11y.math.edge.nested",
            &[("path", path.join(", ").into()), ("edge", edge.into())],
        )
    })
}

fn locate<'a>(
    node: &'a MathNode,
    leaf: NodeId,
    path: &mut Vec<&'static str>,
) -> Option<&'a MathNode> {
    if node.node == leaf {
        return Some(node);
    }
    match &node.body {
        MathBody::Row { children } => children.iter().find_map(|c| locate(c, leaf, path)),
        MathBody::Fraction {
            numerator,
            denominator,
        } => in_slots(
            [(numerator, "分数分子"), (denominator, "分数分母")],
            leaf,
            path,
        ),
        MathBody::Attach { base, sub, sup } => in_slots(
            [
                (Some(base), "底数"),
                (sub.as_ref(), "下标"),
                (sup.as_ref(), "上标"),
            ]
            .into_iter()
            .filter_map(|(slot, name)| Some((slot?, name))),
            leaf,
            path,
        ),
        _ => None,
    }
}

/// First named slot containing `leaf`; its name stays on `path` only when found.
fn in_slots<'a>(
    slots: impl IntoIterator<Item = (&'a Box<MathNode>, &'static str)>,
    leaf: NodeId,
    path: &mut Vec<&'static str>,
) -> Option<&'a MathNode> {
    slots.into_iter().find_map(|(slot, name)| {
        path.push(name);
        let found = locate(slot, leaf, path);
        if found.is_none() {
            path.pop();
        }
        found
    })
}
