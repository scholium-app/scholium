//! 外部源码修改冲突：加载时记内容哈希，保存前重算，不一致就**拒绝写入**并给可读报告。
//!
//! 为什么不加"是否覆盖"的开关：判据要求"拒绝静默覆盖"，静默的部分正是"覆盖"，
//! 只要提供 `force` 就等于把默认行为交给调用方记不记得传参。本 spike 的保存只有
//! 一个出口：哈希一致才写。外部改动只能通过显式的接受动作处理，那是产品设计
//! （报告"对设计的影响"一节讨论），不是验证代码的职责。
//!
//! 用 SHA-256 而不是 mtime：mtime 会被 `touch`、rsync、编辑器原子保存（rename）
//! 误伤或漏报；内容哈希才是"源码是否真的变了"的判据。代价是每次保存要重读全文件，
//! 对源码规模可以接受。

use std::path::{Path, PathBuf};

use crate::error::{Result, io_context};
use crate::fsutil;

/// 一条已加载并进入编辑的源码。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedSource {
    /// 文件路径。
    pub path: PathBuf,
    /// 加载时的内容。
    pub text: String,
    /// 加载时的 SHA-256。
    pub loaded_hash: String,
    /// 加载时的字节数。
    pub loaded_bytes: usize,
}

/// 为什么拒绝保存。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    /// 磁盘内容与加载时不一致。
    ExternalChange {
        /// 加载时的哈希。
        loaded: String,
        /// 当前的哈希。
        current: String,
    },
    /// 磁盘内容与加载时一致，但编辑后的内容没有变化，无需写盘。
    Unchanged,
    /// 文件在编辑期间被删除。
    Removed,
}

impl Rejection {
    /// 判定名，进报告。
    pub fn label(&self) -> &'static str {
        match self {
            Self::ExternalChange { .. } => "external-change",
            Self::Unchanged => "unchanged",
            Self::Removed => "removed",
        }
    }
}

/// 保存尝试的结果。`Written` 之外都是拒写。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveAttempt {
    /// 校验通过并写入，携带新内容哈希。
    Written {
        /// 写入后的内容哈希。
        new_hash: String,
        /// 写入的字节数。
        bytes: usize,
    },
    /// 拒绝写入，附原因。
    Rejected(Rejection),
}

impl SaveAttempt {
    /// 是否发生了写盘。
    pub fn wrote(&self) -> bool {
        matches!(self, Self::Written { .. })
    }
}

impl TrackedSource {
    /// 从磁盘加载。
    ///
    /// # Errors
    ///
    /// 文件不存在或不可读。
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = fsutil::read_file(path)?;
        Ok(Self {
            path: path.to_path_buf(),
            text: String::from_utf8_lossy(&bytes).to_string(),
            loaded_hash: fsutil::sha256_hex(&bytes),
            loaded_bytes: bytes.len(),
        })
    }

    /// 当前磁盘内容哈希；文件不存在返回 `None`。
    ///
    /// # Errors
    ///
    /// 文件存在但不可读。
    pub fn current_hash(&self) -> Result<Option<String>> {
        match fsutil::hash_file(&self.path) {
            Ok(hash) => Ok(Some(hash)),
            Err(crate::error::SpikeError::Io { source, .. })
                if source.kind() == std::io::ErrorKind::NotFound =>
            {
                Ok(None)
            }
            Err(other) => Err(other),
        }
    }

    /// 只做校验，不写入。
    ///
    /// # Errors
    ///
    /// 读磁盘失败（除"文件不存在"以外）。
    pub fn check(&self) -> Result<std::result::Result<(), Rejection>> {
        match self.current_hash()? {
            None => Ok(Err(Rejection::Removed)),
            Some(current) if current != self.loaded_hash => Ok(Err(Rejection::ExternalChange {
                loaded: self.loaded_hash.clone(),
                current,
            })),
            Some(_) => Ok(Ok(())),
        }
    }

    /// 尝试保存新内容。校验通过才写盘；否则返回 [`SaveAttempt::Rejected`]。
    ///
    /// # Errors
    ///
    /// 磁盘读失败，或校验通过但写盘失败。
    pub fn try_save(&self, new_text: &str) -> Result<SaveAttempt> {
        match self.check()? {
            Err(rejection) => Ok(SaveAttempt::Rejected(rejection)),
            Ok(()) => {
                let new_bytes = new_text.as_bytes();
                let new_hash = fsutil::sha256_hex(new_bytes);
                if new_hash == self.loaded_hash {
                    return Ok(SaveAttempt::Rejected(Rejection::Unchanged));
                }
                fsutil::write_file(&self.path, new_bytes)?;
                Ok(SaveAttempt::Written {
                    new_hash,
                    bytes: new_bytes.len(),
                })
            }
        }
    }
}

/// 把拒写原因渲染成**可读报告**。
///
/// 报告必须让人能在不重新运行程序的情况下判断"是编辑器的自动保存，还是别人改了我的文件"，
/// 所以它同时给出：加载与当前哈希、字节数变化、差异行号、差异行内容与行内位置。
pub struct ConflictReport {
    /// 文件路径。
    pub path: PathBuf,
    /// 拒写判定。
    pub rejection: Rejection,
    /// 当前磁盘内容（读不到时为空）。
    pub current_text: String,
    /// 差异行（1-based 行号）。
    pub diff_lines: Vec<DiffLine>,
}

/// 单行差异。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    /// 1-based 行号。
    pub line: usize,
    /// 加载时的行内容。
    pub loaded: String,
    /// 当前的行内容。
    pub current: String,
    /// 首个不同的字符下标（按字符计，不是字节）。
    pub first_diff_char: Option<usize>,
}

impl ConflictReport {
    /// 生成报告；文件被删除时 `current_text` 为空且没有差异行。
    ///
    /// # Errors
    ///
    /// 读磁盘失败。
    pub fn build(source: &TrackedSource, rejection: Rejection) -> Result<Self> {
        let current_text = fsutil::read_file(&source.path)
            .map(|bytes| String::from_utf8_lossy(&bytes).to_string())
            .unwrap_or_default();
        let diff_lines = line_diff(&source.text, &current_text);
        Ok(Self {
            path: source.path.clone(),
            rejection,
            current_text,
            diff_lines,
        })
    }

    /// 渲染为多行文本，直接进报告与 stdout。
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("源码冲突报告: {}\n", self.path.display()));
        out.push_str(&format!("  判定: {}\n", self.rejection.label()));
        match &self.rejection {
            Rejection::ExternalChange { loaded, current } => {
                out.push_str(&format!("  加载时哈希: {loaded}\n"));
                out.push_str(&format!("  当前哈希:   {current}\n"));
                out.push_str("  动作: 拒绝写入（磁盘内容由外部改动，不静默覆盖）\n");
                out.push_str(&format!("  差异行数: {}\n", self.diff_lines.len()));
                for diff in self.diff_lines.iter().take(3) {
                    let at = match diff.first_diff_char {
                        Some(index) => format!("首个差异在第 {index} 个字符"),
                        None => "整行新增或删除".to_string(),
                    };
                    out.push_str(&format!("    第 {} 行（{at}）\n", diff.line));
                    out.push_str(&format!("      - 加载: {}\n", diff.loaded));
                    out.push_str(&format!("      + 现在: {}\n", diff.current));
                }
            }
            Rejection::Removed => {
                out.push_str("  动作: 拒绝写入（文件已被外部删除）\n");
            }
            Rejection::Unchanged => {
                out.push_str("  动作: 无需写入（编辑后内容与磁盘一致）\n");
            }
        }
        out
    }
}

/// 逐行比较，返回内容不同的行。
///
/// 这是**行级**diff，不做 LCS 对齐：源码冲突报告的第一目标是让人一眼看出
/// "哪一行被谁改了"，第二目标才是美观的 hunk。行数不同时按最长补齐，
/// 缺失的一侧记为整行新增/删除。
pub fn line_diff(loaded: &str, current: &str) -> Vec<DiffLine> {
    let loaded_lines: Vec<&str> = loaded.lines().collect();
    let current_lines: Vec<&str> = current.lines().collect();
    let count = loaded_lines.len().max(current_lines.len());
    let mut diffs = Vec::new();
    for index in 0..count {
        let left = loaded_lines.get(index).copied().unwrap_or("");
        let right = current_lines.get(index).copied().unwrap_or("");
        if left != right {
            diffs.push(DiffLine {
                line: index + 1,
                loaded: left.to_string(),
                current: right.to_string(),
                first_diff_char: first_difference(left, right),
            });
        }
    }
    diffs
}

/// 首个不同的**字符**下标（不是字节下标；中文与 UTF-8 下两者不同）。
pub fn first_difference(left: &str, right: &str) -> Option<usize> {
    left.chars()
        .zip(right.chars())
        .position(|(a, b)| a != b)
        .or_else(|| {
            if left == right {
                None
            } else {
                Some(left.chars().count().min(right.chars().count()))
            }
        })
}

/// 在 `dir` 下建立一对源码路径，名字里带引擎名。
pub fn source_path(dir: &Path, job_name: &str, ext: &str) -> PathBuf {
    dir.join(format!("{job_name}.{ext}"))
}

/// 确保目录存在。
///
/// # Errors
///
/// 目录创建失败。
pub fn ensure_dir(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir).map_err(io_context(dir))
}
