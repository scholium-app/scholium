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
    /// 系统 CJK 字体候选路径。
    ///
    /// `typst-assets` 只捆了 New Computer Modern / Libertinus / DejaVu Mono，
    /// **一个 CJK 字体都没有**——不加载这些，所有汉字的字形 id 都是 0（`.notdef`），
    /// 而且不会报任何错：编译成功、span 正确、延迟正常，只有画到屏幕上才看得出。
    ///
    /// 正式实现要随应用分发 OFL 字体（见 PLAN §3），spike 阶段先借系统的。
    const CJK_CANDIDATES: &'static [&'static str] = &[
        "/usr/share/fonts/noto-cjk/NotoSerifCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/opentype/noto/NotoSerifCJK-Regular.ttc",
    ];

    /// 用来自检字体是否齐备的探针码位。
    const PROBE_CJK: char = '标';
    const PROBE_MATH: char = 'π';

    pub fn new(text: impl Into<String>) -> Self {
        // 内置字体：数学与拉丁文
        let mut fonts: Vec<Font> = typst_assets::fonts()
            .flat_map(|bytes| {
                let buffer = Bytes::new(bytes.to_vec());
                // 一个文件可能含多个 face（ttc），全部展开
                (0..).map_while(move |i| Font::new(buffer.clone(), i))
            })
            .collect();

        let builtin = fonts.len();

        // 系统 CJK 字体。`.ttc` 内含 SC/TC/JP/KR 多个 face，同样要全部展开。
        for path in Self::CJK_CANDIDATES {
            let Ok(data) = std::fs::read(path) else { continue };
            let buffer = Bytes::new(data);
            fonts.extend((0..).map_while(|i| Font::new(buffer.clone(), i)));
            break; // 一个够了，多加只是拖慢 FontBook
        }

        if fonts.len() == builtin {
            eprintln!("警告：未找到 CJK 字体，汉字将渲染为 .notdef（字形 id=0）");
        }

        let book = FontBook::from_fonts(&fonts);
        Self::assert_coverage(&book, &fonts);
        let vpath = VirtualPath::new("main.typ").expect("字面量路径必然合法");
        let id = FileId::new(RootedPath::new(VirtualRoot::Project, vpath));

        Self {
            library: LazyHash::new(Library::default()),
            book: LazyHash::new(book),
            fonts,
            main: Source::new(id, text.into()),
        }
    }

    /// 字体覆盖自检。
    ///
    /// 缺字体是**静默降级**：Typst 照常排版、编译成功、span 正确、延迟正常，
    /// 只是返回 `id=0` 的 `.notdef` 字形，只有画到屏幕上才看得出问题。
    /// 所以在启动时主动探关键码位，把问题挡在这里。
    ///
    /// P0 踩过：spike 1 拿到了汉字 `id=0` 却没警觉，因为当时验的是 span 映射，
    /// 那部分确实通过了。
    fn assert_coverage(book: &FontBook, fonts: &[Font]) {
        for probe in [Self::PROBE_CJK, Self::PROBE_MATH] {
            let covered = fonts
                .iter()
                .any(|f| f.info().coverage.contains(probe as u32));
            if covered {
                continue;
            }
            eprintln!(
                "警告：{} 个 face 中没有一个覆盖 {probe:?}，该字符会渲染为 .notdef",
                book.families().count()
            );
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
