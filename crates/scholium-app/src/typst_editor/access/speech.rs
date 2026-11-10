//! Spoken projection of structural math, beside (never inside) the editable text runs.
//!
//! Wording follows the Orca-validated stage-0 conventions (report 0042): formulas read as
//! `分数（分子：…；分母：…）`, slot context reads outside-in, and empty slots read as `空`.
use scholium_model::{
    NodeId,
    structured::{MathBody, MathNode},
};

/// Whole-formula description; empty slots are spoken, never skipped.
#[cfg(test)]
pub(super) fn describe(node: &MathNode) -> String {
    describe_in(node, crate::i18n::Locale::ZhHans)
}

pub(super) fn describe_in(node: &MathNode, locale: crate::i18n::Locale) -> String {
    match &node.body {
        MathBody::Text { text } => text.clone(),
        MathBody::Symbol { symbol } => symbol.to_string(),
        MathBody::Hole => crate::i18n::text(locale, "a11y.math.empty"),
        MathBody::Row { children } => children
            .iter()
            .map(|n| describe_in(n, locale))
            .collect::<Vec<_>>()
            .join(separator(locale)),
        MathBody::Fraction {
            numerator,
            denominator,
        } => crate::i18n::text_with(
            locale,
            "a11y.math.fraction",
            &[
                ("numerator", describe_in(numerator, locale).into()),
                ("denominator", describe_in(denominator, locale).into()),
            ],
        ),
        MathBody::Attach { base, sub, sup } => {
            let slots = [("a11y.math.attach.sub", sub), ("a11y.math.attach.sup", sup)]
                .into_iter()
                .filter_map(|(key, slot)| {
                    Some(crate::i18n::text_with(
                        locale,
                        key,
                        &[("content", describe_in(slot.as_ref()?, locale).into())],
                    ))
                });
            crate::i18n::text_with(
                locale,
                "a11y.math.attach",
                &[
                    ("base", describe_in(base, locale).into()),
                    ("scripts", slots.collect::<String>().into()),
                ],
            )
        }
        MathBody::Root { index, radicand } => {
            let radicand = describe_in(radicand, locale);
            let (key, index) = match index {
                Some(index) => ("a11y.math.root.indexed", describe_in(index, locale)),
                None => ("a11y.math.root.square", String::new()),
            };
            crate::i18n::text_with(
                locale,
                key,
                &[("index", index.into()), ("radicand", radicand.into())],
            )
        }
        MathBody::Delimited { open, close, body } => crate::i18n::text_with(
            locale,
            "a11y.math.delimited",
            &[
                ("open", delimiter_name(*open, locale).into()),
                ("close", delimiter_name(*close, locale).into()),
                ("body", describe_in(body, locale).into()),
            ],
        ),
    }
}

/// Outside-in slot path of `leaf` and its content, e.g. `分数分子，分数分母：空`.
///
/// A leaf directly in the formula (no slot ancestor) reads as `公式：…`; `None` when
/// `leaf` is not inside `root`.
pub(super) fn slot_context(root: &MathNode, leaf: NodeId) -> Option<String> {
    slot_context_in(root, leaf, crate::i18n::Locale::ZhHans)
}

fn slot_context_in(root: &MathNode, leaf: NodeId, locale: crate::i18n::Locale) -> Option<String> {
    let mut path = Vec::new();
    let found = locate(root, leaf, &mut path)?;
    let content = describe_in(found, locale);
    let content = if content.is_empty() {
        crate::i18n::text(locale, "a11y.math.empty")
    } else {
        content
    };
    let path = if path.is_empty() {
        crate::i18n::text(locale, "a11y.math.formula.name")
    } else {
        path.iter()
            .map(|name| slot_name(name, locale))
            .collect::<Vec<_>>()
            .join(separator(locale))
    };
    Some(crate::i18n::text_with(
        locale,
        "a11y.math.context",
        &[("path", path.into()), ("content", content.into())],
    ))
}

fn separator(locale: crate::i18n::Locale) -> &'static str {
    match locale {
        crate::i18n::Locale::ZhHans => "，",
        crate::i18n::Locale::En => ", ",
    }
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
        (MathBody::Root { .. }, true) => "a11y.math.edge.root.before",
        (MathBody::Root { .. }, false) => "a11y.math.edge.root.after",
        (MathBody::Delimited { .. }, true) => "a11y.math.edge.delimited.before",
        (MathBody::Delimited { .. }, false) => "a11y.math.edge.delimited.after",
        (MathBody::Fraction { .. }, true) => "a11y.math.edge.fraction.before",
        (MathBody::Fraction { .. }, false) => "a11y.math.edge.fraction.after",
        (MathBody::Attach { .. }, true) => "a11y.math.edge.attach.before",
        (MathBody::Attach { .. }, false) => "a11y.math.edge.attach.after",
        _ => return slot_context_in(root, at.leaf, locale),
    };
    let edge = crate::i18n::text(locale, key);
    Some(if path.is_empty() {
        edge
    } else {
        crate::i18n::text_with(
            locale,
            "a11y.math.edge.nested",
            &[
                (
                    "path",
                    path.iter()
                        .map(|name| slot_name(name, locale))
                        .collect::<Vec<_>>()
                        .join(separator(locale))
                        .into(),
                ),
                ("edge", edge.into()),
            ],
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
            [
                (numerator, "a11y.math.slot.numerator"),
                (denominator, "a11y.math.slot.denominator"),
            ],
            leaf,
            path,
        ),
        MathBody::Attach { base, sub, sup } => in_slots(
            [
                (Some(base), "a11y.math.slot.base"),
                (sub.as_ref(), "a11y.math.slot.sub"),
                (sup.as_ref(), "a11y.math.slot.sup"),
            ]
            .into_iter()
            .filter_map(|(slot, name)| Some((slot?, name))),
            leaf,
            path,
        ),
        MathBody::Root { index, radicand } => in_slots(
            [
                (index.as_ref(), "a11y.math.slot.root_index"),
                (Some(radicand), "a11y.math.slot.radicand"),
            ]
            .into_iter()
            .filter_map(|(slot, name)| Some((slot?, name))),
            leaf,
            path,
        ),
        MathBody::Delimited { body, .. } => {
            in_slots([(body, "a11y.math.slot.delimited")], leaf, path)
        }
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

fn slot_name(name: &str, locale: crate::i18n::Locale) -> String {
    if name.starts_with("a11y.") {
        crate::i18n::text(locale, name)
    } else {
        name.to_owned()
    }
}

fn delimiter_name(c: char, locale: crate::i18n::Locale) -> String {
    let key = match c {
        '(' => "a11y.math.delimiter.paren.open",
        ')' => "a11y.math.delimiter.paren.close",
        '[' => "a11y.math.delimiter.bracket.open",
        ']' => "a11y.math.delimiter.bracket.close",
        '{' => "a11y.math.delimiter.brace.open",
        '}' => "a11y.math.delimiter.brace.close",
        '|' => "a11y.math.delimiter.abs",
        '‖' => "a11y.math.delimiter.norm",
        '⟨' => "a11y.math.delimiter.angle.open",
        '⟩' => "a11y.math.delimiter.angle.close",
        '⌊' => "a11y.math.delimiter.floor.open",
        '⌋' => "a11y.math.delimiter.floor.close",
        '⌈' => "a11y.math.delimiter.ceil.open",
        '⌉' => "a11y.math.delimiter.ceil.close",
        _ => "a11y.math.delimiter.invisible",
    };
    crate::i18n::text(locale, key)
}
