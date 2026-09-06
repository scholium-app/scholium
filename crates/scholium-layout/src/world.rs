use scholium_doc::Document;
use scholium_serialize::SourceMap;
use typst::diag::{FileError, FileResult, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt};
use typst_layout::PagedDocument;

use crate::InteractionMap;

/// Output of a successful compilation.
#[derive(Debug, Clone)]
pub struct CompileOutput {
    /// The compiled document.
    pub doc: PagedDocument,
    /// Source map from the serialized Typst source.
    pub source_map: SourceMap,
    /// Compilation warnings (if any).
    pub warnings: Vec<SourceDiagnostic>,
    /// Glyph geometry used for hit testing and caret placement.
    pub interaction: InteractionMap,
}

impl CompileOutput {
    /// Number of pages in the compiled document.
    pub fn page_count(&self) -> usize {
        self.doc.pages().len()
    }
}

/// Configuration for font loading in the Typst World.
#[derive(Debug, Clone)]
pub struct FontConfig {
    /// Additional font paths to search for CJK support.
    ///
    /// Typst-assets provides Latin and math fonts but no CJK fonts.
    /// Without a CJK font, Chinese characters render as `.notdef`.
    pub cjk_font_paths: Vec<std::path::PathBuf>,
}

impl FontConfig {
    /// Create a `FontConfig` with default CJK font search paths.
    ///
    /// The default paths cover common Linux system locations for
    /// Noto Serif CJK. Use `FontConfig::with_paths` for custom paths.
    pub fn default_cjk() -> Self {
        Self {
            cjk_font_paths: vec![
                "/usr/share/fonts/noto-cjk/NotoSerifCJK-Regular.ttc".into(),
                "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc".into(),
                "/usr/share/fonts/opentype/noto/NotoSerifCJK-Regular.ttc".into(),
                "/usr/share/fonts/noto/NotoSerifCJK-Regular.ttc".into(),
                "/System/Library/Fonts/PingFang.ttc".into(),
                "/System/Library/Fonts/STHeiti Medium.ttc".into(),
                r"C:\Windows\Fonts\msyh.ttc".into(),
                r"C:\Windows\Fonts\simsun.ttc".into(),
            ],
        }
    }

    /// Create a `FontConfig` with explicit CJK font paths.
    pub fn with_paths(paths: Vec<std::path::PathBuf>) -> Self {
        Self {
            cjk_font_paths: paths,
        }
    }

    /// Empty config — no CJK fonts.
    pub fn none() -> Self {
        Self {
            cjk_font_paths: Vec::new(),
        }
    }
}

/// The Typst `World` for Scholium.
///
/// Manages fonts and source content. The `main` source is updated on each
/// compilation request from the serialized document AST.
pub struct ScholiumWorld {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    main: Source,
}

impl ScholiumWorld {
    /// Create a new `ScholiumWorld` with the given font configuration.
    ///
    /// Loads built-in fonts from `typst-assets` and attempts to load CJK
    /// fonts from the configured paths. Prints a warning if no CJK font
    /// is found.
    ///
    /// # Panics
    /// Panics only if the built-in literal source path is invalid.
    pub fn new(config: &FontConfig) -> Self {
        let mut fonts: Vec<Font> = typst_assets::fonts()
            .flat_map(|bytes| {
                let buffer = Bytes::new(bytes.to_vec());
                (0..).map_while(move |i| Font::new(buffer.clone(), i))
            })
            .collect();

        let builtin = fonts.len();

        for path in &config.cjk_font_paths {
            if let Ok(data) = std::fs::read(path) {
                let buffer = Bytes::new(data);
                fonts.extend((0..).map_while(|i| Font::new(buffer.clone(), i)));
                break;
            }
        }

        if fonts.len() == builtin {
            let path_display: Vec<_> = config
                .cjk_font_paths
                .iter()
                .map(|p| p.display().to_string())
                .collect();
            eprintln!(
                "warn: no CJK font found — Chinese characters will render as .notdef \
                 (searched: {})",
                path_display.join(", ")
            );
        }

        let book = FontBook::from_fonts(&fonts);
        assert_coverage(&book, &fonts);

        let vpath = VirtualPath::new("main.typ").expect("literal path is valid");
        let id = FileId::new(RootedPath::new(VirtualRoot::Project, vpath));
        let main = Source::new(id, String::new());

        Self {
            library: LazyHash::new(Library::default()),
            book: LazyHash::new(book),
            fonts,
            main,
        }
    }

    /// Number of loaded fonts.
    pub fn font_count(&self) -> usize {
        self.fonts.len()
    }
}

impl typst::World for ScholiumWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    fn main(&self) -> FileId {
        self.main.id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main.id() {
            Ok(self.main.clone())
        } else {
            Err(FileError::NotFound(id.vpath().get_without_slash().into()))
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        Err(FileError::NotFound(id.vpath().get_without_slash().into()))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}

/// Serialize a `Document` and compile it with Typst.
///
/// Returns the compiled `PagedDocument` and `SourceMap` for reverse
/// mapping from `Span` → byte range → `NodeId`.
///
/// # Errors
/// Returns Typst compilation errors if the serialized source is invalid.
pub fn compile(
    world: &mut ScholiumWorld,
    doc: &Document,
) -> Result<CompileOutput, Vec<SourceDiagnostic>> {
    let (source_text, source_map) = scholium_serialize::serialize(doc);

    let id = world.main.id();
    world.main = Source::new(id, source_text);

    let result = typst::compile::<PagedDocument>(world);

    match result.output {
        Ok(doc) => {
            let interaction = InteractionMap::build(world, &doc, &source_map);
            Ok(CompileOutput {
                doc,
                source_map,
                warnings: result.warnings.to_vec(),
                interaction,
            })
        }
        Err(errors) => Err(errors.to_vec()),
    }
}

/// Probe font coverage to detect missing CJK support early.
///
/// Typst does not report missing glyphs — it silently compiles and returns
/// `.notdef` (tofu) glyphs. This function warns at startup if critical
/// codepoints are not covered by any loaded font.
fn assert_coverage(book: &FontBook, fonts: &[Font]) {
    let probes: &[(char, &str)] = &[('标', "CJK"), ('π', "math")];
    for &(ch, label) in probes {
        let covered = fonts.iter().any(|f| f.info().coverage.contains(ch as u32));
        if !covered {
            eprintln!(
                "warn: {label} character {ch:?} not covered by any of {} font families",
                book.families().count()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scholium_doc::{Document, EditOp, NodeKind};

    #[test]
    fn compiles_empty_document() {
        let config = FontConfig::none();
        let mut world = ScholiumWorld::new(&config);
        let doc = Document::new();
        let result = compile(&mut world, &doc);
        assert!(
            result.is_ok(),
            "empty document should compile: {:?}",
            result.err()
        );
        let output = result.expect("empty document compiles");
        assert_eq!(output.page_count(), 1);
    }

    #[test]
    fn compiles_simple_paragraph() {
        let config = FontConfig::none();
        let mut world = ScholiumWorld::new(&config);
        let mut doc = Document::new();
        let root = doc.root();
        let p = doc.append_child(root, NodeKind::Paragraph, None);
        doc.append_child(p, NodeKind::Text, Some("Hello, world!".to_string()));
        let result = compile(&mut world, &doc);
        assert!(
            result.is_ok(),
            "paragraph should compile: {:?}",
            result.err()
        );
    }

    #[test]
    fn compiles_heading() {
        let config = FontConfig::none();
        let mut world = ScholiumWorld::new(&config);
        let mut doc = Document::new();
        let root = doc.root();
        let h = doc.append_child(root, NodeKind::Heading, None);
        doc.node_mut(h).heading_level = Some(1);
        doc.append_child(h, NodeKind::Text, Some("Title".to_string()));
        let result = compile(&mut world, &doc);
        assert!(result.is_ok(), "heading should compile: {:?}", result.err());
    }

    #[test]
    fn compile_returns_source_map() {
        let config = FontConfig::none();
        let mut world = ScholiumWorld::new(&config);
        let mut doc = Document::new();
        let root = doc.root();
        let p = doc.append_child(root, NodeKind::Paragraph, None);
        doc.append_child(p, NodeKind::Text, Some("Content".to_string()));
        let output = compile(&mut world, &doc).expect("paragraph compiles");
        assert!(output.source_map.len() >= 2);
    }

    #[test]
    fn incremental_compile_updates_source() {
        let config = FontConfig::none();
        let mut world = ScholiumWorld::new(&config);

        // First compile
        let mut doc = Document::new();
        let root = doc.root();
        let p = doc.append_child(root, NodeKind::Paragraph, None);
        let t = doc.append_child(p, NodeKind::Text, Some("First".to_string()));
        let _ = compile(&mut world, &doc).expect("first compile succeeds");

        // Update and recompile
        doc.node_mut(t).text = Some("Second".to_string());
        let result = compile(&mut world, &doc);
        assert!(result.is_ok());
    }

    #[test]
    fn compiles_every_math_structure() {
        // serialization syntax (`lr()`, `root()`, limit scripts, quoting)
        // is only proven by the real compiler — P0 lesson: docs.rs lies
        let config = FontConfig::none();
        let mut world = ScholiumWorld::new(&config);

        let mut doc = Document::new();
        let p = doc.append_child(doc.root(), NodeKind::Paragraph, None);

        // $frac(x, 2)$
        let m = doc.append_math(p, false);
        let row = doc.node(m).children[0];
        let x = doc.append_math_symbol(row, "x");
        doc.apply_op(&EditOp::WrapNode {
            id: x,
            wrapper: NodeKind::MathFrac,
        })
        .expect("wrap frac");
        let frac = doc.node(row).children[0];
        doc.append_math_symbol(doc.node(frac).children[1], "2");

        // $sum_(i = 1)^n$
        let m2 = doc.append_math(p, false);
        let row2 = doc.node(m2).children[0];
        let sum = doc.append_math_symbol(row2, "sum");
        doc.apply_op(&EditOp::WrapNode {
            id: sum,
            wrapper: NodeKind::MathBigOp,
        })
        .expect("wrap bigop");
        let big = doc.node(row2).children[0];
        let lower = doc.node(big).children[1];
        for name in ["i", "=", "1"] {
            doc.append_math_symbol(lower, name);
        }
        doc.append_math_symbol(doc.node(big).children[2], "n");

        // $lr(x, left: "(", right: ")")$
        let m3 = doc.append_math(p, false);
        let row3 = doc.node(m3).children[0];
        let y = doc.append_math_symbol(row3, "x");
        doc.apply_op(&EditOp::WrapNode {
            id: y,
            wrapper: NodeKind::MathDelimited,
        })
        .expect("wrap delimited");

        // $root(3, x)$
        let m4 = doc.append_math(p, false);
        let row4 = doc.node(m4).children[0];
        let z = doc.append_math_symbol(row4, "x");
        doc.apply_op(&EditOp::WrapNode {
            id: z,
            wrapper: NodeKind::MathRoot,
        })
        .expect("wrap root");
        let root_node = doc.node(row4).children[0];
        let deg = doc.append_math_row(root_node);
        doc.append_math_symbol(deg, "3");

        // display env: $ x + 1 $
        let m5 = doc.append_math(p, true);
        let row5 = doc.node(m5).children[0];
        for name in ["x", "+", "1"] {
            doc.append_math_symbol(row5, name);
        }

        let result = compile(&mut world, &doc);
        assert!(
            result.is_ok(),
            "math document should compile: {:?}",
            result.err()
        );
    }
}
