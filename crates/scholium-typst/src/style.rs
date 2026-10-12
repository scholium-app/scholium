//! The one definition of the document style. The A4 preview source (preamble)
//! and the editor's Content styles are both derived from these constants, so
//! the editing surface and the output cannot drift apart.

use std::fmt::Write;

/// Latin serif plus the classic Chinese thesis pairing for body text. Taken
/// from system-installed fonts; typst-kit embedded fonts are the fallback
/// when a family is missing.
pub const BODY_FONTS: &[&str] = &["Times New Roman", "SimSun"];
/// Heading pairing: Latin serif with the Chinese heading face.
pub const HEADING_FONTS: &[&str] = &["Times New Roman", "SimHei"];

/// Body text size in pt.
pub const BODY_SIZE_PT: f64 = 12.0;
/// Line gap, in em of the text size.
pub const LEADING_EM: f64 = 1.0;
/// First-line indent of every paragraph, in em.
pub const FIRST_LINE_INDENT_EM: f64 = 2.0;
/// Document language.
pub const LANG: &str = "zh";
/// Document region.
pub const REGION: &str = "cn";

/// A4 paper width in mm.
pub const PAGE_WIDTH_MM: f64 = 210.0;
/// Left and right page margin in cm.
pub const MARGIN_X_CM: f64 = 2.5;
/// Top and bottom page margin in cm.
pub const MARGIN_Y_CM: f64 = 2.54;

const PT_PER_CM: f64 = 72.0 / 2.54;

/// Horizontal page margin in pt.
#[must_use]
pub fn margin_x_pt() -> f64 {
    MARGIN_X_CM * PT_PER_CM
}

/// Vertical page margin in pt.
#[must_use]
pub fn margin_y_pt() -> f64 {
    MARGIN_Y_CM * PT_PER_CM
}

/// Text column width between the horizontal margins, in pt.
#[must_use]
pub fn text_width_pt() -> f64 {
    PAGE_WIDTH_MM / 10.0 * PT_PER_CM - 2.0 * margin_x_pt()
}

fn font_list(fonts: &[&str]) -> String {
    let quoted: Vec<String> = fonts.iter().map(|font| format!("\"{font}\"")).collect();
    quoted.join(", ")
}

/// Typst source preamble for the A4 preview. Heading numbering is
/// deliberately absent: the editor lays out with an empty introspector and
/// cannot run counters, so numbering returns with document templates.
#[must_use]
pub fn preamble() -> String {
    let mut out = String::new();
    // Writing to a String cannot fail.
    let _ = write!(
        out,
        "#set page(paper: \"a4\", margin: (x: {MARGIN_X_CM}cm, y: {MARGIN_Y_CM}cm), numbering: \"1\")\n\
         #set text(font: ({body}), size: {BODY_SIZE_PT}pt, lang: \"{LANG}\", region: \"{REGION}\")\n\
         #show heading: set text(font: ({heading}), weight: \"bold\")\n\
         #set par(justify: true, leading: {LEADING_EM}em, first-line-indent: (amount: {FIRST_LINE_INDENT_EM}em, all: true))",
        body = font_list(BODY_FONTS),
        heading = font_list(HEADING_FONTS),
    );
    out
}
