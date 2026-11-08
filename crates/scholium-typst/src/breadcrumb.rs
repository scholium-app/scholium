//! Syntax-only navigation for source-authoritative projects.
/// A source breadcrumb and its UTF-8 byte jump target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Breadcrumb {
    /// Heading text, or `math` / `raw` for the enclosing syntax.
    pub label: String,
    /// UTF-8 byte offset of the syntax start.
    pub byte: usize,
}
/// Heading ancestry and enclosing math/raw syntax at the caret; never evaluates source.
#[must_use]
pub fn breadcrumbs(text: &str, caret: usize) -> Vec<Breadcrumb> {
    let source = typst::syntax::Source::detached(text.to_owned());
    let mut headings = Vec::new();
    let mut enclosing = Vec::new();
    visit(
        typst::syntax::LinkedNode::new(source.root()),
        text,
        caret.min(text.len()),
        &mut headings,
        &mut enclosing,
    );
    headings
        .into_iter()
        .map(|(_, crumb)| crumb)
        .chain(enclosing)
        .collect()
}
fn visit(
    node: typst::syntax::LinkedNode<'_>,
    text: &str,
    caret: usize,
    headings: &mut Vec<(usize, Breadcrumb)>,
    enclosing: &mut Vec<Breadcrumb>,
) {
    use typst::syntax::{
        SyntaxKind,
        ast::{AstNode, Heading},
    };
    let range = node.range();
    if range.start > caret {
        return;
    }
    if let Some(heading) = node.get().cast::<Heading>() {
        let depth = heading.depth().get();
        headings.retain(|(d, _)| *d < depth);
        headings.push((
            depth,
            Breadcrumb {
                label: heading.body().to_untyped().full_text().trim().to_string(),
                byte: range.start,
            },
        ));
    }
    if matches!(node.kind(), SyntaxKind::Equation | SyntaxKind::Raw)
        && (range.contains(&caret) || caret == text.len() && caret == range.end)
    {
        enclosing.push(Breadcrumb {
            label: if node.kind() == SyntaxKind::Raw {
                "raw"
            } else {
                "math"
            }
            .into(),
            byte: range.start,
        });
        return;
    }
    // Only document markup headings participate in the section path. Raw/code content
    // can contain heading-shaped text but cannot open a document section.
    if matches!(
        node.kind(),
        SyntaxKind::Raw | SyntaxKind::CodeBlock | SyntaxKind::Heading
    ) {
        return;
    }
    for child in node.children() {
        visit(child, text, caret, headings, enclosing);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn headings_and_enclosing_math_follow_utf8_caret() {
        let text = "= 引言\n== 方法\n$x + y$\n= End\n";
        let path = breadcrumbs(
            text,
            text.find('x').expect("fixture contains this source marker"),
        );
        assert_eq!(
            path.iter().map(|p| p.label.as_str()).collect::<Vec<_>>(),
            ["引言", "方法", "math"]
        );
        assert_eq!(
            path[2].byte,
            text.find('$').expect("fixture contains this source marker")
        );
        assert_eq!(breadcrumbs(text, text.len())[0].label, "End");
    }
    #[test]
    fn comments_and_raw_heading_text_are_not_sections() {
        let text = "= Real\n// = Fake\n```typ\n= Fake\n```\n";
        let at = text
            .rfind("= Fake\n```")
            .expect("fixture contains this source marker");
        let path = breadcrumbs(text, at);
        assert_eq!(
            path.iter().map(|p| p.label.as_str()).collect::<Vec<_>>(),
            ["Real", "raw"]
        );
        assert_eq!(breadcrumbs(text, text.len()).len(), 1);
    }
}
