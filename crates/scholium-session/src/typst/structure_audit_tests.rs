//! T31's independent logical-stop oracle and deletion/Scene audit fixtures.
use super::tests::Fixture;
use super::*;
use scholium_model::{BlockKind, NodeId, Revision};
use std::collections::HashSet;
use unicode_segmentation::UnicodeSegmentation;

type Stop = (NodeId, usize);

fn node(body: MathBody) -> MathNode {
    MathNode {
        node: NodeId::fresh(),
        body,
    }
}
fn text(value: &str) -> MathNode {
    node(MathBody::Text { text: value.into() })
}
fn fraction(num: MathNode, den: MathNode) -> MathNode {
    node(MathBody::Fraction {
        numerator: Box::new(num),
        denominator: Box::new(den),
    })
}
fn attach(base: MathNode, sub: Option<MathNode>, sup: Option<MathNode>) -> MathNode {
    node(MathBody::Attach {
        base: Box::new(base),
        sub: sub.map(Box::new),
        sup: sup.map(Box::new),
    })
}
fn inline(body: InlineBody) -> StructuredInline {
    StructuredInline {
        node: NodeId::fresh(),
        body,
    }
}
fn math(root: MathNode) -> StructuredInline {
    inline(InlineBody::Math { root })
}
fn body(value: &str, style: TextStyle) -> StructuredInline {
    inline(InlineBody::Text {
        text: value.into(),
        style,
    })
}

struct Case {
    name: &'static str,
    kind: BlockKind,
    content: Vec<StructuredInline>,
}
fn roots() -> Vec<(&'static str, MathNode)> {
    vec![
        ("superscript", attach(text("x"), None, Some(text("2")))),
        ("subscript", attach(text("x"), Some(text("i")), None)),
        (
            "both-scripts",
            attach(text("x"), Some(text("i")), Some(text("2"))),
        ),
        (
            "fraction-base",
            attach(fraction(text("a"), text("b")), None, Some(text("2"))),
        ),
        (
            "fraction-in-script",
            attach(text("x"), None, Some(fraction(text("a"), text("b")))),
        ),
        (
            "fraction-in-fraction",
            fraction(fraction(text("a"), text("b")), text("c")),
        ),
        (
            "adjacent-structures",
            node(MathBody::Row {
                children: vec![
                    fraction(text("a"), text("b")),
                    attach(text("x"), Some(text("i")), None),
                ],
            }),
        ),
        (
            "row-text-edges",
            node(MathBody::Row {
                children: vec![text("l"), fraction(text("a"), text("b")), text("r")],
            }),
        ),
        (
            "empty-fraction",
            fraction(MathNode::hole(), MathNode::hole()),
        ),
        (
            "empty-script",
            attach(text("x"), None, Some(MathNode::hole())),
        ),
        ("empty-formula", MathNode::hole()),
        ("graphemes", fraction(text("a\u{301}"), text("👩‍🔬"))),
    ]
}
fn fixtures() -> Vec<Case> {
    let mut cases: Vec<_> = roots()
        .into_iter()
        .map(|(name, root)| Case {
            name,
            kind: BlockKind::Paragraph,
            content: vec![
                body("l", TextStyle::Plain),
                math(root),
                body("r", TextStyle::Plain),
            ],
        })
        .collect();
    for (name, leading, trailing) in [
        ("block-start", false, true),
        ("block-end", true, false),
        ("formula-only-import", false, false),
    ] {
        let mut content = Vec::new();
        if leading {
            content.push(body("l", TextStyle::Plain));
        }
        content.push(math(attach(text("x"), None, Some(text("2")))));
        if trailing {
            content.push(body("r", TextStyle::Plain));
        }
        cases.push(Case {
            name,
            kind: BlockKind::Paragraph,
            content,
        });
    }
    cases.extend(roots().into_iter().map(|(name, root)| Case {
        name,
        kind: BlockKind::Equation,
        content: vec![math(root)],
    }));
    for name in [
        "equation-alone",
        "equation-start",
        "equation-middle",
        "equation-end",
    ] {
        cases.push(Case {
            name,
            kind: BlockKind::Equation,
            content: vec![math(fraction(
                attach(text("x"), None, Some(text("2"))),
                text("b"),
            ))],
        });
    }
    cases.extend(empty_body_cases());
    cases.extend(inline_context_cases());
    cases
}
fn empty_body_cases() -> Vec<Case> {
    vec![
        Case {
            name: "empty-body-boundaries",
            kind: BlockKind::Paragraph,
            content: vec![
                body("", TextStyle::Plain),
                math(fraction(text("a"), text("b"))),
                body("", TextStyle::Plain),
            ],
        },
        Case {
            name: "empty-body-row-boundaries",
            kind: BlockKind::Heading1,
            content: vec![
                body("", TextStyle::Plain),
                math(node(MathBody::Row {
                    children: vec![text("ab"), fraction(text("n"), text("d")), text("c")],
                })),
                body("", TextStyle::Plain),
            ],
        },
    ]
}
fn inline_context_cases() -> Vec<Case> {
    vec![
        Case {
            name: "adjacent-formulas",
            kind: BlockKind::Paragraph,
            content: vec![
                math(fraction(text("a"), text("b"))),
                math(attach(text("x"), Some(text("i")), None)),
            ],
        },
        Case {
            name: "styled-neighbours",
            kind: BlockKind::Paragraph,
            content: vec![
                body("bold", TextStyle::Strong),
                math(fraction(text("a"), text("b"))),
                body("em", TextStyle::Emphasis),
            ],
        },
        Case {
            name: "heading",
            kind: BlockKind::Heading1,
            content: vec![
                body("Title", TextStyle::Plain),
                math(attach(text("x"), Some(text("i")), Some(text("2")))),
            ],
        },
        Case {
            name: "raw-next-to-structure",
            kind: BlockKind::Paragraph,
            content: vec![
                body("l", TextStyle::Plain),
                inline(InlineBody::RawMath {
                    source: "sqrt(x)".into(),
                }),
                math(fraction(text("a"), text("b"))),
                body("r", TextStyle::Plain),
            ],
        },
    ]
}

fn position((leaf, byte): Stop) -> Position {
    Position {
        leaf,
        byte,
        affinity: Affinity::Downstream,
    }
}
fn stop(core: &EditorCore) -> Stop {
    (core.cursor.leaf, core.cursor.byte)
}
fn key(core: &mut EditorCore, key: Key) {
    core.input(Input::Key {
        key,
        mods: Modifiers::default(),
    })
    .expect("audit key");
}
fn text_stops(id: NodeId, value: &str, out: &mut Vec<Stop>) {
    out.extend(value.grapheme_indices(true).map(|(at, _)| (id, at)));
    out.push((id, value.len()));
}
// Independent oracle: only direct same-row Text/Hole neighbours replace compound edges.
fn math_stops(root: &MathNode, before: bool, after: bool, out: &mut Vec<Stop>) {
    match &root.body {
        MathBody::Text { text } => text_stops(root.node, text, out),
        MathBody::Hole => out.push((root.node, 0)),
        MathBody::Row { children } => {
            let supplies = |n: &MathNode| matches!(n.body, MathBody::Text { .. } | MathBody::Hole);
            for (i, child) in children.iter().enumerate() {
                math_stops(
                    child,
                    i > 0 && supplies(&children[i - 1]),
                    children.get(i + 1).is_some_and(supplies),
                    out,
                );
            }
        }
        MathBody::Fraction {
            numerator,
            denominator,
        } => {
            if !before {
                out.push((root.node, 0));
            }
            math_stops(numerator, false, false, out);
            math_stops(denominator, false, false, out);
            if !after {
                out.push((root.node, 1));
            }
        }
        MathBody::Attach { base, sub, sup } => {
            if !before {
                out.push((root.node, 0));
            }
            math_stops(base, false, false, out);
            for slot in [sub, sup].into_iter().flatten() {
                math_stops(slot, false, false, out);
            }
            if !after {
                out.push((root.node, 1));
            }
        }
        MathBody::Symbol { .. } => {}
    }
}
fn expected(doc: &StructuredDocument) -> Vec<Stop> {
    let mut out = Vec::new();
    for inline in doc.blocks.iter().flat_map(|b| &b.content) {
        match &inline.body {
            InlineBody::Text { text, .. } => text_stops(inline.node, text, &mut out),
            InlineBody::Math { root } => math_stops(root, false, false, &mut out),
            InlineBody::RawMath { .. } => {}
        }
    }
    out
}
fn fixture(case: Case) -> Fixture {
    let mut f = Fixture::new();
    let mut doc = (*f.core.snapshot).clone();
    doc.revision = Revision(0);
    doc.blocks = vec![StructuredBlock {
        node: NodeId::fresh(),
        kind: case.kind,
        content: case.content,
    }];
    let paragraph = |value| StructuredBlock {
        node: NodeId::fresh(),
        kind: BlockKind::Paragraph,
        content: vec![body(value, TextStyle::Plain)],
    };
    if matches!(case.name, "equation-middle" | "equation-end") {
        doc.blocks.insert(0, paragraph("before"));
    }
    if matches!(case.name, "equation-middle" | "equation-start") {
        doc.blocks.push(paragraph("after"));
    }
    f.core
        .restore_document(doc, vec![])
        .expect("valid audit fixture");
    f.core.cursor = position(expected(&f.core.snapshot)[0]);
    f
}
fn walk(core: &mut EditorCore, key_name: Key) -> Vec<Stop> {
    let mut result = vec![stop(core)];
    for _ in 0..512 {
        key(core, key_name);
        let next = stop(core);
        if result.last() == Some(&next) {
            return result;
        }
        assert!(!result.contains(&next), "cycle at {next:?}");
        result.push(next);
    }
    panic!("walk did not terminate");
}

#[test]
fn structure_audit_visits_every_stop_once_in_exact_reverse_order() {
    for case in fixtures() {
        let name = case.name;
        let mut f = fixture(case);
        let expected = expected(&f.core.snapshot);
        assert_eq!(
            expected.iter().copied().collect::<HashSet<_>>().len(),
            expected.len(),
            "{name}"
        );
        assert_eq!(
            walk(&mut f.core, Key::ArrowRight),
            expected,
            "{name}: Right"
        );
        assert_eq!(
            walk(&mut f.core, Key::ArrowLeft),
            expected.iter().copied().rev().collect::<Vec<_>>(),
            "{name}: Left"
        );
        assert!(f.core.undo.is_empty(), "{name}: navigation history");
        println!("AUDIT-WALK {name}: Right={expected:?}; Left=reverse");
    }
}

fn empty(node: &MathNode) -> bool {
    match &node.body {
        MathBody::Hole => true,
        MathBody::Text { text } => text.is_empty(),
        MathBody::Symbol { .. } => false,
        _ => input::math_children(node).into_iter().all(empty),
    }
}
fn removable(root: &MathNode, at: NodeId) -> bool {
    if !math_contains(root, at) {
        return false;
    }
    if empty(root)
        && matches!(
            root.body,
            MathBody::Fraction { .. } | MathBody::Attach { .. }
        )
    {
        return true;
    }
    if let MathBody::Attach { sub, sup, .. } = &root.body
        && [sub, sup]
            .into_iter()
            .flatten()
            .any(|s| s.node == at && empty(s))
    {
        return true;
    }
    input::math_children(root)
        .into_iter()
        .any(|c| removable(c, at))
}
fn can_delete(core: &EditorCore, forward: bool) -> bool {
    let at = core.cursor;
    if !input::leaves(&core.snapshot)
        .into_iter()
        .any(|l| l.id == at.leaf)
    {
        return false;
    }
    let grapheme = input::leaves(&core.snapshot).into_iter().any(|l| {
        l.id == at.leaf
            && if forward {
                at.byte < l.text.len()
            } else {
                at.byte > 0
            }
    });
    let merge = core.snapshot.blocks.windows(2).any(|pair| {
        if pair.iter().any(|b| b.kind == BlockKind::Equation) { return false; }
        let inline = if forward { pair[0].content.last() } else { pair[1].content.first() };
        inline.is_some_and(|i| matches!(&i.body, InlineBody::Text { text, .. } if i.node == at.leaf && at.byte == if forward { text.len() } else { 0 }))
    });
    grapheme || merge
        || core
            .snapshot
            .blocks
            .iter()
            .flat_map(|b| &b.content)
            .any(|i| matches!(&i.body, InlineBody::Math { root } if removable(root, at.leaf) || (math_contains(root, at.leaf) && empty(root))))
}
#[test]
fn structure_audit_deletion_only_edits_deletable_units_otherwise_matches_arrows() {
    for forward in [false, true] {
        for case in fixtures() {
            let name = case.name;
            let mut f = fixture(case);
            let stops = expected(&f.core.snapshot);
            f.core.cursor = position(if forward {
                stops[0]
            } else {
                *stops.last().expect("stops")
            });
            let mut visited = Vec::new();
            for _ in 0..512 {
                let before = f.core.record();
                let undo = f.core.undo.len();
                let run = format!("{:?}", f.core.run);
                let allowed = can_delete(&f.core, forward);
                let arrow =
                    scholium_typst::editor::next_caret(&f.core.snapshot, f.core.cursor, forward)
                        .map(|p| (p.leaf, p.byte))
                        .unwrap_or(stop(&f.core));
                visited.push(stop(&f.core));
                key(
                    &mut f.core,
                    if forward { Key::Delete } else { Key::Backspace },
                );
                let changed = f.core.snapshot.as_ref() != &before.snapshot;
                assert_eq!(
                    changed, allowed,
                    "{name}: direction={forward}, trace={visited:?}"
                );
                if !changed {
                    assert_eq!(stop(&f.core), arrow, "{name}: deletion must match arrow");
                    assert_eq!(f.core.undo.len(), undo, "{name}: movement added history");
                    assert_eq!(
                        format!("{:?}", f.core.run),
                        run,
                        "{name}: movement reset run"
                    );
                    assert_eq!(
                        f.core.record().requests,
                        before.requests,
                        "{name}: movement added action"
                    );
                    if stop(&f.core) == (before.cursor.leaf, before.cursor.byte) {
                        break;
                    }
                }
                assert!(visited.len() < 512, "{name}: deletion did not terminate");
            }
            println!(
                "AUDIT-DELETE {name} forward={forward}: {visited:?}; final={:?}",
                stop(&f.core)
            );
        }
    }
}

#[test]
fn structure_audit_every_renderable_stop_has_nonempty_scene_geometry() {
    let mut failures = Vec::new();
    for case in fixtures() {
        let name = case.name;
        // ADR 0012/0023 preserve RawMath without compiling it in-process. The coordinator
        // retained explicit layout rejection; traversal/deletion still audit this fixture.
        if name == "raw-next-to-structure" {
            continue;
        }
        let mut f = fixture(case);
        let path = std::env::temp_dir().join(format!("t31-scene-{:?}.sqlite", RequestId::fresh()));
        let mut runtime =
            EditorCore::open_with_fonts(&path, || {}, scholium_typst::FontProfile::Bundled)
                .expect("worker");
        f.core.runtime = runtime.runtime.take();
        drop(runtime);
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
        f.core.submit_layout();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while !f.core.scene_is_current() && f.core.layout_error.is_none() {
            f.core.poll_layout();
            assert!(
                std::time::Instant::now() < deadline,
                "{name}: worker timeout"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        if let Some(error) = &f.core.layout_error {
            failures.push(format!("{name}: no Scene: {error:?}"));
            continue;
        }
        let geometry = &f.core.current_scene().expect("current Scene").geometry;
        for at in expected(&f.core.snapshot) {
            match geometry.caret(position(at)) {
                Some(c)
                    if c.top[0].is_finite() && c.top[1].is_finite() && c.bottom[1] > c.top[1] => {}
                other => failures.push(format!("{name}: {at:?}: {other:?}")),
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn structure_audit_raw_math_explicitly_rejects_layout_with_its_node_identity() {
    let case = fixtures()
        .into_iter()
        .find(|c| c.name == "raw-next-to-structure")
        .expect("RawMath fixture");
    let f = fixture(case);
    let raw = f.core.snapshot.blocks[0]
        .content
        .iter()
        .find(|i| matches!(i.body, InlineBody::RawMath { .. }))
        .expect("RawMath")
        .node;
    let worker = scholium_typst::editor::EditorWorker::spawn_with_fonts(
        || {},
        scholium_typst::FontProfile::Bundled,
    )
    .expect("worker");
    worker.submit(
        f.core.snapshot.clone(),
        f.core.scene_stamp(),
        f.core.raster,
        f.core.view,
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        if let Some((_, result)) = worker.poll() {
            assert!(
                matches!(result, Err(scholium_typst::editor::EditorError::Raw(id)) if id == raw)
            );
            break;
        }
        assert!(std::time::Instant::now() < deadline, "worker timeout");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
