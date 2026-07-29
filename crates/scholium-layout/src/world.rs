use scholium_doc::Document;
use scholium_serialize::SourceMap;
use typst::diag::{FileError, FileResult, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt};
use typst_layout::PagedDocument;

/// Output of a successful compilation.
#[derive(Debug, Clone)]
pub struct CompileOutput {
    /// The compiled document.
    pub doc: PagedDocument,
    /// Source map from the serialized Typst source.
    pub source_map: SourceMap,
    /// Compilation warnings (if any).
    pub warnings: Vec<SourceDiagnostic>,
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
    /// Panics if no GPU-compatible rendering backend is available (unlikely on
    /// regular desktop/server environments).
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
        Ok(doc) => Ok(CompileOutput {
            doc,
            source_map,
            warnings: result.warnings.to_vec(),
        }),
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
    use scholium_doc::{Document, NodeKind};

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
}
