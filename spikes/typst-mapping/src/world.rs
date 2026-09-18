//! 最小 Typst `World`：把生成好的源码交给编译器，字体来自 `typst-kit`。
//!
//! 这是**验证用的最小实现**，只服务单文件编译：不解析文件系统、不下载包、
//! 不提供时间（`today` 返回 `None`）。正式实现需要受控的资源解析与字体来源。

use std::sync::Mutex;

use typst::Library;
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::LibraryExt;
use typst::utils::LazyHash;
use typst_kit::fonts::FontStore;

/// 只服务单文件编译的最小 World。
pub struct SpikeWorld {
    library: LazyHash<Library>,
    fonts: FontStore,
    main: FileId,
    /// 源码用 `Mutex` 提供内部可变性：**同一个 World 实例**换源码，
    /// comemo 的按 World 身份的缓存才会命中，增量编译才可能发生。
    source: Mutex<Source>,
}

impl SpikeWorld {
    /// 替换主文件源码。**不重建 World**，因此缓存仍然有效。
    pub fn set_source(&self, text: String) {
        if let Ok(mut source) = self.source.lock() {
            source.replace(&text);
        }
    }

    /// 用给定源码建立 World。字体取自内置与系统字体。
    pub fn new(text: String) -> Self {
        let mut fonts = FontStore::new();
        fonts.extend(typst_kit::fonts::embedded());
        fonts.extend(typst_kit::fonts::system());

        let path = RootedPath::new(
            VirtualRoot::Project,
            VirtualPath::new("main.typ").expect("固定路径总是合法"),
        );
        let main = path.intern();

        Self {
            library: LazyHash::new(Library::default()),
            fonts,
            main,
            source: Mutex::new(Source::new(main, text)),
        }
    }

}

impl typst::World for SpikeWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.book()
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main {
            // 锁中毒时取回内部值：这里没有不变量会被破坏。
            let source = self
                .source
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            Ok(source.clone())
        } else {
            Err(FileError::NotFound(
                id.vpath().get_without_slash().into(),
            ))
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        Err(FileError::NotFound(
            id.vpath().get_without_slash().into(),
        ))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        // 不提供时间：文档里出现 `today()` 会明确失败，而不是给出不稳定结果。
        None
    }
}
