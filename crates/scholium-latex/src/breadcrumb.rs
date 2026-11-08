//! Lightweight lexical navigation; never expands TeX macros.
/// A source breadcrumb and its UTF-8 byte jump target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Breadcrumb {
    /// Source-derived section title or environment name.
    pub label: String,
    /// UTF-8 byte offset of the opening command.
    pub byte: usize,
}
/// Section ancestry and open environments at a UTF-8 caret offset.
/// Offsets beyond EOF are clamped; no TeX expansion or counter evaluation occurs.
#[must_use]
pub fn breadcrumbs(text: &str, caret: usize) -> Vec<Breadcrumb> {
    let caret = caret.min(text.len());
    let mut sections: Vec<(usize, Breadcrumb)> = Vec::new();
    let mut environments: Vec<Breadcrumb> = Vec::new();
    let mut skip = 0;
    for token in crate::highlight(text) {
        if token.bytes.start > caret {
            break;
        }
        if token.bytes.start < skip || token.kind != crate::TokenKind::Control {
            continue;
        }
        let command = &text[token.bytes.clone()];
        if command == "\\verb" {
            skip = verb_end(text, token.bytes.end);
            continue;
        }
        let Some((label, end)) = argument(text, token.bytes.end) else {
            continue;
        };
        let crumb = Breadcrumb {
            label,
            byte: token.bytes.start,
        };
        skip = end;
        if let Some(depth) = section_depth(command) {
            sections.retain(|(d, _)| *d < depth);
            sections.push((depth, crumb));
        } else if command == "\\begin" {
            if matches!(
                crumb.label.as_str(),
                "verbatim" | "verbatim*" | "lstlisting" | "minted"
            ) {
                let close = format!("\\end{{{}}}", crumb.label);
                skip = text[end..]
                    .find(&close)
                    .map_or(text.len(), |n| end + n + close.len());
                if caret < skip {
                    environments.push(crumb);
                }
            } else if crumb.label != "document" {
                environments.push(crumb);
            }
        } else if command == "\\end"
            && let Some(index) = environments.iter().rposition(|c| c.label == crumb.label)
        {
            environments.truncate(index);
        }
    }
    sections
        .into_iter()
        .map(|(_, c)| c)
        .chain(environments)
        .collect()
}
fn verb_end(text: &str, start: usize) -> usize {
    let rest = text[start..].trim_start_matches('*');
    let Some(delimiter) = rest.chars().next() else {
        return text.len();
    };
    let end = delimiter.len_utf8();
    rest[end..]
        .find(delimiter)
        .map_or(text.len(), |n| text.len() - rest.len() + end + n + end)
}
fn section_depth(command: &str) -> Option<usize> {
    [
        "\\part",
        "\\chapter",
        "\\section",
        "\\subsection",
        "\\subsubsection",
        "\\paragraph",
        "\\subparagraph",
    ]
    .iter()
    .position(|c| *c == command)
}
fn argument(text: &str, start: usize) -> Option<(String, usize)> {
    let mut at = start;
    if text[at..].starts_with('*') {
        at += 1;
    }
    at += text[at..].len() - text[at..].trim_start().len();
    if text[at..].starts_with('[') {
        at = group_end(text, at, '[', ']')?;
        at += text[at..].len() - text[at..].trim_start().len();
    }
    if !text[at..].starts_with('{') {
        return None;
    }
    let end = group_end(text, at, '{', '}')?;
    Some((
        text[at + 1..end - 1]
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
        end,
    ))
}
fn group_end(text: &str, start: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0;
    let mut escaped = false;
    let mut comment = false;
    for (offset, ch) in text[start..].char_indices() {
        if comment {
            comment = ch != '\n';
            continue;
        }
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '%' {
            comment = true;
        } else if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                return Some(start + offset + ch.len_utf8());
            }
        }
    }
    None
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cursor_follows_section_hierarchy_and_nested_environments() {
        let text = "\\section{引言}\n\\subsection{方法}\n\\begin{equation}x\\end{equation}\n\\section*{末尾}";
        let at = text.find('x').expect("fixture contains this source marker");
        let path = breadcrumbs(text, at);
        assert_eq!(
            path.iter().map(|p| p.label.as_str()).collect::<Vec<_>>(),
            ["引言", "方法", "equation"]
        );
        assert_eq!(
            path[2].byte,
            text.find("\\begin")
                .expect("fixture contains this source marker")
        );
        assert_eq!(
            breadcrumbs(text, text.len())
                .iter()
                .map(|p| p.label.as_str())
                .collect::<Vec<_>>(),
            ["末尾"]
        );
    }
    #[test]
    fn comments_escaped_commands_and_verbatim_do_not_create_structure() {
        let text = "\\section{Real {title}}\n% \\section{fake}\n\\\\section{fake}\n\\begin{verbatim}\\section{fake}\\end{verbatim}\n";
        assert_eq!(breadcrumbs(text, text.len())[0].label, "Real {title}");
        assert_eq!(breadcrumbs(text, text.len()).len(), 1);
    }
    #[test]
    fn optional_titles_and_incomplete_groups_are_safe() {
        let text = "\\section[short]{Long title}\n\\subsection{unfinished";
        assert_eq!(breadcrumbs(text, text.len())[0].label, "Long title");
        assert_eq!(breadcrumbs(text, 0).len(), 1);
    }
}
