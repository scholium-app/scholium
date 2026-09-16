//! 崩溃夹具：作为**子进程**被主进程启动，写到一半用 SIGKILL 自杀。
//!
//! 为什么必须是子进程：本进程无法在被 `SIGKILL` 之后继续跑断言。让子进程只负责
//! "写字节并死掉"，主进程负责"重新打开日志并判断恢复到哪一条"——这样崩溃点是真实的
//! 进程死亡，不是在同一进程里模拟出来的错误分支。
//!
//! 为什么用手工 `write_all` 分块写而不调用 [`crate::wal::append`]：`append` 一条记录
//! 只有一次 `write_all`，用户态无法在中间停住；分块写并在块之间 `fsync` 才能构造出
//! "头完整、负载缺一截"的真实撕裂。编码 bytes 仍由 `Record::encode` 生成，
//! 所以撕的是产品格式，不是夹具自造的假格式。

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::cli::arg_value;
use crate::error::{Result, SpikeError, io_context};
use crate::wal::Record;

/// 子进程模式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriterMode {
    /// 正常写完 `count` 条记录后正常退出（进程退出码 0）。
    Normal {
        /// 记录条数。
        count: u64,
    },
    /// 写完 `crash_at - 1` 条完整记录，然后分块写第 `crash_at` 条并在
    /// 第 `chunk` 块之后 `SIGKILL` 自己。
    Crash {
        /// 目标崩溃记录序号（从 1 开始）。
        crash_at: u64,
        /// 分块大小。
        chunk: usize,
    },
    /// 写完 `count` 条完整记录后，翻转尾部记录里的一个字节再自杀。
    ///
    /// 这一分支**不是**真实崩溃能产生的（真实撕裂不会产生自洽的头加坏负载的后来记录），
    /// 它对应的是磁盘位翻转或外部改写，用来验证 CRC 确实拦得住。
    CorruptTail {
        /// 完整记录条数。
        count: u64,
    },
}

/// 子进程入口：解析参数、执行模式。
///
/// # Errors
///
/// 参数缺失/非法，或写入失败。
pub fn run(args: &[String]) -> Result<()> {
    let path = PathBuf::from(require(args, "--wal")?);
    match arg_value(args, "--mode").unwrap_or_else(|| "normal".to_string()).as_str() {
        "normal" => {
            let count = parse_u64(args, "--count")?;
            write_normal(&path, count)
        }
        "crash" => {
            let crash_at = parse_u64(args, "--crash-at")?;
            let chunk = parse_usize(args, "--chunk")?;
            if crash_at < 2 || crash_at > 64 {
                return Err(SpikeError::Core(format!(
                    "crash-at 必须落在 [2, 64]，收到 {crash_at}"
                )));
            }
            if chunk == 0 || chunk > 4096 {
                return Err(SpikeError::Core(format!("chunk 必须落在 [1, 4096]，收到 {chunk}")));
            }
            crash_mid_record(&path, crash_at, chunk)
        }
        "corrupt-tail" => {
            let count = parse_u64(args, "--count")?;
            corrupt_tail(&path, count)
        }
        other => Err(SpikeError::Core(format!("未知 writer 模式: {other}"))),
    }
}

/// 正常写完并 `fsync`，进程正常退出。
///
/// # Errors
///
/// 目录不可写或 `fsync` 失败。
pub fn write_normal(path: &Path, count: u64) -> Result<()> {
    for seq in 1..=count {
        crate::wal::append(path, &fixture_record(seq))?;
    }
    Ok(())
}

/// 分块写一条记录，并在第 `kill_after_chunks` 块之后自杀。
///
/// # Errors
///
/// 写入失败。函数正常情况下不返回：进程会先被 `SIGKILL` 终止。
pub fn crash_mid_record(path: &Path, seq: u64, chunk: usize) -> Result<()> {
    write_normal(path, seq - 1)?;

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(io_context(path))?;
    let bytes = fixture_record(seq).encode();
    for piece in bytes.chunks(chunk) {
        file.write_all(piece).map_err(io_context(path))?;
        file.sync_all().map_err(io_context(path))?;
    }
    // 写完却到达这里说明 chunk 整除记录长度，没有制造出尾部残缺；
    // 夹具必须明确失败，而不是悄悄变成"正常关闭"用例。
    Err(SpikeError::Core(format!(
        "分块 {chunk} 未能在第 {seq} 条记录中间截断（记录 {} B）",
        bytes.len()
    )))
}

/// 写完完整记录后破坏尾部记录的一个负载字节，再自杀。
///
/// # Errors
///
/// 读回或改写失败。
pub fn corrupt_tail(path: &Path, count: u64) -> Result<()> {
    write_normal(path, count)?;
    let mut bytes = crate::fsutil::read_file(path)?;
    // 翻转最后一个字节：它在最后一条记录的负载里，CRC 必然失配。
    if let Some(last) = bytes.last_mut() {
        *last ^= 0xff;
    }
    std::fs::write(path, &bytes).map_err(io_context(path))?;
    kill_self()
}

/// `SIGKILL` 自己。`libc::raise` 之后进程不再有机会执行任何 Rust 代码，
/// 这正是"崩溃"与"返回错误"的区别。
///
/// # Errors
///
/// 理论上不会失败；失败时返回 IO 错误而不是假装成功。
pub fn kill_self() -> Result<()> {
    // SAFETY: raise(SIGKILL) 只接受一个有效信号号，没有指针参数，
    // 不会与 Rust 的内存模型交互；此调用之后进程立即终止，不会返回。
    let rc = unsafe { libc::raise(libc::SIGKILL) };
    if rc != 0 {
        return Err(SpikeError::Core(format!("raise(SIGKILL) 返回 {rc}")));
    }
    Err(SpikeError::Core("SIGKILL 被忽略".to_string()))
}

/// 夹具记录：序号进负载，长度随序号变化，避免"所有记录等长"掩盖偏移计算错误。
pub fn fixture_record(seq: u64) -> Record {
    let filler = "x".repeat((seq as usize * 7) % 53);
    Record::new(seq, 1, format!("rec-{seq:04}-{filler}"))
}

fn require(args: &[String], key: &str) -> Result<String> {
    arg_value(args, key).ok_or_else(|| SpikeError::Core(format!("缺少参数 {key}")))
}

fn parse_u64(args: &[String], key: &str) -> Result<u64> {
    let raw = require(args, key)?;
    raw.parse::<u64>()
        .map_err(|source| SpikeError::Core(format!("{key}={raw} 不是整数: {source}")))
}

fn parse_usize(args: &[String], key: &str) -> Result<usize> {
    let raw = require(args, key)?;
    raw.parse::<usize>()
        .map_err(|source| SpikeError::Core(format!("{key}={raw} 不是整数: {source}")))
}
