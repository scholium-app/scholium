//! Tight source-engine line boxes with measured descent (PDF bp).
use crate::{
    diag::Diagnostic,
    generate::Ctx,
    ir::{Component, Dialect},
};
use std::path::Path;

pub(crate) fn source(ctx: &Ctx<'_>, component: &Component) -> Result<String, Diagnostic> {
    match component.dialect {
        Dialect::Latex => {
            let preamble = crate::generate::preamble(ctx.project, component.dialect)
                .lines()
                .filter(|line| !line.contains("{geometry}"))
                .collect::<Vec<_>>()
                .join("\n")
                .replace(
                    "\\documentclass[11pt]{article}",
                    "\\documentclass[11pt,border=0pt]{standalone}",
                );
            let fragment = crate::generate::include_fragment(ctx, component)?;
            Ok(format!(
                "{preamble}\n\\begin{{document}}\n\\setbox0=\\hbox{{{fragment}}}\n\\typeout{{SCHOLIUMDEPTH:\\the\\dp0}}\n\\usebox0\n\\end{{document}}"
            ))
        }
        Dialect::Typst => {
            let fragment = crate::generate_typst::include_fragment(ctx, component)?;
            Ok(format!(
                "#set page(width: auto, height: auto, margin: 0pt)\n#set text(size: 11pt, top-edge: \"bounds\", bottom-edge: \"bounds\")\n#box[{fragment}]"
            ))
        }
    }
}
pub(crate) fn latex_depth(dir: &Path, id: &str) -> Result<(), Diagnostic> {
    let run = || -> std::io::Result<()> {
        let log = std::fs::read_to_string(dir.join(format!("{id}.log")))?;
        let value = log
            .lines()
            .find_map(|l| l.strip_prefix("SCHOLIUMDEPTH:"))
            .and_then(|s| s.strip_suffix("pt"))
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or_else(|| std::io::Error::other("missing inline depth"))?;
        // TeX pt is 1/72.27 inch, PDF bp and Typst pt are 1/72 inch.
        std::fs::write(
            dir.join(format!("{id}-depth.json")),
            (value * 72.0 / 72.27).to_string(),
        )
    };
    run().map_err(|e| Diagnostic::new("inline-metrics", e.to_string()))
}
