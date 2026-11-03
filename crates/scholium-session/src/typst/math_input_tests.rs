use super::*;
use scholium_math_input::accept::{self, Adapter, Step, Tree};

struct TypstAdapter(super::tests::Fixture);
impl Adapter for TypstAdapter {
    fn step(&mut self, step: &Step) {
        let input = match step {
            Step::Text(s) => Input::Text((*s).into()),
            Step::Structure(t) => Input::Command(
                CommandId::MathInput,
                CommandArgs::Math(scholium_math_input::Event::Structure(*t)),
            ),
            Step::Tab(reverse) => Input::Key {
                key: Key::Tab,
                mods: Modifiers {
                    shift: *reverse,
                    ..Modifiers::default()
                },
            },
            Step::Right => Input::Key {
                key: Key::ArrowRight,
                mods: Modifiers::default(),
            },
            Step::Accept => Input::Key {
                key: Key::Enter,
                mods: Modifiers::default(),
            },
            Step::Cancel => Input::Key {
                key: Key::Escape,
                mods: Modifiers::default(),
            },
        };
        self.0.core.input(input).expect("shared acceptance input");
    }
    fn tree(&self) -> Tree {
        tree(self.0.core.formula_root().expect("formula"))
    }
}
fn tree(node: &MathNode) -> Tree {
    match &node.body {
        MathBody::Hole => Tree::Hole,
        MathBody::Text { text } => Tree::Text(text.clone()),
        MathBody::Row { children } => {
            let mut nodes: Vec<_> = children.iter().map(tree).collect();
            if nodes.len() == 1 {
                nodes.remove(0)
            } else {
                Tree::Row(nodes)
            }
        }
        MathBody::Fraction {
            numerator,
            denominator,
        } => Tree::Fraction(Box::new(tree(numerator)), Box::new(tree(denominator))),
        MathBody::Root { radicand, .. } => Tree::Root(Box::new(tree(radicand))),
        MathBody::Attach { base, sub, sup } => Tree::Attach(
            Box::new(tree(base)),
            sub.as_deref().map(tree).map(Box::new),
            sup.as_deref().map(tree).map(Box::new),
        ),
        MathBody::Delimited { open, close, body } => {
            Tree::Delimiter(*open, *close, Box::new(tree(body)))
        }
        other => panic!("unsupported fixture node {other:?}"),
    }
}
#[test]
fn typst_executes_the_shared_keystroke_to_formula_tree_acceptance_cases() {
    for case in accept::cases() {
        let mut adapter = TypstAdapter(super::tests::Fixture::new());
        adapter.0.run(CommandId::Math);
        accept::check(&case, &mut adapter);
    }
}
#[test]
fn tab_leaves_the_last_fraction_slot_at_the_structure_after_edge() {
    let mut f = super::tests::Fixture::new();
    f.run(CommandId::Math);
    f.run(CommandId::Fraction);
    for _ in 0..2 {
        f.core
            .input(Input::Key {
                key: Key::Tab,
                mods: Modifiers::default(),
            })
            .expect("Tab");
    }
    let root = f.core.formula_root().expect("formula");
    assert_eq!(f.core.cursor.leaf, root.node);
    assert_eq!(f.core.cursor.byte, 1);
}
#[test]
fn ime_commit_and_paste_keep_literal_rules_and_command_cancel_is_revision_neutral() {
    let mut f = super::tests::Fixture::new();
    f.run(CommandId::Math);
    for origin in [Origin::Paste, Origin::Commit] {
        f.core
            .input(Input::Command(
                CommandId::Text,
                CommandArgs::Text("<=RR(".into(), origin),
            ))
            .expect("literal");
    }
    let revision = f.core.snapshot.revision;
    f.core.input(Input::Text("\\frac".into())).expect("command");
    assert_eq!(f.core.snapshot.revision, revision);
    f.core
        .input(Input::Key {
            key: Key::Escape,
            mods: Modifiers::default(),
        })
        .expect("cancel");
    assert_eq!(f.core.snapshot.revision, revision);
    assert!(f.core.in_math());
}
