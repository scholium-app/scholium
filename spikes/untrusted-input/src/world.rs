//! 拒绝未声明资源的 Typst World。
/// 最小 Typst World：除主文件外**任何**文件都读不到。
pub(crate) struct SpikeWorld {
    library: typst::utils::LazyHash<typst::Library>,
    fonts: typst_kit::fonts::FontStore,
    main: typst::syntax::FileId,
    source: std::sync::Mutex<typst::syntax::Source>,
}

impl SpikeWorld {
    pub(crate) fn new(text: String) -> Self {
        let mut fonts = typst_kit::fonts::FontStore::new();
        fonts.extend(typst_kit::fonts::embedded());
        fonts.extend(typst_kit::fonts::system());
        let path = typst::syntax::RootedPath::new(
            typst::syntax::VirtualRoot::Project,
            typst::syntax::VirtualPath::new("main.typ").expect("固定路径"),
        );
        let main = path.intern();
        Self {
            library: typst::utils::LazyHash::new(<typst::Library as typst::LibraryExt>::default()),
            fonts,
            main,
            source: std::sync::Mutex::new(typst::syntax::Source::new(main, text)),
        }
    }
}

impl typst::World for SpikeWorld {
    fn library(&self) -> &typst::utils::LazyHash<typst::Library> {
        &self.library
    }

    fn book(&self) -> &typst::utils::LazyHash<typst::text::FontBook> {
        self.fonts.book()
    }

    fn main(&self) -> typst::syntax::FileId {
        self.main
    }

    fn source(
        &self,
        id: typst::syntax::FileId,
    ) -> Result<typst::syntax::Source, typst::diag::FileError> {
        if id == self.main {
            let source = self
                .source
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            return Ok(source.clone());
        }
        Err(typst::diag::FileError::NotFound(
            id.vpath().get_without_slash().into(),
        ))
    }

    fn file(
        &self,
        id: typst::syntax::FileId,
    ) -> Result<typst::foundations::Bytes, typst::diag::FileError> {
        // 关键：除主文件外一律拒绝，**不做任何磁盘访问**。
        Err(typst::diag::FileError::NotFound(
            id.vpath().get_without_slash().into(),
        ))
    }

    fn font(&self, index: usize) -> Option<typst::text::Font> {
        self.fonts.font(index)
    }

    fn today(
        &self,
        _offset: Option<typst::foundations::Duration>,
    ) -> Option<typst::foundations::Datetime> {
        None
    }
}
