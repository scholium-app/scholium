//! 手写二进制编解码与哈希。
//!
//! 不引入 `serde`：快照与规范状态只需要 "写进去、读回来" 这一件事，手写格式让字节布局
//! 完全可见（报告里要比较字节相等），也避免为了一个 spike 引入派生宏依赖链。
//!
//! 所有多字节整数都是小端；长度前缀是 `u32`。位置标识的每一层数字都小于 `BASE = 2^16`，
//! 因此按 `u16` 写。

use crate::error::CrdtError;
use crate::ids::{ActorId, Id, Lamport};

/// 追加式字节写入器。
#[derive(Debug, Default)]
pub(crate) struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    /// 空写入器。
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// 写一个字节。
    pub(crate) fn u8(&mut self, value: u8) {
        self.buf.push(value);
    }

    /// 写布尔值（1 字节）。
    pub(crate) fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    /// 写 `u16`。
    pub(crate) fn u16(&mut self, value: u16) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    /// 写 `u32`。
    pub(crate) fn u32(&mut self, value: u32) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    /// 写 `u64`。
    pub(crate) fn u64(&mut self, value: u64) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    /// 写字符码点。
    pub(crate) fn char(&mut self, value: char) {
        self.u32(value as u32);
    }

    /// 写带长度前缀的字节串。
    pub(crate) fn bytes(&mut self, value: &[u8]) {
        self.u32(value.len() as u32);
        self.buf.extend_from_slice(value);
    }

    /// 写字符串。
    pub(crate) fn str(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }

    /// 写可选字符串（属性值）。
    pub(crate) fn opt_str(&mut self, value: Option<&str>) {
        match value {
            None => self.u8(0),
            Some(text) => {
                self.u8(1);
                self.str(text);
            }
        }
    }

    /// 写全局标识。
    pub(crate) fn id(&mut self, value: Id) {
        self.u32(value.actor.value());
        self.u32(value.seq);
    }

    /// 写逻辑时间戳。
    pub(crate) fn lamport(&mut self, value: Lamport) {
        self.u64(value.counter);
        self.u32(value.actor.value());
    }

    /// 写位置标识。
    pub(crate) fn position(&mut self, value: &[u32]) {
        self.u32(value.len() as u32);
        for digit in value {
            self.u16(*digit as u16);
        }
    }

    /// 已完成字节。
    pub(crate) fn finish(self) -> Vec<u8> {
        self.buf
    }
}

/// 顺序读取器。
#[derive(Debug)]
pub(crate) struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    /// 从字节切片读取。
    pub(crate) fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    /// 是否已经读到末尾。
    pub(crate) fn is_empty(&self) -> bool {
        self.pos >= self.buf.len()
    }

    /// 读一个字节。
    ///
    /// # Errors
    ///
    /// 输入不足时返回 [`CrdtError::UnexpectedEof`]。
    pub(crate) fn u8(&mut self) -> Result<u8, CrdtError> {
        let byte = *self.buf.get(self.pos).ok_or(CrdtError::UnexpectedEof)?;
        self.pos += 1;
        Ok(byte)
    }

    /// 读布尔值。
    ///
    /// # Errors
    ///
    /// 输入不足或值不是 0/1 时返回错误。
    pub(crate) fn bool(&mut self) -> Result<bool, CrdtError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(CrdtError::InvalidTag {
                what: "bool",
                tag: other,
            }),
        }
    }

    /// 读 `u16`。
    ///
    /// # Errors
    ///
    /// 输入不足时返回 [`CrdtError::UnexpectedEof`]。
    pub(crate) fn u16(&mut self) -> Result<u16, CrdtError> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    /// 读 `u32`。
    ///
    /// # Errors
    ///
    /// 输入不足时返回 [`CrdtError::UnexpectedEof`]。
    pub(crate) fn u32(&mut self) -> Result<u32, CrdtError> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// 读 `u64`。
    ///
    /// # Errors
    ///
    /// 输入不足时返回 [`CrdtError::UnexpectedEof`]。
    pub(crate) fn u64(&mut self) -> Result<u64, CrdtError> {
        let bytes = self.take(8)?;
        let mut raw = [0u8; 8];
        raw.copy_from_slice(bytes);
        Ok(u64::from_le_bytes(raw))
    }

    /// 读字符码点。
    ///
    /// # Errors
    ///
    /// 输入不足或码点非法时返回错误。
    pub(crate) fn char(&mut self) -> Result<char, CrdtError> {
        let raw = self.u32()?;
        char::from_u32(raw).ok_or(CrdtError::InvalidChar(raw))
    }

    /// 读长度前缀字节串。
    ///
    /// # Errors
    ///
    /// 输入不足时返回 [`CrdtError::UnexpectedEof`]。
    pub(crate) fn bytes(&mut self) -> Result<&'a [u8], CrdtError> {
        let len = self.u32()? as usize;
        self.take(len)
    }

    /// 读字符串。
    ///
    /// # Errors
    ///
    /// 输入不足或不是合法 UTF-8 时返回错误。
    pub(crate) fn str(&mut self) -> Result<String, CrdtError> {
        let bytes = self.bytes()?;
        std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| CrdtError::InvalidUtf8)
    }

    /// 读可选字符串。
    ///
    /// # Errors
    ///
    /// 输入不足或不是合法 UTF-8 时返回错误。
    pub(crate) fn opt_str(&mut self) -> Result<Option<String>, CrdtError> {
        match self.u8()? {
            0 => Ok(None),
            1 => Ok(Some(self.str()?)),
            other => Err(CrdtError::InvalidTag {
                what: "option",
                tag: other,
            }),
        }
    }

    /// 读全局标识。
    ///
    /// # Errors
    ///
    /// 输入不足时返回 [`CrdtError::UnexpectedEof`]。
    pub(crate) fn id(&mut self) -> Result<Id, CrdtError> {
        let actor = ActorId(self.u32()?);
        let seq = self.u32()?;
        Ok(Id::new(actor, seq))
    }

    /// 读逻辑时间戳。
    ///
    /// # Errors
    ///
    /// 输入不足时返回 [`CrdtError::UnexpectedEof`]。
    pub(crate) fn lamport(&mut self) -> Result<Lamport, CrdtError> {
        let counter = self.u64()?;
        let actor = ActorId(self.u32()?);
        Ok(Lamport::new(counter, actor))
    }

    /// 读位置标识并校验不变量。
    ///
    /// # Errors
    ///
    /// 输入不足、长度为 0、出现尾零或数字越界时返回错误。
    pub(crate) fn position(&mut self) -> Result<Vec<u32>, CrdtError> {
        let len = self.u32()? as usize;
        let mut out = Vec::with_capacity(len.min(1 << 16));
        for _ in 0..len {
            out.push(u32::from(self.u16()?));
        }
        if crate::position::is_valid(&out) {
            Ok(out)
        } else {
            Err(CrdtError::InvalidPosition)
        }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], CrdtError> {
        let end = self.pos.checked_add(len).ok_or(CrdtError::UnexpectedEof)?;
        let slice = self
            .buf
            .get(self.pos..end)
            .ok_or(CrdtError::UnexpectedEof)?;
        self.pos = end;
        Ok(slice)
    }
}

/// FNV-1a 64 位哈希，用于在报告里给出可读的短标识。
///
/// 判据本身比较**完整字节**，哈希只是便于人眼比对与贴进报告。
pub(crate) fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}
