//! 最小 Typst `World`：多文件、可嵌入矢量 PDF 图片、字体来自 `typst-kit`。
//!
//! 这是**验证用的最小实现**：只服务受控的 spike 项目目录，不访问网络、不下载包、
//! 不提供时间（`today` 返回 `None`）。正式实现需要受控的资源解析与字体来源。
//!
//! 与 `spikes/typst-mapping/src/world.rs` 的差别：
//! - 支持任意多个虚拟文件（`#include` / `#import` / 组件源文件）；
//! - `file()` 能返回真实字节，因此 `image("…pdf")` 可以嵌入矢量 PDF（Typst 0.15 支持）。

use std::collections::BTreeMap;
use std::sync::Mutex;

use typst::Library;
use typst::LibraryExt;
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst_kit::fonts::FontStore;

/// 虚拟文件系统里的一项。
pub(crate) enum Entry {
    /// 文本源码（`.typ`）。
    Text(String),
    /// 二进制资源（图片）。
    Bytes(Vec<u8>),
}

/// 只服务受控项目目录的最小 Typst World。
pub(crate) struct World {
    library: LazyHash<Library>,
    fonts: FontStore,
    main: FileId,
    files: Mutex<BTreeMap<String, Entry>>,
}

impl World {
    /// 用主文件源码建立 World；字体取自内置与系统字体。
    pub(crate) fn new(main_source: String) -> Self {
        let mut fonts = FontStore::new();
        fonts.extend(typst_kit::fonts::embedded());
        fonts.extend(typst_kit::fonts::system());
        let main = file_id("main.typ");
        let mut files = BTreeMap::new();
        files.insert("main.typ".to_string(), Entry::Text(main_source));
        Self {
            library: LazyHash::new(Library::default()),
            fonts,
            main,
            files: Mutex::new(files),
        }
    }

    /// 写入或覆盖一个虚拟文件。`path` 是相对项目根的路径。
    pub(crate) fn write_text(&self, path: &str, text: String) {
        if let Ok(mut files) = self.files.lock() {
            files.insert(path.to_string(), Entry::Text(text));
        }
    }

    /// 写入或覆盖一个虚拟二进制资源。
    pub(crate) fn write_bytes(&self, path: &str, bytes: Vec<u8>) {
        if let Ok(mut files) = self.files.lock() {
            files.insert(path.to_string(), Entry::Bytes(bytes));
        }
    }
}

impl typst::World for World {
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
        let key = id.vpath().get_without_slash().to_string();
        // 锁中毒时取回内部值：这里没有不变量会被破坏。
        let files = self
            .files
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match files.get(&key) {
            Some(Entry::Text(text)) => Ok(Source::new(id, text.clone())),
            _ => Err(FileError::NotFound(key.into())),
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        let key = id.vpath().get_without_slash().to_string();
        let files = self
            .files
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match files.get(&key) {
            Some(Entry::Bytes(bytes)) => Ok(Bytes::new(bytes.clone())),
            _ => Err(FileError::NotFound(key.into())),
        }
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        // 不提供时间：文档里出现 `today()` 会明确失败，而不是给出不稳定结果。
        None
    }
}

/// 为相对项目根的路径构造 `FileId`。
pub(crate) fn file_id(path: &str) -> FileId {
    RootedPath::new(
        VirtualRoot::Project,
        VirtualPath::new(path).expect("项目内相对路径总是合法"),
    )
    .intern()
}
