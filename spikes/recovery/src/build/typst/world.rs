//! 最小 Typst `World`：单文件、无文件系统、无网络、`today` 返回 `None`。
//!
//! 与 `spikes/typst-mapping/src/world.rs` 的做法一致：源码放在 `Mutex` 里以便原地替换。
//! 本 spike 只做单次编译，不依赖 comemo 缓存命中，但保留同一结构可以让"隔离构建"
//! 与既有的映射 spike 用同一套 World 语义，减少两处行为漂移。

use std::sync::Mutex;

use typst::Library;
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst_kit::fonts::FontStore;

/// 只服务单文件编译的最小 World。
pub struct SpikeWorld {
    library: LazyHash<Library>,
    fonts: FontStore,
    main: FileId,
    source: Mutex<Source>,
}

impl SpikeWorld {
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
            // 锁中毒时取回内部值：这里没有不变量会被破坏（Source 是不可变快照）。
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
        // 单文件编译：除主文件外不解析任何资源。正式实现需要受控的资源解析。
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
