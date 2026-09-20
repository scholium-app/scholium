//! Recreate PDF link annotations in host layout without touching vector paint.
use super::{Geometry, Target};
use crate::{
    diag::Diagnostic,
    generate::native_owner,
    ir::{Dialect, Project},
};
use std::{fmt::Write, path::Path};
// typst-pdf 0.15.1 link::pos_to_xyz adds this viewing margin on export.
const TYPST_DESTINATION_MARGIN_PT: f64 = 10.0;

fn valid_label(label: &str) -> bool {
    !label.is_empty()
        && label
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b":._-".contains(&b))
}
pub(crate) fn write(project: &Project, id: &str, dir: &Path) -> Result<(), Diagnostic> {
    let run = || -> std::io::Result<()> {
        let geometry = super::worker::run(&dir.join(format!("{id}.pdf")))?;
        std::fs::write(
            dir.join(format!("{id}-geometry.json")),
            serde_json::to_vec_pretty(&geometry)?,
        )?;
        let inline = project
            .component(id)
            .is_some_and(|c| c.placement == crate::ir::Placement::Inline);
        let depth = if inline {
            let value: f64 =
                serde_json::from_slice(&std::fs::read(dir.join(format!("{id}-depth.json")))?)?;
            if !value.is_finite() || value < 0.0 || value > geometry.size[1] {
                return Err(std::io::Error::other("invalid inline depth"));
            }
            Some(value)
        } else {
            None
        };
        let source = source(project, id, &geometry, depth)?;
        let ext = if project.host == Dialect::Latex {
            "tex"
        } else {
            "typ"
        };
        std::fs::write(dir.join(format!("{id}-embed.{ext}")), source)
    };
    run().map_err(|e| Diagnostic::new("vector-annotation-unsupported", format!("组件 `{id}`: {e}")))
}
fn reference(project: &Project, uri: &str) -> std::io::Result<(String, bool)> {
    if let Some(label) = uri.strip_prefix("scholium-ref:") {
        if !valid_label(label) {
            return Err(std::io::Error::other("unsafe symbol"));
        }
        let owner = project
            .all_blocks()
            .find(|(_, b)| b.label() == Some(label))
            .map(|(o, _)| o)
            .ok_or_else(|| std::io::Error::other("missing link target"))?;
        return Ok((
            if native_owner(project, owner) {
                label.to_owned()
            } else {
                format!("sch:{label}")
            },
            false,
        ));
    }
    if uri.contains(['{', '}', '\\', '\n', '\r', '"']) {
        return Err(std::io::Error::other("unsafe URI"));
    }
    Ok((uri.to_owned(), true))
}
fn source(
    project: &Project,
    id: &str,
    g: &Geometry,
    depth: Option<f64>,
) -> std::io::Result<String> {
    let [w, h] = g.size;
    let latex = project.host == Dialect::Latex;
    let mut out = header(id, [w, h], depth, latex);
    for (label, point) in &g.anchors {
        if !valid_label(label) {
            return Err(std::io::Error::other("unsafe anchor"));
        }
        anchor(&mut out, &format!("sch:{label}"), *point, h, latex);
    }
    for (index, link) in g.links.iter().enumerate() {
        let (target, external) = match &link.target {
            Target::Point(point) => {
                let label = format!("sch:{id}:link:{index}");
                anchor(&mut out, &label, *point, h, latex);
                (label, false)
            }
            Target::Uri(uri) => reference(project, uri)?,
        };
        annotation(&mut out, link.rect, (&target, external), h, latex)?;
    }
    out.push_str(if latex {
        "\\end{picture}}"
    } else if depth.is_some() {
        "] }"
    } else {
        "] })"
    });
    Ok(out)
}
fn header(id: &str, [w, h]: [f64; 2], depth: Option<f64>, latex: bool) -> String {
    if latex {
        let wrapper = depth
            .map(|d| format!("\\raisebox{{-{d}bp}}"))
            .unwrap_or_else(|| "\\resizebox{0.55\\linewidth}{!}".into());
        format!(
            "{wrapper}{{\\setlength{{\\unitlength}}{{1bp}}\\begin{{picture}}({w},{h})\n\\put(0,0){{\\includegraphics[width={w}bp,height={h}bp]{{{id}.pdf}}}}%\n"
        )
    } else if let Some(depth) = depth {
        format!(
            "#{{ let s = 1pt; box(baseline: (at: bottom, shift: {depth}pt), width: {w}pt, height: {h}pt)[#image(\"{id}.pdf\", width: {w}pt, height: {h}pt)\n"
        )
    } else {
        format!(
            "#layout(size => {{ let s = size.width * 0.55 / {w}; box(width: {w} * s, height: {h} * s)[#image(\"{id}.pdf\", width: {w} * s, height: {h} * s)\n"
        )
    }
}
fn annotation(
    out: &mut String,
    [x, y, right, top]: [f64; 4],
    (target, external): (&str, bool),
    h: f64,
    latex: bool,
) -> std::io::Result<()> {
    let (width, height) = (right - x, top - y);
    if width < 0.0 || height < 0.0 {
        return Err(std::io::Error::other("inverted link rectangle"));
    }
    if latex {
        let action = if external {
            let hex: String = target.bytes().map(|b| format!("{b:02x}")).collect();
            format!("/S /URI /URI <{hex}>")
        } else if target.starts_with("sch:") {
            format!("/S /GoTo /D ({target})")
        } else {
            format!("/S /GoTo /D (\\getrefbykeydefault{{{target}}}{{anchor}}{{}})")
        };
        // Empty hyperref boxes are dropped by xdvipdfmx. An explicit bounded
        // annotation preserves the click rectangle without painting over the PDF.
        let _ = writeln!(
            out,
            "\\put({x},{y}){{\\special{{pdf:ann width {width}bp height {height}bp depth 0bp << /Type /Annot /Subtype /Link /Border [0 0 0] /A << {action} >> >>}}}}%"
        );
    } else {
        let dest = if external {
            format!("\"{target}\"")
        } else {
            format!("label(\"{target}\")")
        };
        let _ = writeln!(
            out,
            "#place(top + left, dx: {x} * s, dy: {} * s, link({dest}, box(width: {width} * s, height: {height} * s)))",
            h - top
        );
    }
    Ok(())
}
fn anchor(out: &mut String, label: &str, [x, y]: [f64; 2], height: f64, latex: bool) {
    if latex {
        let _ = writeln!(out, "\\put({x},{y}){{\\hypertarget{{{label}}}{{}}}}%");
    } else {
        let _ = writeln!(
            out,
            // Typst 0.15.1 pos_to_xyz subtracts 10pt for PDF viewing. The
            // imported destination already includes its source viewing offset.
            "#place(top + left, dx: {x} * s, dy: {} * s + {TYPST_DESTINATION_MARGIN_PT}pt)[#metadata(none) <{label}>]",
            height - y
        );
    }
}
