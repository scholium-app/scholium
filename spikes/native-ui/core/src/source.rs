//! 源码面板：方言、可写性与大文本缓冲。
//!
//! 团队语言门禁在这里只是占位：同一时刻只有一个方言可写，其余只读。
//! 真正的切换屏障、epoch、许可与写集校验属阶段 0 第 6 项，不在本 crate。

use crate::error::EditError;

/// 源码方言。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dialect {
    /// LaTeX。
    Latex,
    /// Typst。
    Typst,
    /// Markdown。
    Markdown,
    /// BibTeX。
    Bib,
}

impl Dialect {
    /// 文件扩展名。
    pub fn extension(self) -> &'static str {
        match self {
            Dialect::Latex => "tex",
            Dialect::Typst => "typ",
            Dialect::Markdown => "md",
            Dialect::Bib => "bib",
        }
    }
}

/// 一个源码面板。
#[derive(Clone, Debug)]
pub struct SourcePane {
    dialect: Dialect,
    buffer: String,
    writable: bool,
}

impl SourcePane {
    /// 建立**只读**面板。可写性必须由团队语言控制显式开启，默认拒绝写入。
    pub fn new(dialect: Dialect, initial: impl Into<String>) -> Self {
        Self {
            dialect,
            buffer: initial.into(),
            writable: false,
        }
    }

    /// 大源码夹具。用于源码视图的打开与局部编辑采样。
    pub fn with_lines(dialect: Dialect, lines: usize) -> Self {
        const LINE: &str = "the quick brown fox jumps over the lazy dog ";
        let mut buffer = String::with_capacity(lines * (LINE.len() + 8));
        for index in 0..lines {
            buffer.push_str(LINE);
            buffer.push_str(&index.to_string());
            buffer.push('\n');
        }
        Self {
            dialect,
            buffer,
            writable: false,
        }
    }

    /// 方言。
    pub fn dialect(&self) -> Dialect {
        self.dialect
    }

    /// 当前是否可写。
    pub fn is_writable(&self) -> bool {
        self.writable
    }

    /// 设置可写性。团队语言切换时由上层调用。
    pub fn set_writable(&mut self, writable: bool) {
        self.writable = writable;
    }

    /// 全部文本。
    pub fn text(&self) -> &str {
        &self.buffer
    }

    /// 字节长度。
    pub fn len_bytes(&self) -> usize {
        self.buffer.len()
    }

    /// 行数。
    pub fn line_count(&self) -> usize {
        self.buffer.lines().count()
    }

    /// 某行首的字节偏移。行号从 0 开始。
    pub fn line_offset(&self, line: usize) -> Option<usize> {
        if line == 0 {
            return Some(0);
        }
        let mut seen = 0usize;
        for (index, byte) in self.buffer.bytes().enumerate() {
            if byte == b'\n' {
                seen += 1;
                if seen == line {
                    return Some(index + 1);
                }
            }
        }
        None
    }

    /// 局部替换。只读或偏移非法时拒绝，且不修改缓冲。
    pub fn replace(&mut self, start: usize, end: usize, text: &str) -> Result<(), EditError> {
        if !self.writable {
            return Err(EditError::SourceReadOnly {
                dialect: self.dialect,
            });
        }
        if !self.buffer.is_char_boundary(start) {
            return Err(EditError::InvalidSourceOffset { offset: start });
        }
        if start > end || end > self.buffer.len() || !self.buffer.is_char_boundary(end) {
            return Err(EditError::InvalidSourceOffset { offset: end });
        }
        self.buffer.replace_range(start..end, text);
        Ok(())
    }
}
