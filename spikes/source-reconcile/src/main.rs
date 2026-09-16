//! 阶段 0 第 3 项验证：源码 reconcile。
//!
//! 同一份小文档生成 LaTeX 与 Typst 源码，模拟人工改写，检验能否可靠映射回语义图。
//! 判据（动手前写定）：
//!
//! 1. 无编辑 → 语义图不变，且**再生成 == 原文**（定点）；
//! 2. 文本叶子局部改字 → 归因到该叶子，**再生成 == 编辑后源码**（往返）；
//! 3. 结构内部叶子改字 → 归因到内层叶子；
//! 4. 把节点整体换成受支持结构（`\sqrt{}` / `sqrt()`）→ `Wrap`；
//! 5. 未知语法 → 落成 `Raw`，**原文逐字保留**，且再生成 == 编辑后源码；
//! 6. 跨节点编辑 → 冲突（拒绝猜测）；
//! 7. 括号不闭合 → 冲突（拒绝，不猜）；
//! 8. 增删行 → 冲突（明确不支持自动归因）。
//!
//! 用法：`cargo run --release`

mod apply;
use apply::apply;
mod generate;
mod reconcile;

use scholium_spike_core::doc::Node;
use scholium_spike_core::{
    ActorId, Document, Editor, Intent, NodeId, NodeKind, SemanticEdit, fixture,
};

use generate::{Dialect, Generated};
use reconcile::Reconciled;

fn main() {
    for dialect in [Dialect::Latex, Dialect::Typst] {
        println!("=== {dialect:?} ===");
        let sample = build_document();
        let original = generate::generate(sample.document(), dialect);
        println!("生成源码：");
        for line in original.text.lines() {
            println!("  | {line}");
        }
        println!("原始语义图：{}", describe(sample.document()));
        // 行映射自检：每行起始字节必须能反查回该行。
        let mut line_map_ok = true;
        for (index, info) in original.lines.iter().enumerate() {
            if original.line_at_byte(info.start_byte) != Some(index) {
                line_map_ok = false;
                println!("  行映射自检 FAIL：行 {index} 起始字节 {}", info.start_byte);
            }
        }
        println!(
            "行映射自检：{}（{} 行）",
            if line_map_ok { "PASS" } else { "FAIL" },
            original.line_count()
        );
        println!();

        case_unchanged(dialect, &original);
        case_text_edit(dialect, &original);
        case_inner_edit(dialect, &original);
        case_wrap(dialect, &original);
        case_unknown(dialect, &original);
        case_cross_node(dialect, &original);
        case_unbalanced(dialect, &original);
        case_line_count(dialect, &original);
        println!();
    }
}

/// 建一份小文档：段落（文本）+ 独立公式（分数 a/b）。
fn build_document() -> Editor {
    let mut editor = Editor::new();
    fixture::build_text(&mut editor, "正文与 ");
    let root = editor.document().root();

    editor
        .apply(
            ActorId(1),
            Intent::Typing,
            SemanticEdit::InsertNode {
                parent: root,
                slot: 0,
                index: 1,
                kind: NodeKind::Math,
            },
        )
        .expect("插入公式");
    let math = child_at(&editor, root, 0, 1);

    editor
        .apply(
            ActorId(1),
            Intent::Typing,
            SemanticEdit::InsertNode {
                parent: math,
                slot: 0,
                index: 0,
                kind: NodeKind::Fraction,
            },
        )
        .expect("插入分数");
    let fraction = child_at(&editor, math, 0, 0);

    for (slot, text) in [(0usize, "a"), (1, "b")] {
        editor
            .apply(
                ActorId(1),
                Intent::Typing,
                SemanticEdit::InsertNode {
                    parent: fraction,
                    slot,
                    index: 0,
                    kind: NodeKind::Text,
                },
            )
            .expect("插入文本");
        let leaf = child_at(&editor, fraction, slot, 0);
        editor
            .apply(
                ActorId(1),
                Intent::Typing,
                SemanticEdit::InsertText {
                    node: leaf,
                    at: 0,
                    text: text.to_string(),
                },
            )
            .expect("写入文本");
    }
    editor
}

fn child_at(editor: &Editor, parent: NodeId, slot: usize, index: usize) -> NodeId {
    editor.document().slot(parent, slot).expect("槽位存在")[index]
}

/// 语义图的规范化描述。
fn describe(document: &Document) -> String {
    fn walk(document: &Document, node: NodeId, out: &mut String) {
        let Ok(current): Result<&Node, _> = document.node(node) else {
            return;
        };
        let kind = format!("{:?}", current.kind);
        match current.kind {
            NodeKind::Text | NodeKind::Raw => out.push_str(&format!(
                "{kind}({:?})",
                document.text_of(node).unwrap_or_default()
            )),
            _ => {
                out.push_str(&format!("{kind}("));
                for slot in 0..4 {
                    let Ok(children) = document.slot(node, slot) else {
                        continue;
                    };
                    for (index, child) in children.iter().enumerate() {
                        if index > 0 {
                            out.push(',');
                        }
                        walk(document, *child, out);
                    }
                }
                out.push(')');
            }
        }
    }
    let mut out = String::new();
    walk(document, document.root(), &mut out);
    out
}

/// 替换某一行的文本，返回编辑后的整份源码。
fn edit_line(original: &Generated, index: usize, new_line: &str) -> String {
    let mut lines: Vec<String> = original.text.lines().map(|line| line.to_string()).collect();
    lines[index] = new_line.to_string();
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// 一个用例的判定与输出。
struct Verdict {
    name: &'static str,
    expectation: &'static str,
    outcome: Reconciled,
    applied: Result<String, String>,
    checks: Vec<(&'static str, bool, String)>,
}

fn run_case(
    name: &'static str,
    expectation: &'static str,
    dialect: Dialect,
    original: &Generated,
    edited: String,
    checks: &[(&'static str, String)],
) -> Verdict {
    // Editor 不是 Clone：用两份同样的文档，一份做参照、一份承接编辑。
    let sample = build_document();
    let mut editor = build_document();
    let outcome = reconcile::reconcile(dialect, original, &edited, sample.document());
    let applied = apply(&mut editor, &outcome);

    // 每个用例自带校验：把"结果文档描述""往返一致性"等作为独立断言。
    let mut results = Vec::new();
    for (label, expected) in checks {
        let described = describe(editor.document());
        let regenerated = generate::generate(editor.document(), dialect).text;
        let ok = match *label {
            "包含" => described.contains(expected.as_str()),
            "往返一致" => regenerated == edited,
            "语义图不变" => described == expected.as_str(),
            _ => false,
        };
        let evidence = if *label == "往返一致" {
            format!("再生成={regenerated:?}")
        } else {
            described.clone()
        };
        results.push((*label, ok, evidence));
    }
    Verdict {
        name,
        expectation,
        outcome,
        applied,
        checks: results,
    }
}

fn report(verdict: &Verdict) {
    let accepted = verdict.applied.is_ok();
    let conflicted = matches!(verdict.outcome, Reconciled::Conflict { .. });
    let attribution_ok = accepted != conflicted || !accepted;
    let all_checks = verdict.checks.iter().all(|(_, ok, _)| *ok);
    let pass = attribution_ok && all_checks;
    println!(
        "  [{}] {}（期望：{}）",
        if pass { "PASS" } else { "FAIL" },
        verdict.name,
        verdict.expectation
    );
    match &verdict.outcome {
        Reconciled::Unchanged => println!("        归因：无改动"),
        Reconciled::ReplaceText {
            node,
            start,
            end,
            text,
        } => println!(
            "        归因：节点 {} 的 [{}..{}) → {text:?}",
            node.index(),
            start,
            end
        ),
        Reconciled::Wrap { node, structure } => {
            println!("        归因：节点 {} 包裹为 {structure}", node.index())
        }
        Reconciled::ReplaceWithRaw {
            node,
            text,
            start,
            end,
            ..
        } => println!(
            "        归因：叶子 {} 的 [{}..{}) → Raw({text:?})",
            node.index(),
            start,
            end
        ),
        Reconciled::Conflict { reason } => println!("        归因：冲突 —— {reason}"),
    }
    match &verdict.applied {
        Ok(note) => println!("        应用：{note}"),
        Err(note) => println!("        应用：被拒绝 —— {note}"),
    }
    for (label, ok, evidence) in &verdict.checks {
        println!(
            "        检查[{label}]：{} {}",
            if *ok { "PASS" } else { "FAIL" },
            truncate(evidence, 150)
        );
    }
}

fn truncate(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let head: String = text.chars().take(limit).collect();
    format!("{head}…")
}

fn case_unchanged(dialect: Dialect, original: &Generated) {
    let before = describe(build_document().document());
    let verdict = run_case(
        "无编辑",
        "不动语义图",
        dialect,
        original,
        original.text.clone(),
        &[("语义图不变", before)],
    );
    report(&verdict);
}

fn case_text_edit(dialect: Dialect, original: &Generated) {
    let new = original.line(0).replace('与', "以及与");
    let edited = edit_line(original, 0, &new);
    let verdict = run_case(
        "段落文本局部改字",
        "归因到文本叶子 + 往返一致",
        dialect,
        original,
        edited,
        &[
            ("包含", "Text(\"正文以及与 \")".to_string()),
            ("往返一致", String::new()),
        ],
    );
    report(&verdict);
}

fn case_inner_edit(dialect: Dialect, original: &Generated) {
    let sample = build_document();
    let Some(line) = original.line_of_kind(sample.document(), NodeKind::Math) else {
        println!("  [FAIL] 找不到公式行");
        return;
    };
    let old = original.line(line).to_string();
    let new = match dialect {
        Dialect::Latex => old.replace("\\frac{a}{b}", "\\frac{ab}{b}"),
        Dialect::Typst => old.replace("frac(a, b)", "frac(ab, b)"),
    };
    let edited = edit_line(original, line, &new);
    let verdict = run_case(
        "结构内部叶子改字",
        "归因到内层叶子 + 往返一致",
        dialect,
        original,
        edited,
        &[
            ("包含", "Fraction(Text(\"ab\")".to_string()),
            ("往返一致", String::new()),
        ],
    );
    report(&verdict);
}

fn case_wrap(dialect: Dialect, original: &Generated) {
    let sample = build_document();
    let Some(line) = original.line_of_kind(sample.document(), NodeKind::Math) else {
        println!("  [FAIL] 找不到公式行");
        return;
    };
    let old = original.line(line).to_string();
    let (from, to) = match dialect {
        Dialect::Latex => ("\\frac{a}{b}", "\\frac{\\sqrt{a}}{b}"),
        Dialect::Typst => ("frac(a, b)", "frac(sqrt(a), b)"),
    };
    let edited = edit_line(original, line, &old.replace(from, to));
    let verdict = run_case(
        "整体换成受支持结构",
        "Wrap 成根式 + 往返一致",
        dialect,
        original,
        edited,
        &[
            ("包含", "Sqrt(Text(\"a\")".to_string()),
            ("往返一致", String::new()),
        ],
    );
    report(&verdict);
}

fn case_unknown(dialect: Dialect, original: &Generated) {
    let (from, to, raw) = match dialect {
        Dialect::Latex => ("正文", "\\customcmd{正文}", "\\customcmd{正文}"),
        Dialect::Typst => ("正文", "#customcmd[正文]", "#customcmd[正文]"),
    };
    let edited = edit_line(original, 0, &original.line(0).replace(from, to));
    let verdict = run_case(
        "未知语法",
        "落成 Raw、原文逐字保留 + 往返一致",
        dialect,
        original,
        edited,
        &[
            ("包含", format!("Raw({raw:?})")),
            ("往返一致", String::new()),
        ],
    );
    report(&verdict);
}

fn case_cross_node(dialect: Dialect, original: &Generated) {
    let sample = build_document();
    let Some(line) = original.line_of_kind(sample.document(), NodeKind::Math) else {
        println!("  [FAIL] 找不到公式行");
        return;
    };
    let old = original.line(line).to_string();
    // 删掉分子与分母之间的分隔，编辑区间横跨两个叶子。
    let new = match dialect {
        Dialect::Latex => old.replace("a}{b", "ab"),
        Dialect::Typst => old.replace("a, b", "ab"),
    };
    let edited = edit_line(original, line, &new);
    let verdict = run_case(
        "跨节点编辑",
        "冲突（拒绝猜测）",
        dialect,
        original,
        edited,
        &[],
    );
    report(&verdict);
}

fn case_unbalanced(dialect: Dialect, original: &Generated) {
    let sample = build_document();
    let Some(line) = original.line_of_kind(sample.document(), NodeKind::Math) else {
        println!("  [FAIL] 找不到公式行");
        return;
    };
    let old = original.line(line).to_string();
    let new = match dialect {
        Dialect::Latex => old.replace("\\frac{a}{b}", "\\frac{a}{b"),
        Dialect::Typst => old.replace("frac(a, b)", "frac(a, b"),
    };
    let edited = edit_line(original, line, &new);
    let verdict = run_case(
        "括号不闭合",
        "冲突（拒绝，不猜）",
        dialect,
        original,
        edited,
        &[],
    );
    report(&verdict);
}

fn case_line_count(dialect: Dialect, original: &Generated) {
    let edited = format!("{}\n", original.text);
    let verdict = run_case(
        "增删行",
        "冲突（明确不支持）",
        dialect,
        original,
        edited,
        &[],
    );
    report(&verdict);
}
