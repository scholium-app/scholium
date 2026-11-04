//! Conservative TeX transcript parsing. Unmapped diagnostics retain their original log position.
use scholium_model::diagnostic::{
    Diagnostic, DiagnosticKernel, DiagnosticRange, LogPosition, Severity, SourcePoint,
    resource_path,
};

/// Parse TeX errors, warnings and box messages without opening files.
/// Relative file-stack entries and the explicit `/project` sandbox mount map to source.
/// Other absolute engine files stay unmapped.
#[must_use]
pub fn parse_log(text: &str) -> Vec<Diagnostic> {
    let mut parser = Parser::default();
    let mut offset = 0;
    for (index, raw) in text.split_inclusive('\n').enumerate() {
        let line = raw.trim_end_matches(['\r', '\n']);
        parser.line(line, index + 1, offset);
        offset += raw.len();
    }
    parser.items
}

#[derive(Default)]
struct Parser {
    stack: Vec<Option<String>>,
    items: Vec<Diagnostic>,
    pending_error: Option<usize>,
    pending_warning: Option<usize>,
}
impl Parser {
    fn line(&mut self, line: &str, number: usize, offset: usize) {
        if let Some(message) = line.strip_prefix("! ") {
            let index = self.push(message, Severity::Error, number, offset, line.len());
            self.pending_error = Some(index);
            self.pending_warning = None;
            return;
        }
        if let Some(rest) = line.strip_prefix("l.") {
            if let Some(n) = leading_number(rest)
                && let Some(index) = self.pending_error.take()
            {
                self.locate(index, n);
            }
            return;
        }
        if let Some(severity) = header(line) {
            self.pending_error = None;
            let index = self.push(line, severity, number, offset, line.len());
            if let Some(n) = input_line(line).or_else(|| box_line(line)) {
                self.locate(index, n);
                self.pending_warning = None;
            } else if severity == Severity::Error {
                self.pending_error = Some(index);
                self.pending_warning = None;
            } else {
                self.pending_warning = Some(index);
            }
            return;
        }
        if let Some(index) = self.pending_warning {
            // TeX wraps warning messages onto indented or package-prefixed continuation lines.
            if (!line.is_empty() && !line.starts_with(['(', ')']))
                || line.starts_with('(') && line.contains(") ")
            {
                self.items[index].message.push(' ');
                self.items[index].message.push_str(line.trim());
                if let Some(n) = input_line(&self.items[index].message) {
                    self.locate(index, n);
                    self.pending_warning = None;
                }
                return;
            }
            self.pending_warning = None;
        }
        // Error context can contain literal parentheses; it is not a file-stack transition.
        if self.pending_error.is_none() {
            self.files(line);
        }
    }

    fn push(
        &mut self,
        message: &str,
        severity: Severity,
        line: usize,
        byte: usize,
        len: usize,
    ) -> usize {
        self.items.push(Diagnostic {
            severity,
            kernel: DiagnosticKernel::Latex,
            resource: self.stack.last().cloned().flatten(),
            range: None,
            node: None,
            message: message.into(),
            code: None,
            log: Some(LogPosition {
                line,
                bytes: byte..byte + len,
            }),
        });
        self.items.len() - 1
    }

    fn locate(&mut self, index: usize, line: usize) {
        if line > 0 && self.items[index].resource.is_some() {
            self.items[index].range = Some(DiagnosticRange {
                start: SourcePoint { line, column: 1 },
                end: None,
                bytes: None,
            });
        }
    }

    fn files(&mut self, line: &str) {
        let mut chars = line.char_indices().peekable();
        while let Some((index, ch)) = chars.next() {
            if ch == ')' {
                self.stack.pop();
            } else if ch == '(' {
                let tail = &line[index + 1..];
                let token = if let Some(quoted) = tail.strip_prefix('"') {
                    quoted.split('"').next().unwrap_or_default()
                } else {
                    tail.split(|c: char| c.is_whitespace() || c == '(' || c == ')')
                        .next()
                        .unwrap_or_default()
                };
                // Track non-source files too: an error inside an absolute .sty must not map to main.tex.
                let is_file = [
                    ".tex", ".sty", ".cls", ".aux", ".bbl", ".cfg", ".def", ".clo", ".fd", ".out",
                ]
                .iter()
                .any(|ext| token.ends_with(ext));
                self.stack.push(if is_file {
                    resource_path(token.strip_prefix("/project/").unwrap_or(token))
                } else {
                    None
                });
                // Parentheses inside a quoted filename are data.
                if tail.starts_with('"') {
                    let end = index + 2 + token.len();
                    while chars.peek().is_some_and(|(i, _)| *i <= end) {
                        chars.next();
                    }
                }
            }
        }
    }
}

fn header(line: &str) -> Option<Severity> {
    if line.starts_with("Overfull ") || line.starts_with("Underfull ") {
        Some(Severity::Info)
    } else if line.starts_with("LaTeX Warning:")
        || line.starts_with("LaTeX Font Warning:")
        || line.starts_with("Package ") && line.contains(" Warning:")
        || line.starts_with("Class ") && line.contains(" Warning:")
    {
        Some(Severity::Warning)
    } else if line.starts_with("Package ") && line.contains(" Error:") {
        Some(Severity::Error)
    } else {
        None
    }
}
fn leading_number(text: &str) -> Option<usize> {
    text.chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .ok()
}
fn input_line(text: &str) -> Option<usize> {
    leading_number(text.split_once("on input line ")?.1)
}
fn box_line(text: &str) -> Option<usize> {
    let tail = text
        .split_once("at lines ")
        .or_else(|| text.split_once("at line "))?
        .1;
    leading_number(tail)
}
