//! 控制记录的追加日志与恢复。
//!
//! 帧格式是本 spike 的简化实现：`u32 payload_len | payload | u64 fnv1a`。
//! 正式持久化格式待持久化 ADR，见 `docs/MIXED_SOURCE_EDITING.md` 第 8 节。
//!
//! 恢复语义（对应 `docs/HISTORY_COLLABORATION.md` 第 7 节）：
//! - 尾部半写记录可截断，并报告被丢弃的字节数；
//! - 中部损坏停止自动恢复并返回错误，不允许退化成"默认打开某种语言"的可写状态。

use crate::error::RecoveryError;
use crate::model::Dialect;

/// 控制平面的可持久记录。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LogRecord {
    /// 初始控制记录：范围、语言、epoch。
    Bootstrap {
        /// 共享范围。
        scope: String,
        /// 初始活动语言。
        dialect: Dialect,
        /// 初始 epoch。
        epoch: u64,
        /// 时刻。
        tick: u64,
    },
    /// 成员加入。
    MemberAdded {
        /// 成员。
        actor: String,
    },
    /// 发放许可。
    PermitGranted {
        /// 许可 ID。
        id: u64,
        /// 持证人。
        actor: String,
        /// 语言。
        dialect: Dialect,
        /// epoch。
        epoch: u64,
        /// 发放时刻。
        tick: u64,
        /// 失效时刻。
        expires_at: u64,
    },
    /// 撤销许可。
    PermitRevoked {
        /// 许可 ID。
        id: u64,
        /// 撤销时刻。
        tick: u64,
    },
    /// 切换开始（进入屏障）。
    SwitchStarted {
        /// 原语言。
        from: Dialect,
        /// 目标语言。
        to: Dialect,
        /// 目标 epoch。
        target_epoch: u64,
        /// 时刻。
        tick: u64,
        /// 需要确认 drain 的成员。
        expected: Vec<String>,
    },
    /// 成员在屏障中被隔离。
    Quarantined {
        /// 成员。
        actor: String,
        /// 时刻。
        tick: u64,
    },
    /// 屏障期间旧语言在途写入入队（待冲刷）。
    DrainQueued {
        /// 提交者。
        actor: String,
        /// 序号。
        seq: u64,
        /// 语言。
        dialect: Dialect,
        /// epoch。
        epoch: u64,
        /// 时刻。
        tick: u64,
        /// 写集。
        ops: Vec<(String, String)>,
    },
    /// 屏障队列已冲刷。
    DrainFlushed {
        /// 时刻。
        tick: u64,
    },
    /// 切换原子提交：活动语言与 epoch 更新。
    SwitchCommitted {
        /// 原语言。
        from: Dialect,
        /// 新语言。
        to: Dialect,
        /// 新 epoch。
        epoch: u64,
        /// 时刻。
        tick: u64,
    },
    /// 接受的写入。
    WriteAccepted {
        /// 提交者。
        actor: String,
        /// 序号。
        seq: u64,
        /// 语言。
        dialect: Dialect,
        /// epoch。
        epoch: u64,
        /// 时刻。
        tick: u64,
        /// 写集。
        ops: Vec<(String, String)>,
    },
}

/// 追加日志。
#[derive(Clone, Debug, Default)]
pub(crate) struct Wal {
    bytes: Vec<u8>,
}

impl Wal {
    /// 空日志。
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// 追加一条记录（帧化 + 校验和）。
    pub(crate) fn append(&mut self, record: &LogRecord) {
        let payload = encode(record);
        let checksum = fnv1a64(&payload);
        self.bytes
            .extend_from_slice(&(payload.len() as u32).to_le_bytes());
        self.bytes.extend_from_slice(&payload);
        self.bytes.extend_from_slice(&checksum.to_le_bytes());
    }

    /// 当前字节内容。
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// 恢复结果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Recovered {
    /// 成功解码的记录。
    pub records: Vec<LogRecord>,
    /// 被截断的尾部字节数；`None` 表示无尾部残片。
    pub truncated_tail: Option<usize>,
}

/// 扫描字节流并解码记录。
///
/// # Errors
/// - `NoControlRecord`：没有任何完整记录（不允许默认回退到可写状态）。
/// - `MidLogCorruption`：校验和不匹配或记录无法解码。
pub(crate) fn recover(bytes: &[u8]) -> Result<Recovered, RecoveryError> {
    let mut pos = 0usize;
    let mut records = Vec::new();
    let mut truncated_tail = None;
    while pos < bytes.len() {
        let Some(frame) = frame_at(bytes, pos) else {
            truncated_tail = Some(bytes.len() - pos);
            break;
        };
        if fnv1a64(frame.payload) != frame.checksum {
            return Err(RecoveryError::MidLogCorruption {
                offset: pos,
                detail: "checksum mismatch".to_owned(),
            });
        }
        let record = decode(frame.payload).map_err(|detail| RecoveryError::MidLogCorruption {
            offset: pos,
            detail,
        })?;
        records.push(record);
        pos = frame.end;
    }
    if records.is_empty() {
        return Err(RecoveryError::NoControlRecord);
    }
    Ok(Recovered {
        records,
        truncated_tail,
    })
}

/// FNV-1a 64 位校验和（无需额外依赖，确定性）。
pub(crate) fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// 一帧的切片视图。
struct Frame<'a> {
    payload: &'a [u8],
    checksum: u64,
    end: usize,
}

/// 尝试在 `pos` 处解析一帧；长度不足时返回 `None`（尾部半写）。
fn frame_at(bytes: &[u8], pos: usize) -> Option<Frame<'_>> {
    let header_end = pos.checked_add(4)?;
    if header_end > bytes.len() {
        return None;
    }
    let len = u32::from_le_bytes(bytes[pos..header_end].try_into().ok()?) as usize;
    let payload_end = header_end.checked_add(len)?;
    let end = payload_end.checked_add(8)?;
    if end > bytes.len() {
        return None;
    }
    let checksum = u64::from_le_bytes(bytes[payload_end..end].try_into().ok()?);
    Some(Frame {
        payload: &bytes[header_end..payload_end],
        checksum,
        end,
    })
}

/// 字节编码。
fn encode(record: &LogRecord) -> Vec<u8> {
    let mut out = Vec::new();
    match record {
        LogRecord::Bootstrap {
            scope,
            dialect,
            epoch,
            tick,
        } => {
            out.push(1);
            put_str(&mut out, scope);
            out.push(dialect_byte(*dialect));
            put_u64(&mut out, *epoch);
            put_u64(&mut out, *tick);
        }
        LogRecord::MemberAdded { actor } => {
            out.push(2);
            put_str(&mut out, actor);
        }
        LogRecord::PermitGranted {
            id,
            actor,
            dialect,
            epoch,
            tick,
            expires_at,
        } => {
            out.push(3);
            put_u64(&mut out, *id);
            put_str(&mut out, actor);
            out.push(dialect_byte(*dialect));
            put_u64(&mut out, *epoch);
            put_u64(&mut out, *tick);
            put_u64(&mut out, *expires_at);
        }
        LogRecord::PermitRevoked { id, tick } => {
            out.push(4);
            put_u64(&mut out, *id);
            put_u64(&mut out, *tick);
        }
        LogRecord::SwitchStarted {
            from,
            to,
            target_epoch,
            tick,
            expected,
        } => {
            out.push(5);
            out.push(dialect_byte(*from));
            out.push(dialect_byte(*to));
            put_u64(&mut out, *target_epoch);
            put_u64(&mut out, *tick);
            put_str_vec(&mut out, expected);
        }
        LogRecord::Quarantined { actor, tick } => {
            out.push(6);
            put_str(&mut out, actor);
            put_u64(&mut out, *tick);
        }
        LogRecord::DrainQueued {
            actor,
            seq,
            dialect,
            epoch,
            tick,
            ops,
        } => {
            out.push(7);
            put_str(&mut out, actor);
            put_u64(&mut out, *seq);
            out.push(dialect_byte(*dialect));
            put_u64(&mut out, *epoch);
            put_u64(&mut out, *tick);
            put_ops(&mut out, ops);
        }
        LogRecord::DrainFlushed { tick } => {
            out.push(8);
            put_u64(&mut out, *tick);
        }
        LogRecord::SwitchCommitted {
            from,
            to,
            epoch,
            tick,
        } => {
            out.push(9);
            out.push(dialect_byte(*from));
            out.push(dialect_byte(*to));
            put_u64(&mut out, *epoch);
            put_u64(&mut out, *tick);
        }
        LogRecord::WriteAccepted {
            actor,
            seq,
            dialect,
            epoch,
            tick,
            ops,
        } => {
            out.push(10);
            put_str(&mut out, actor);
            put_u64(&mut out, *seq);
            out.push(dialect_byte(*dialect));
            put_u64(&mut out, *epoch);
            put_u64(&mut out, *tick);
            put_ops(&mut out, ops);
        }
    }
    out
}

/// 字节解码。
fn decode(payload: &[u8]) -> Result<LogRecord, String> {
    let mut cursor = Cursor::new(payload);
    let tag = cursor.read_u8()?;
    let record = match tag {
        1 => LogRecord::Bootstrap {
            scope: cursor.read_str()?,
            dialect: cursor.read_dialect()?,
            epoch: cursor.read_u64()?,
            tick: cursor.read_u64()?,
        },
        2 => LogRecord::MemberAdded {
            actor: cursor.read_str()?,
        },
        3 => LogRecord::PermitGranted {
            id: cursor.read_u64()?,
            actor: cursor.read_str()?,
            dialect: cursor.read_dialect()?,
            epoch: cursor.read_u64()?,
            tick: cursor.read_u64()?,
            expires_at: cursor.read_u64()?,
        },
        4 => LogRecord::PermitRevoked {
            id: cursor.read_u64()?,
            tick: cursor.read_u64()?,
        },
        5 => LogRecord::SwitchStarted {
            from: cursor.read_dialect()?,
            to: cursor.read_dialect()?,
            target_epoch: cursor.read_u64()?,
            tick: cursor.read_u64()?,
            expected: cursor.read_str_vec()?,
        },
        6 => LogRecord::Quarantined {
            actor: cursor.read_str()?,
            tick: cursor.read_u64()?,
        },
        7 => LogRecord::DrainQueued {
            actor: cursor.read_str()?,
            seq: cursor.read_u64()?,
            dialect: cursor.read_dialect()?,
            epoch: cursor.read_u64()?,
            tick: cursor.read_u64()?,
            ops: cursor.read_ops()?,
        },
        8 => LogRecord::DrainFlushed {
            tick: cursor.read_u64()?,
        },
        9 => LogRecord::SwitchCommitted {
            from: cursor.read_dialect()?,
            to: cursor.read_dialect()?,
            epoch: cursor.read_u64()?,
            tick: cursor.read_u64()?,
        },
        10 => LogRecord::WriteAccepted {
            actor: cursor.read_str()?,
            seq: cursor.read_u64()?,
            dialect: cursor.read_dialect()?,
            epoch: cursor.read_u64()?,
            tick: cursor.read_u64()?,
            ops: cursor.read_ops()?,
        },
        other => return Err(format!("unknown record tag {other}")),
    };
    cursor.finish()?;
    Ok(record)
}

/// 小端写入工具。
fn put_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// 长度前缀字符串写入。
fn put_str(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u32).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

/// 字符串数组写入。
fn put_str_vec(out: &mut Vec<u8>, values: &[String]) {
    out.extend_from_slice(&(values.len() as u32).to_le_bytes());
    for value in values {
        put_str(out, value);
    }
}

/// 写集写入。
fn put_ops(out: &mut Vec<u8>, ops: &[(String, String)]) {
    out.extend_from_slice(&(ops.len() as u32).to_le_bytes());
    for (path, text) in ops {
        put_str(out, path);
        put_str(out, text);
    }
}

/// 方言字节映射。
fn dialect_byte(dialect: Dialect) -> u8 {
    match dialect {
        Dialect::Latex => 0,
        Dialect::Typst => 1,
    }
}

/// 解码游标。
struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    /// 新建游标。
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    /// 读取 1 字节。
    fn read_u8(&mut self) -> Result<u8, String> {
        let value = *self
            .bytes
            .get(self.pos)
            .ok_or_else(|| "unexpected end (u8)".to_owned())?;
        self.pos += 1;
        Ok(value)
    }

    /// 读取 u32。
    fn read_u32(&mut self) -> Result<u32, String> {
        let end = self.pos + 4;
        let slice = self
            .bytes
            .get(self.pos..end)
            .ok_or_else(|| "unexpected end (u32)".to_owned())?;
        self.pos = end;
        slice
            .try_into()
            .map(u32::from_le_bytes)
            .map_err(|_| "bad u32".to_owned())
    }

    /// 读取 u64。
    fn read_u64(&mut self) -> Result<u64, String> {
        let end = self.pos + 8;
        let slice = self
            .bytes
            .get(self.pos..end)
            .ok_or_else(|| "unexpected end (u64)".to_owned())?;
        self.pos = end;
        slice
            .try_into()
            .map(u64::from_le_bytes)
            .map_err(|_| "bad u64".to_owned())
    }

    /// 读取长度前缀字符串。
    fn read_str(&mut self) -> Result<String, String> {
        let len = self.read_u32()? as usize;
        let end = self.pos + len;
        let slice = self
            .bytes
            .get(self.pos..end)
            .ok_or_else(|| "unexpected end (str)".to_owned())?;
        self.pos = end;
        String::from_utf8(slice.to_vec()).map_err(|_| "invalid utf8".to_owned())
    }

    /// 读取方言。
    fn read_dialect(&mut self) -> Result<Dialect, String> {
        match self.read_u8()? {
            0 => Ok(Dialect::Latex),
            1 => Ok(Dialect::Typst),
            other => Err(format!("unknown dialect byte {other}")),
        }
    }

    /// 读取字符串数组。
    fn read_str_vec(&mut self) -> Result<Vec<String>, String> {
        let count = self.read_u32()? as usize;
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            values.push(self.read_str()?);
        }
        Ok(values)
    }

    /// 读取写集。
    fn read_ops(&mut self) -> Result<Vec<(String, String)>, String> {
        let count = self.read_u32()? as usize;
        let mut ops = Vec::with_capacity(count);
        for _ in 0..count {
            ops.push((self.read_str()?, self.read_str()?));
        }
        Ok(ops)
    }

    /// 确认没有多余字节。
    fn finish(self) -> Result<(), String> {
        if self.pos == self.bytes.len() {
            Ok(())
        } else {
            Err(format!(
                "trailing bytes: {} of {}",
                self.pos,
                self.bytes.len()
            ))
        }
    }
}
