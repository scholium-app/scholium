//! Append-only 预写日志：记录编码、追加、扫描恢复与尾部截断分类。
//!
//! 记录编码集中在这里，恢复逻辑集中在 [`recovery`]，因为"写进去的字节"与
//! "怎么判定它完整"是两个必须能分别审阅的职责。
//!
//! 磁盘布局（小端）：
//!
//! ```text
//! 偏移  长度  含义
//! 0     4    魔数 "SWAL"
//! 4     4    记录总长度 total_len（含 32 字节头）
//! 8     8    序号 seq，从 1 连续递增
//! 16    8    epoch，语言/分支纪元，恢复时要保留
//! 24    4    负载长度 payload_len
//! 28    4    CRC-32，覆盖 [4, total_len)（即除魔数外的整条记录）
//! 32    n    负载
//! ```
//!
//! CRC 覆盖头字段本身：这样"头被撕裂但长度字段恰好自洽"的情况也会被校验拦住，
//! 而不是只校验负载。魔数不参与 CRC，因为它是用来**同步**的，不是用来校验的。

pub mod recovery;

use std::fs::OpenOptions;
use std::path::Path;

use crc32fast::Hasher;

use crate::error::{Result, SpikeError, io_context};

/// 记录魔数。
pub const MAGIC: [u8; 4] = *b"SWAL";

/// 固定头长度。
pub const HEADER_LEN: usize = 32;

/// 头部中参与 CRC 的部分：`[MAGIC_LEN, HEADER_LEN - CRC_LEN)`，即长度/序号/纪元/负载长度。
///
/// 单独具名是因为这里踩过一次：`encode` 里先写 28 字节头字段再算 CRC，
/// 若把范围写成 `..HEADER_LEN` 就会越界 panic。CRC 位置在头的最后 4 字节。
pub const CRC_INPUT: std::ops::Range<usize> = 4..28;

/// 负载长度上限：1 MiB。超过说明头已损坏，而不是真有这么大的记录。
pub const MAX_PAYLOAD: usize = 1024 * 1024;

/// 一次追加操作写入的记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// 序号，从 1 开始连续。
    pub seq: u64,
    /// 纪元（对应设计里的语言 epoch）。
    pub epoch: u64,
    /// 负载字节。
    pub payload: Vec<u8>,
}

impl Record {
    /// 用给定序号与负载构造记录。
    pub fn new(seq: u64, epoch: u64, payload: impl Into<Vec<u8>>) -> Self {
        Self {
            seq,
            epoch,
            payload: payload.into(),
        }
    }

    /// 编码后的总长度。
    pub fn encoded_len(&self) -> usize {
        HEADER_LEN + self.payload.len()
    }

    /// 编码为完整记录字节。
    pub fn encode(&self) -> Vec<u8> {
        let total_len = self.encoded_len() as u32;
        let payload_len = self.payload.len() as u32;

        let mut out = Vec::with_capacity(self.encoded_len());
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&total_len.to_le_bytes());
        out.extend_from_slice(&self.seq.to_le_bytes());
        out.extend_from_slice(&self.epoch.to_le_bytes());
        out.extend_from_slice(&payload_len.to_le_bytes());
        let crc = record_crc(&out[CRC_INPUT], &self.payload);
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&self.payload);
        out
    }
}

/// 计算一条记录的 CRC-32：头字段（除魔数）加负载。
///
/// 名字带 `record_` 前缀是为了和 [`Record::checksum`] 区分：两者都在本模块可见，
/// 同名会让 `crate::wal::checksum(header, payload)` 解析到方法调用上并 panic。
pub fn record_crc(header_tail: &[u8], payload: &[u8]) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(header_tail);
    hasher.update(payload);
    hasher.finalize()
}

/// 追加写入 WAL，返回写入偏移。
///
/// 每条记录写完立即 `fsync`：崩溃恢复的正确性依赖"写成功的记录已经落盘"，
/// 缓冲在页缓存里的记录在真实断电下可能根本没有出现过。
///
/// # Errors
///
/// 目录不可写、磁盘满或 `fsync` 失败。
pub fn append(path: &Path, record: &Record) -> Result<u64> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(io_context(path))?;
    let offset = file.metadata().map_err(io_context(path))?.len();
    let bytes = record.encode();
    crate::fsutil::append_synced(&mut file, &bytes)?;
    Ok(offset)
}

/// 负载与序号一致性的守卫：拒绝非连续序号，避免夹具自己写错日志还判"通过"。
///
/// # Errors
///
/// 序号不严格连续，或负载超过 [`MAX_PAYLOAD`]。
pub fn validate_sequence(records: &[Record]) -> std::result::Result<(), String> {
    for (index, record) in records.iter().enumerate() {
        let expected = index as u64 + 1;
        if record.seq != expected {
            return Err(format!(
                "夹具写入了错误序号：第 {index} 条 seq={} 期望 {expected}",
                record.seq
            ));
        }
        if record.payload.len() > MAX_PAYLOAD {
            return Err(format!("第 {index} 条负载过大: {}", record.payload.len()));
        }
    }
    Ok(())
}

/// 把错误信息变成 spike 错误（夹具断言用）。
pub fn sequence_error(message: String) -> SpikeError {
    SpikeError::Core(message)
}
