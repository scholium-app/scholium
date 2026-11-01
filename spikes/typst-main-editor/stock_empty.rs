//! Actual stock source compilation: independent text/fixtures and no adapter dependency.
use typst::{
    Library, World,
    diag::FileResult,
    foundations::{Bytes, Datetime, Duration},
    syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot},
    text::{Font, FontBook},
    utils::LazyHash,
};

struct SourceWorld<'a> {
    fonts: &'a super::kernel::ProbeWorld,
    source: Source,
}

impl World for SourceWorld<'_> {
    fn library(&self) -> &LazyHash<Library> {
        self.fonts.library()
    }
    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.book()
    }
    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }
    fn main(&self) -> FileId {
        self.source.id()
    }
    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main() {
            Ok(self.source.clone())
        } else {
            self.fonts.source(id)
        }
    }
    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.fonts.file(id)
    }
    fn today(&self, _: Option<Duration>) -> Option<Datetime> {
        None
    }
}

fn font_list(world: &super::kernel::ProbeWorld, names: &[&str]) -> String {
    names
        .iter()
        // Source compilation warns about absent preferences even when a later
        // fallback exists; Content layout already skips these absent families.
        .filter(|name| {
            world
                .book()
                .select_family(&name.to_lowercase())
                .next()
                .is_some()
        })
        .map(|name| format!("\"{name}\""))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn page(
    fonts: &super::kernel::ProbeWorld,
    name: &str,
) -> Result<typst_layout::Page, String> {
    let empty = "#par[#text(\"\")]";
    let blocks = match name {
        "empty-start" => vec![empty, "alpha"],
        "empty-middle" => vec!["alpha", empty, "omega"],
        "empty-three" => vec!["alpha", empty, empty, empty, "omega"],
        "empty-end" => vec!["alpha", empty],
        "empty-heading" => vec![empty, "= Heading", empty, "omega"],
        "empty-between-headings" => vec!["= Heading", empty, "= Second", "omega"],
        "empty-only" => vec![empty, empty, empty],
        _ => panic!("named independent empty paragraph fixture"),
    };
    // Auto height exposes the final empty line as well as the visible text positions.
    // Same column and margins as the continuous editor, without a footer.
    let source = format!(
        "#set page(width: 21cm, height: auto, margin: (x: 2.5cm, y: 2.54cm))\n\
         #set text(font: ({body}), size: 12pt, lang: \"zh\", region: \"cn\")\n\
         #show heading: set text(font: ({heading}), weight: \"bold\")\n\
         #set par(justify: true, leading: 1em, first-line-indent: (amount: 2em, all: true))\n\n{}\n",
        blocks.join("\n\n"),
        body = font_list(fonts, &super::BODY_FONTS),
        heading = font_list(fonts, &super::HEADING_FONTS),
    );
    let id = RootedPath::new(
        VirtualRoot::Project,
        VirtualPath::new("empty.typ").expect("path"),
    )
    .intern();
    let world = SourceWorld {
        fonts,
        source: Source::new(id, source),
    };
    let compiled = typst::compile::<typst_layout::PagedDocument>(&world);
    assert!(compiled.warnings.is_empty(), "stock compile warnings");
    let doc = compiled.output.map_err(|e| format!("{e:?}"))?;
    assert_eq!(doc.pages().len(), 1);
    Ok(doc.pages()[0].clone())
}
