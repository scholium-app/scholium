//! P0 spike 用的最小 World 实现。
//!
//! 只做验证需要的事：单个内存源文件 + typst-assets 自带字体。
//! 不处理 import、包管理、系统字体查找——那些会干扰验证结论。

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt};

pub struct SpikeWorld {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    main: Source,
}

impl SpikeWorld {
    pub fn new(text: impl Into<String>) -> Self {
        let fonts: Vec<Font> = typst_assets::fonts()
            .flat_map(|bytes| {
                let buffer = Bytes::new(bytes.to_vec());
                // 一个文件可能含多个 face（ttc），全部展开
                (0..).map_while(move |i| Font::new(buffer.clone(), i))
            })
            .collect();

        let book = FontBook::from_fonts(&fonts);
        let vpath = VirtualPath::new("main.typ").expect("字面量路径必然合法");
        let id = FileId::new(RootedPath::new(VirtualRoot::Project, vpath));

        Self {
            library: LazyHash::new(Library::default()),
            book: LazyHash::new(book),
            fonts,
            main: Source::new(id, text.into()),
        }
    }

    /// 替换正文，模拟一次编辑。返回被替换的字节范围以便对照。
    pub fn set_text(&mut self, text: impl Into<String>) {
        let id = self.main.id();
        self.main = Source::new(id, text.into());
    }

    /// 在指定字节偏移插入一段文本，模拟单字符编辑。
    pub fn edit(&mut self, at: usize, insert: &str) {
        self.main.edit(at..at, insert);
    }

    pub fn source_ref(&self) -> &Source {
        &self.main
    }

    pub fn font_count(&self) -> usize {
        self.fonts.len()
    }
}

impl typst::World for SpikeWorld {
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
        // 索引可能来自过期的 font book，越界返回 None 而非 panic
        self.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}
