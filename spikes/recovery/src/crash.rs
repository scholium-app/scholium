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

/// 子进程模式。由 `--mode` 解析而来；未知取值在解析处直接报错。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// 正常写完 `count` 条记录后正常退出（退出码 0）。
    Normal,
    /// 写完 `crash_at - 1` 条完整记录，再分块写第 `crash_at` 条并中途 SIGKILL。
    Crash,
    /// 写完完整记录后翻转尾部一个字节，再 SIGKILL。
    CorruptTail,
}

impl Mode {
    /// 解析 `--mode` 取值。
    fn parse(raw: &str) -> Result<Self> {
        match raw {
            "normal" => Ok(Self::Normal),
            "crash" => Ok(Self::Crash),
            "corrupt-tail" => Ok(Self::CorruptTail),
            other => Err(SpikeError::Core(format!("未知 writer 模式: {other}"))),
        }
    }
}

/// 子进程入口：解析参数、执行模式。
///
/// # Errors
///
/// 参数缺失/非法，或写入失败。
pub fn run(args: &[String]) -> Result<()> {
    let path = PathBuf::from(require(args, "--wal")?);
    let mode = Mode::parse(&arg_value(args, "--mode").unwrap_or_else(|| "normal".to_string()))?;
    match mode {
        Mode::Normal => {
            let count = parse_u64(args, "--count")?;
            write_normal(&path, count)
        }
        Mode::Crash => {
            let crash_at = parse_u64(args, "--crash-at")?;
            let chunk = parse_usize(args, "--chunk")?;
            let kill_after = parse_usize(args, "--kill-after-chunks")?;
            if !(2..=64).contains(&crash_at) {
                return Err(SpikeError::Core(format!(
                    "crash-at 必须落在 [2, 64]，收到 {crash_at}"
                )));
            }
            if !(1..=4096).contains(&chunk) {
                return Err(SpikeError::Core(format!(
                    "chunk 必须落在 [1, 4096]，收到 {chunk}"
                )));
            }
            if kill_after == 0 || kill_after > 64 {
                return Err(SpikeError::Core(format!(
                    "kill-after-chunks 必须落在 [1, 64]，收到 {kill_after}"
                )));
            }
            crash_mid_record(&path, crash_at, chunk, kill_after)
        }
        Mode::CorruptTail => {
            let count = parse_u64(args, "--count")?;
            corrupt_tail(&path, count)
        }
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

/// 分块写一条记录，并在写完 `kill_after_chunks` 块之后自杀。
///
/// 为什么必须显式给"杀在第几块"而不是"写完循环就杀"：`bytes.chunks(chunk)` 只要
/// `chunk` 不整除记录长度，循环就会把整条记录写完，于是这个"崩溃夹具"会悄悄退化成
/// **正常关闭**，把撕裂用例测成空。`kill_after_chunks` 让"停在记录中间"成为
/// 夹具的前置条件，并在这里直接断言。
///
/// # Errors
///
/// 写入失败，或参数根本制造不出残缺（此时明确报错，不返回 Ok）。
pub fn crash_mid_record(
    path: &Path,
    seq: u64,
    chunk: usize,
    kill_after_chunks: usize,
) -> Result<()> {
    write_normal(path, seq - 1)?;

    // 只用 `.append(true)`（它已隐含可写；clippy 的
    // `suspicious_open_options` 会指出多写的 `.write(true)` 是冗余的）。
    // 这里曾经漏掉可写性判断、写死 `.append(true)` 却以为不可写，于是改用
    // 「写完循环再自杀」的写法，结果分块整除时记录被完整写下，
    // "尾部撕裂"用例静默退化成"正常关闭"。
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(io_context(path))?;
    let bytes = fixture_record(seq).encode();
    let chunks: Vec<&[u8]> = bytes.chunks(chunk).collect();
    if kill_after_chunks >= chunks.len() {
        return Err(SpikeError::Core(format!(
            "kill-after-chunks={kill_after_chunks} 覆盖整条记录（{} B / {chunk} B 分块 = {} 块），\
             制造不出残缺尾部",
            bytes.len(),
            chunks.len()
        )));
    }

    for piece in chunks.iter().take(kill_after_chunks) {
        file.write_all(piece).map_err(io_context(path))?;
        file.sync_all().map_err(io_context(path))?;
    }
    kill_self()
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
/// 实际上不会走到错误返回：`SIGKILL` 不能被捕获或阻塞，进程在 `raise` 内即终止。
/// 两个 `Err` 分支是为了让类型是 `Result`，并保证"没死成"不会被当成成功。
pub fn kill_self() -> Result<()> {
    // SAFETY: `raise` 的参数是一个信号号常量，没有指针、没有缓冲区、没有别名问题，
    // 也不与 Rust 的内存模型交互；`SIGKILL` 在该调用内终止进程，函数不会正常返回。
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
