//! Shared language-adapter acceptance contract. Trees contain no language source/code.
use crate::Template;

/// Normalized semantic math tree, ignoring identities and redundant single-child rows.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum Tree {
    Hole,
    Text(String),
    Row(Vec<Tree>),
    Fraction(Box<Tree>, Box<Tree>),
    Root(Box<Tree>),
    Attach(Box<Tree>, Option<Box<Tree>>, Option<Box<Tree>>),
    Delimiter(char, char, Box<Tree>),
}
/// Language-independent keystrokes and semantic commands in a shared scenario.
#[derive(Debug, Clone)]
#[allow(missing_docs)]
pub enum Step {
    Text(&'static str),
    Structure(Template),
    Tab(bool),
    Right,
    Accept,
    Cancel,
}
/// An executable shared fixture and the resulting abstract formula tree.
#[derive(Debug, Clone)]
#[allow(missing_docs)]
pub struct Case {
    pub name: &'static str,
    pub steps: Vec<Step>,
    pub expected: Tree,
}
/// A language adapter replays these through its real input and authoritative document.
pub trait Adapter {
    /// Execute one abstract step in a fresh, empty formula.
    fn step(&mut self, step: &Step);
    /// Normalize the accepted authoritative formula tree.
    fn tree(&self) -> Tree;
}
/// Run the same expectation against a language adapter.
/// # Panics
/// Panics on an acceptance mismatch, naming the shared fixture.
pub fn check(case: &Case, adapter: &mut impl Adapter) {
    for step in &case.steps {
        adapter.step(step);
    }
    assert_eq!(adapter.tree(), case.expected, "{}", case.name);
}
/// Core independently authored fixtures. LaTeX L3 runs this same collection later.
pub fn cases() -> Vec<Case> {
    use Step::*;
    let text = |s: &str| Tree::Text(s.into());
    vec![
        Case {
            name: "slanted relation",
            steps: vec![Text("<=")],
            expected: text("⩽"),
        },
        Case {
            name: "blackboard cycle",
            steps: vec![Text("RR"), Tab(false)],
            expected: text("ℛ"),
        },
        Case {
            name: "reverse cycle",
            steps: vec![Text("RR"), Tab(true)],
            expected: text("R"),
        },
        Case {
            name: "fraction slots",
            steps: vec![
                Structure(Template::Fraction),
                Text("a"),
                Tab(false),
                Text("b"),
            ],
            expected: Tree::Fraction(Box::new(text("a")), Box::new(text("b"))),
        },
        Case {
            name: "root",
            steps: vec![Structure(Template::Root), Text("x")],
            expected: Tree::Root(Box::new(text("x"))),
        },
        Case {
            name: "paired absolute value",
            steps: vec![Text("|x")],
            expected: Tree::Delimiter('|', '|', Box::new(text("x"))),
        },
        Case {
            name: "command structure",
            steps: vec![Text("\\frac"), Accept, Text("1"), Tab(false), Text("2")],
            expected: Tree::Fraction(Box::new(text("1")), Box::new(text("2"))),
        },
        Case {
            name: "command cancellation",
            steps: vec![Text("x"), Text("\\sqrt"), Cancel],
            expected: text("x"),
        },
        Case {
            name: "both scripts",
            steps: vec![Text("a_n"), Right, Text("^2")],
            expected: Tree::Attach(
                Box::new(text("a")),
                Some(Box::new(text("n"))),
                Some(Box::new(text("2"))),
            ),
        },
    ]
}
