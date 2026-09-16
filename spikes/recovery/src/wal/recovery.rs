//! WAL 扫描恢复：顺序校验、停在最后一个完整记录、把尾部损坏分类。
//!
//! 恢复语义只有一条：**从文件头顺序扫描，遇到第一条不完整或校验失败的记录就停止，
//! 它之后的字节（包括"看起来完整"的后续记录）一律丢弃。**
//!
//! 为什么不做"跳过坏记录继续往后扫"：append-only 日志的坏尾几乎总是撕裂写，
//! 而撕裂写之后的"完整记录"只可能来自更早的残留或蓄意构造。跳过它会把
//! 一个物理上更晚的状态当成有效状态，恢复结果就不可解释了。宁可少恢复，不可乱恢复。
//! 这条取舍写在报告里，是有意的简化，不是遗漏。

use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

use crate::error::{Result, io_context};
use crate::wal::{HEADER_LEN, MAGIC, MAX_PAYLOAD, Record};

/// 恢复停止的原因。`None` 表示整个文件都是完整记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TailDefect {
    /// 日志文件不存在（首次运行）。
    MissingFile,
    /// 尾部不足一个头：撕裂发生在头部中间。
    TruncatedHeader {
        /// 剩余字节数。
        remaining: usize,
    },
    /// 头完整但负载不足：撕裂发生在负载中间。
    TruncatedPayload {
        /// 声明需要的负载字节数。
        declared: usize,
        /// 实际剩余字节数。
        remaining: usize,
    },
    /// 边界处魔数不匹配：不是撕裂，而是字节被改写。
    CorruptMagic {
        /// 读到的 4 字节。
        found: [u8; 4],
    },
    /// 头里的长度字段自相矛盾（小于头长或超过上限）。
    CorruptLength {
        /// 读到的总长度。
        total_len: u32,
    },
    /// CRC 校验失败：头或负载被改写。
    ChecksumMismatch {
        /// 记录头里的校验和。
        expected: u32,
        /// 实际内容的校验和。
        actual: u32,
    },
}

impl TailDefect {
    /// 人可读的单行说明，进报告。
    pub fn describe(&self) -> String {
        match self {
            Self::MissingFile => "日志不存在".to_string(),
            Self::TruncatedHeader { remaining } => {
                format!("尾部头不完整：剩余 {remaining} B，少于 {HEADER_LEN} B")
            }
            Self::TruncatedPayload {
                declared,
                remaining,
            } => format!("尾部负载不完整：声明 {declared} B，实际剩余 {remaining} B"),
            Self::CorruptMagic { found } => format!("边界魔数损坏: {found:02x?}"),
            Self::CorruptLength { total_len } => format!("长度字段损坏: {total_len}"),
            Self::ChecksumMismatch { expected, actual } => {
                format!("CRC 不匹配: 头内 {expected:#010x} vs 实算 {actual:#010x}")
            }
        }
    }
}

impl std::fmt::Display for TailDefect {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.describe())
    }
}

/// 恢复结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recovery {    /// 从文件头开始连续完整的记录。
    pub records: Vec<Record>,
    /// 最后一条完整记录的结束偏移；文件应当被截断到这里。
    pub good_bytes: usize,
    /// 停止扫描的原因；`None` 表示文件整体完整。
    pub defect: Option<TailDefect>,
    /// 被丢弃的字节数（文件长度减 `good_bytes`）。
    pub discarded_bytes: usize,
    /// 文件总长度。
    pub file_bytes: usize,
}

impl Recovery {
    /// 是否存在被丢弃的尾部。
    pub fn has_tail(&self) -> bool {
        self.discarded_bytes > 0
    }
}

/// 读取日志并恢复到最后一个完整记录。
///
/// # Errors
///
/// 日志存在但无法读取（权限、IO 错误）。
pub fn recover(path: &Path) -> Result<Recovery> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Recovery {
                records: Vec::new(),
                good_bytes: 0,
                defect: Some(TailDefect::MissingFile),
                discarded_bytes: 0,
                file_bytes: 0,
            });
        }
        Err(source) => return Err(io_context(path)(source)),
    };
    Ok(scan(&bytes))
}

/// 扫描字节并恢复。与 IO 分离，便于对内存里的构造样本做逐字节断言。
pub fn scan(bytes: &[u8]) -> Recovery {
    let mut records = Vec::new();
    let mut offset = 0usize;

    while offset < bytes.len() {
        // 用 saturating_add：偏移由文件内容驱动，不能假设它不会溢出。
        let header_end = offset.saturating_add(HEADER_LEN);
        let Some(header) = bytes.get(offset..header_end) else {
            let remaining = bytes.len() - offset;
            return finish(records, offset, bytes.len(), Some(TailDefect::TruncatedHeader { remaining }));
        };

        let magic: [u8; 4] = header[0..4].try_into().unwrap_or(MAGIC);
        if magic != MAGIC {
            return finish(records, offset, bytes.len(), Some(TailDefect::CorruptMagic { found: magic }));
        }

        let total_len = u32::from_le_bytes(header[4..8].try_into().unwrap_or([0; 4]));
        let seq = u64::from_le_bytes(header[8..16].try_into().unwrap_or([0; 8]));
        let epoch = u64::from_le_bytes(header[16..24].try_into().unwrap_or([0; 8]));
        let payload_len = u32::from_le_bytes(header[24..28].try_into().unwrap_or([0; 4]));
        let expected_crc = u32::from_le_bytes(header[28..32].try_into().unwrap_or([0; 4]));

        let consistent = total_len as usize == HEADER_LEN + payload_len as usize
            && payload_len as usize <= MAX_PAYLOAD;
        if !consistent {
            return finish(
                records,
                offset,
                bytes.len(),
                Some(TailDefect::CorruptLength { total_len }),
            );
        }

        let end = offset + total_len as usize;
        let Some(record_bytes) = bytes.get(offset..end) else {
            let remaining = bytes.len() - offset;
            return finish(
                records,
                offset,
                bytes.len(),
                Some(TailDefect::TruncatedPayload {
                    declared: total_len as usize,
                    remaining,
                }),
            );
        };

        let payload = &record_bytes[HEADER_LEN..];
        let actual_crc = crate::wal::checksum(&record_bytes[4..HEADER_LEN], payload);
        if actual_crc != expected_crc {
            return finish(
                records,
                offset,
                bytes.len(),
                Some(TailDefect::ChecksumMismatch {
                    expected: expected_crc,
                    actual: actual_crc,
                }),
            );
        }

        records.push(Record {
            seq,
            epoch,
            payload: payload.to_vec(),
        });
        offset = end;
    }

    finish(records, offset, bytes.len(), None)
}

fn finish(
    records: Vec<Record>,
    good_bytes: usize,
    file_bytes: usize,
    defect: Option<TailDefect>,
) -> Recovery {
    Recovery {
        records,
        good_bytes,
        defect,
        discarded_bytes: file_bytes - good_bytes,
        file_bytes,
    }
}

/// 把日志尾部截断到最后一个完整记录（真实恢复动作）。
///
/// # Errors
///
/// 日志无法以读写方式打开或 `set_len` 失败。
pub fn truncate_to_good(path: &Path, recovery: &Recovery) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .map_err(io_context(path))?;
    file.seek(SeekFrom::Start(recovery.good_bytes as u64))
        .map_err(io_context(path))?;
    file.set_len(recovery.good_bytes as u64)
        .map_err(io_context(path))?;
    file.flush().map_err(io_context(path))?;
    file.sync_all().map_err(io_context(path))
}
