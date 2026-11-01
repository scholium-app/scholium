//! Source-byte line fingerprints for a fixed two-page mixed document.

use super::FontProfile;
use crate::PreviewWorld;
use typst::WorldExt;
use typst::layout::{Frame, FrameItem, Point};
use typst_layout::PagedDocument;

fn fixture() -> String {
    let mut source = String::new();
    for _ in 0..2 {
        source.push_str("= Portable typography 中英混排\n\n");
        for _ in 0..7 {
            source.push_str(
                "中文排版需要稳定的字宽与断行。Portable typesetting keeps the same words on every line, \
                 including #strong[bold English 粗体] and #emph[italic English 强调] text. \
                 行内公式 $x + y = z$ 与正文使用同一排版引擎，所有源位置都以 UTF-8 byte 计数。\n\n",
            );
        }
    }
    source
}

/// Page → each physical line's first UTF-8 byte in `fixture()` (preamble excluded).
fn fingerprint(profile: FontProfile, latin: &str, cjk: &str) -> Vec<Vec<usize>> {
    let mut world = PreviewWorld::with_fonts(profile);
    // Only the named comparison families enter this otherwise host-independent World.
    if profile == FontProfile::Bundled && (latin != "Liberation Serif" || cjk != "Noto Sans CJK SC")
    {
        world
            .fonts
            .extend(typst_kit::fonts::system().filter(|(_, info)| {
                [latin, cjk]
                    .iter()
                    .any(|name| info.family.eq_ignore_ascii_case(name))
                    && !["Liberation Serif", "Noto Sans CJK SC"].contains(&info.family.as_str())
            }));
    }
    let override_font = if latin == "Liberation Serif" && cjk == "Noto Sans CJK SC" {
        String::new()
    } else {
        format!("#set text(font: (\"{latin}\", \"{cjk}\"))\n")
    };
    let prefix = format!(
        "{}\n{override_font}#set page(numbering: none)\n\n",
        crate::preamble()
    );
    let start = prefix.len();
    world.set_source(prefix + &fixture());
    let output = typst::compile::<PagedDocument>(&world)
        .output
        .expect("mixed document compiles");
    assert_eq!(output.pages().len(), 2, "fixture is exactly two pages");
    let mut families = std::collections::BTreeSet::new();
    let pages = output
        .pages()
        .iter()
        .map(|page| line_starts(&world, &page.frame, start, &mut families))
        .collect();
    assert!(
        families.contains(&latin.to_lowercase()),
        "Latin face actually used: {families:?}"
    );
    assert!(
        families.contains(&cjk.to_lowercase()),
        "CJK face actually used: {families:?}"
    );
    pages
}

fn line_starts(
    world: &PreviewWorld,
    frame: &Frame,
    start: usize,
    families: &mut std::collections::BTreeSet<String>,
) -> Vec<usize> {
    let mut runs = Vec::new();
    collect(world, frame, Point::zero(), start, &mut runs, families);
    runs.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let mut lines: Vec<(f64, usize)> = Vec::new();
    for (y, _, byte) in runs {
        if lines.last().is_none_or(|(last, _)| (y - last).abs() > 0.5) {
            lines.push((y, byte));
        }
    }
    lines.into_iter().map(|(_, byte)| byte).collect()
}

fn collect(
    world: &PreviewWorld,
    frame: &Frame,
    offset: Point,
    start: usize,
    runs: &mut Vec<(f64, f64, usize)>,
    families: &mut std::collections::BTreeSet<String>,
) {
    for (pos, item) in frame.items() {
        let at = offset + *pos;
        match item {
            FrameItem::Group(group) => collect(world, &group.frame, at, start, runs, families),
            FrameItem::Text(text) => {
                families.insert(text.font.info().family.to_lowercase());
                if let Some(glyph) = text.glyphs.first()
                    && let Some(range) = world.range(glyph.span.0)
                    && let Some(byte) = (range.start + usize::from(glyph.span.1)).checked_sub(start)
                {
                    runs.push((at.y.to_pt(), at.x.to_pt(), byte));
                }
            }
            _ => {}
        }
    }
}

#[test]
fn bundled_two_page_mixed_document_keeps_every_line_start() {
    let first = fingerprint(FontProfile::Bundled, "Liberation Serif", "Noto Sans CJK SC");
    let second = fingerprint(FontProfile::Bundled, "Liberation Serif", "Noto Sans CJK SC");
    assert_eq!(first, second);
    assert_eq!(first.iter().map(Vec::len).sum::<usize>(), 44);
    let recorded: Vec<Vec<usize>> =
        serde_json::from_str(include_str!("line-starts.json")).expect("recorded line starts");
    assert_eq!(first, recorded, "every line's first source byte");
}

#[test]
#[ignore = "local measurement: requires installed Noto Serif CJK SC"]
fn installed_noto_serif_and_bundled_sans_keep_every_line_start() {
    let sans = fingerprint(FontProfile::Bundled, "Liberation Serif", "Noto Sans CJK SC");
    let serif = fingerprint(
        FontProfile::Bundled,
        "Liberation Serif",
        "Noto Serif CJK SC",
    );
    println!("Noto Sans CJK SC: {sans:?}\nNoto Serif CJK SC: {serif:?}");
    assert_eq!(sans, serif, "CJK body face must keep every line start");
}

#[test]
#[ignore = "local measurement: requires installed Times New Roman"]
fn installed_times_and_bundled_liberation_keep_every_line_start() {
    let liberation = fingerprint(FontProfile::Bundled, "Liberation Serif", "Noto Sans CJK SC");
    let times = fingerprint(FontProfile::Bundled, "Times New Roman", "Noto Sans CJK SC");
    println!("Liberation Serif: {liberation:?}\nTimes New Roman: {times:?}");
    assert_eq!(
        liberation, times,
        "Latin body face must keep every line start"
    );
}
