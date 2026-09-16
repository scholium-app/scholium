//! 判据 2：WAL 与未提交草稿恢复。
//!
//! 三种情况各自独立判定：正常关闭、尾部截断、尾部字节损坏。
//! 崩溃一律由**子进程 + SIGKILL** 制造：主进程只负责重新打开日志并判断恢复到哪一条，
//! 因此崩溃点是真实进程死亡，不是同一进程里的模拟错误分支。
//!
//! 每条记录都带长度与 CRC-32；恢复语义是"从文件头顺序扫描，遇到第一条不完整或校验失败的
//! 记录就停止，其后的字节一律丢弃"（理由见 `wal::recovery` 模块注释）。

use std::path::{Path, PathBuf};

use crate::check::Checks;
use crate::crash;
use crate::error::{Result, SpikeError};
use crate::fsutil::Scratch;
use crate::process::{self, ProcessOutput};
use crate::wal::recovery::{self, TailDefect};
use crate::wal::{self, Record};

/// 夹具记录条数。
const RECORDS: u64 = 8;
/// 分块大小：设计成**不能整除**探针记录长度，构造出真的尾部残缺。
/// 探针记录长度见用例内断言，不靠注释声称。
const CHUNK: usize = 64;
/// 刻意选一条负载长度**不是** `CHUNK` 整数倍的记录作为崩溃点。
const CRASH_SEQ: u64 = 5;

/// 崩溃子进程的启动结果。
#[derive(Debug)]
struct Spawned {
    log: PathBuf,
    output: ProcessOutput,
}

impl Spawned {
    /// 子进程是否被 `SIGKILL` 杀死。
    fn killed(&self) -> bool {
        self.output.killed_by_sigkill()
    }
}

/// 跑判据 2 的全部用例。
///
/// # Errors
///
/// 夹具编码、子进程或文件系统操作失败。
pub fn run(checks: &mut Checks, workspace: &Path) -> Result<()> {
    println!("\n## 判据 2：WAL 与未提交草稿恢复");
    let scratch = Scratch::create(workspace, "c2")?;

    check_encoding(checks)?;
    case_normal_close(checks, scratch.root())?;
    case_tail_truncated(checks, scratch.root())?;
    case_tail_corrupted(checks, scratch.root())?;
    Ok(())
}

/// 用例 0：编码自检——恢复的比对基准必须来自同一套编码，否则后面的断言没意义。
fn check_encoding(checks: &mut Checks) -> Result<()> {
    checks.case("wal.encoding");
    let record = Record::new(7, 3, b"payload-7".to_vec());
    let bytes = record.encode();
    checks.expect(
        bytes.len() == record.encoded_len(),
        "编码长度与声明一致",
        &format!("{} / {}", bytes.len(), record.encoded_len()),
    );
    checks.expect(
        bytes.starts_with(&wal::MAGIC),
        "魔数在头部",
        &format!("{:02x?}", &bytes[..4]),
    );
    let total_len = u32::from_le_bytes(bytes[4..8].try_into().unwrap_or([0; 4]));
    checks.expect(
        total_len as usize == bytes.len(),
        "长度字段覆盖整条记录",
        &format!("头内 {total_len} / 实际 {}", bytes.len()),
    );
    // 翻转负载任意一个字节都必须被 CRC 拦住。
    let mut corrupted = bytes.clone();
    let last = corrupted.len() - 1;
    corrupted[last] ^= 0xff;
    let scan = recovery::scan(&corrupted);
    checks.expect(
        matches!(scan.defect, Some(TailDefect::ChecksumMismatch { .. })),
        "单字节翻转被 CRC 拦住",
        &format!("defect={:?}", scan.defect),
    );

    // 探针记录长度必须不是分块大小整数倍，否则崩溃用例会退化成"完整记录"。
    let probe = crash::fixture_record(CRASH_SEQ);
    checks.expect(
        probe.encoded_len() % CHUNK != 0,
        "崩溃探针记录长度不是分块整数倍",
        &format!("记录 {} B，分块 {CHUNK} B", probe.encoded_len()),
    );
    checks.expect(
        probe.encoded_len() < CHUNK * 2,
        "崩溃探针在第二块内就结束（只会留下一个分块）",
        &format!(
            "记录 {} B，第一块 {CHUNK} B，剩余 {} B",
            probe.encoded_len(),
            probe.encoded_len() - CHUNK
        ),
    );
    Ok(())
}

/// 用例 1：正常关闭——全部记录可恢复，无尾部缺陷。
fn case_normal_close(checks: &mut Checks, root: &Path) -> Result<()> {
    checks.case("wal.normal-close");
    let spawned = spawn_writer(
        root,
        "normal",
        &["--mode", "normal", "--count", &RECORDS.to_string()],
    )?;
    checks.expect(
        spawned.output.succeeded(),
        "子进程正常退出（退出码 0）",
        &format!("code={:?} signal={:?}", spawned.output.code, spawned.output.signal),
    );

    let recovered = recovery::recover(&spawned.log)?;
    let expected: Vec<Record> = (1..=RECORDS).map(crash::fixture_record).collect();
    checks.note(
        "恢复正常关闭日志",
        &format!(
            "文件 {} B，恢复 {} 条，good_bytes={}，defect={:?}",
            recovered.file_bytes,
            recovered.records.len(),
            recovered.good_bytes,
            recovered.defect
        ),
    );
    checks.expect(
        recovered.records.len() == expected.len(),
        "记录条数一致",
        &format!("{} / {}", recovered.records.len(), expected.len()),
    );
    checks.expect(
        recovered.records == expected,
        "逐条字节与写入内容一致",
        &format!("首条 seq={} 末条 seq={}", recovered.records[0].seq, recovered.records[RECORDS as usize - 1].seq),
    );
    checks.expect(
        recovered.defect.is_none(),
        "没有尾部缺陷",
        &format!("{:?}", recovered.defect),
    );
    checks.expect(
        !recovered.has_tail(),
        "没有字节被丢弃",
        &format!("discarded={}", recovered.discarded_bytes),
    );
    checks.expect(
        recovered.good_bytes == recovered.file_bytes,
        "good_bytes 覆盖整个文件",
        &format!("{} / {}", recovered.good_bytes, recovered.file_bytes),
    );
    Ok(())
}

/// 用例 2：尾部截断——写到一半被 SIGKILL，只恢复到最后一个完整记录。
fn case_tail_truncated(checks: &mut Checks, root: &Path) -> Result<()> {
    checks.case("wal.tail-truncated");

    // 2a：撕裂落在负载中间，而且第一块写在头上、第二块写不完。
    let spawned = spawn_writer(
        root,
        "truncated-payload",
        &[
            "--mode",
            "crash",
            "--crash-at",
            &CRASH_SEQ.to_string(),
            "--chunk",
            &CHUNK.to_string(),
        ],
    )?;
    checks.expect(
        spawned.killed(),
        "子进程被 SIGKILL 杀死（不是返回错误）",
        &format!("code={:?} signal={:?}", spawned.output.code, spawned.output.signal),
    );
    let raw = crate::fsutil::read_file(&spawned.log)?;
    let complete: usize = (1..CRASH_SEQ)
        .map(|seq| crash::fixture_record(seq).encoded_len())
        .sum();
    checks.note(
        "崩溃现场",
        &format!(
            "日志 {} B：完整 {complete} B + 尾部残缺 {} B",
            raw.len(),
            raw.len() - complete
        ),
    );
    checks.expect(
        raw.len() > complete,
        "尾部确实留下了残缺字节（不是空文件也不是恰好完整）",
        &format!("{} B > {complete} B", raw.len()),
    );

    let recovered = recovery::recover(&spawned.log)?;
    checks.expect(
        recovered.records.len() as u64 == CRASH_SEQ - 1,
        "恢复到崩溃前的最后一条完整记录",
        &format!("{} / {}", recovered.records.len(), CRASH_SEQ - 1),
    );
    checks.expect(
        matches!(recovered.defect, Some(TailDefect::TruncatedPayload { .. })),
        "缺陷分类为负载截断",
        &format!("{:?}", recovered.defect),
    );
    checks.expect(
        recovered.good_bytes == complete,
        "good_bytes 正好落在记录边界",
        &format!("{} / {complete}", recovered.good_bytes),
    );
    checks.expect(
        recovered.discarded_bytes == raw.len() - complete,
        "丢弃字节数等于残缺长度",
        &format!("{} / {}", recovered.discarded_bytes, raw.len() - complete),
    );
    checks.expect(
        recovered.good_bytes
            == (1..CRASH_SEQ)
                .map(|seq| crash::fixture_record(seq).encoded_len())
                .sum::<usize>(),
        "恢复点等于已恢复记录的长度之和（只能落在记录边界上）",
        &format!("good_bytes={} 条数={}", recovered.good_bytes, recovered.records.len()),
    );

    // 落盘修复：截断到 good_bytes 后再恢复必须无缺陷且记录数不变。
    recovery::truncate_to_good(&spawned.log, &recovered)?;
    let after = recovery::recover(&spawned.log)?;
    checks.expect(
        after.defect.is_none() && after.records == recovered.records,
        "截断修复后再次恢复：无缺陷且记录不变",
        &format!(
            "defect={:?} 条数={} 文件={} B",
            after.defect,
            after.records.len(),
            after.file_bytes
        ),
    );
    checks.expect(
        after.file_bytes == complete,
        "修复后文件长度等于完整部分",
        &format!("{} / {complete}", after.file_bytes),
    );

    // 2b：撕裂落在头中间，缺陷必须是"头不完整"而不是被误判成损坏。
    let header_case = spawn_writer(
        root,
        "truncated-header",
        &[
            "--mode",
            "crash",
            "--crash-at",
            &CRASH_SEQ.to_string(),
            "--chunk",
            "512",
        ],
    )?;
    let header_recovered = recovery::recover(&header_case.log)?;
    let header_raw = crate::fsutil::read_file(&header_case.log)?;
    checks.expect(
        header_case.killed(),
        "头撕裂场景中子进程也被 SIGKILL",
        &format!("signal={:?}", header_case.output.signal),
    );
    checks.expect(
        matches!(header_recovered.defect, Some(TailDefect::TruncatedHeader { .. })),
        "缺陷分类为头截断",
        &format!("残缺 {} B，{:?}", header_raw.len() - complete, header_recovered.defect),
    );
    checks.expect(
        header_recovered.records.len() as u64 == CRASH_SEQ - 1,
        "头撕裂时仍恢复到同一完整边界",
        &format!("{} / {}", header_recovered.records.len(), CRASH_SEQ - 1),
    );
    Ok(())
}

/// 用例 3：尾部字节损坏——CRC 必须拦住，恢复停在损坏记录之前。
fn case_tail_corrupted(checks: &mut Checks, root: &Path) -> Result<()> {
    checks.case("wal.tail-corrupted");
    let spawned = spawn_writer(
        root,
        "corrupted-tail",
        &[
            "--mode",
            "corrupt-tail",
            "--count",
            &(RECORDS - 1).to_string(),
        ],
    )?;
    checks.expect(
        spawned.killed(),
        "子进程写完后被 SIGKILL（保留坏负载文件）",
        &format!("signal={:?}", spawned.output.signal),
    );

    let recovered = recovery::recover(&spawned.log)?;
    let expected_good: usize = (1..RECORDS - 1)
        .map(|seq| crash::fixture_record(seq).encoded_len())
        .sum();
    checks.note(
        "损坏现场",
        &format!(
            "日志 {} B；恢复 {} 条；defect={}",
            recovered.file_bytes,
            recovered.records.len(),
            recovered
                .defect
                .as_ref()
                .map(TailDefect::describe)
                .unwrap_or_else(|| "无".to_string())
        ),
    );
    checks.expect(
        matches!(recovered.defect, Some(TailDefect::ChecksumMismatch { .. })),
        "缺陷分类为 CRC 不匹配",
        &format!("{:?}", recovered.defect),
    );
    checks.expect(
        recovered.records.len() as u64 == RECORDS - 2,
        "恢复到损坏记录之前的那一条",
        &format!("{} / {}", recovered.records.len(), RECORDS - 2),
    );
    checks.expect(
        recovered.good_bytes == expected_good,
        "good_bytes 停在损坏记录起点",
        &format!("{} / {expected_good}", recovered.good_bytes),
    );
    checks.expect(
        recovered.discarded_bytes == crash::fixture_record(RECORDS - 1).encoded_len(),
        "被丢弃的正好是那条损坏记录",
        &format!(
            "{} / {}",
            recovered.discarded_bytes,
            crash::fixture_record(RECORDS - 1).encoded_len()
        ),
    );
    checks.expect(
        recovered.records == (1..RECORDS - 1).map(crash::fixture_record).collect::<Vec<_>>(),
        "丢弃损坏记录后，之前的记录逐字节完好",
        &format!("末条 seq={}", recovered.records.last().map(|r| r.seq).unwrap_or(0)),
    );

    recovery::truncate_to_good(&spawned.log, &recovered)?;
    let after = recovery::recover(&spawned.log)?;
    checks.expect(
        after.defect.is_none() && after.records == recovered.records,
        "截断修复后再次恢复：无缺陷且记录不变",
        &format!("defect={:?} 条数={}", after.defect, after.records.len()),
    );
    Ok(())
}

/// 启动 writer 子进程。
fn spawn_writer(root: &Path, label: &str, args: &[&str]) -> Result<Spawned> {
    let dir = root.join(label);
    std::fs::create_dir_all(&dir).map_err(crate::error::io_context(&dir))?;
    let log = dir.join("draft.wal");

    let executable = std::env::current_exe()
        .map_err(crate::error::io_context(std::path::Path::new("<current_exe>")))?;
    let mut full: Vec<String> = vec!["writer".to_string(), "--wal".to_string(), log.display().to_string()];
    full.extend(args.iter().map(|arg| arg.to_string()));

    let output = process::spawn_inherit(
        executable
            .to_str()
            .ok_or_else(|| SpikeError::Core("可执行文件路径不是 UTF-8".to_string()))?,
        &full,
        &dir,
    )?;
    Ok(Spawned { log, output })
}

/// 直接以进程内方式写日志（用于随机截断用例准备基准文件）。
///
/// # Errors
///
/// 写盘失败或序号不连续。
pub fn write_baseline(path: &Path, count: u64) -> Result<Vec<Record>> {
    let records: Vec<Record> = (1..=count).map(crash::fixture_record).collect();
    wal::validate_sequence(&records).map_err(wal::sequence_error)?;
    for record in &records {
        wal::append(path, record)?;
    }
    Ok(records)
}
