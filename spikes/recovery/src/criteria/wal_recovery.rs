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
use crate::wal::{self, HEADER_LEN, Record};

/// 夹具记录条数。
const RECORDS: u64 = 8;
/// 负载撕裂用例的分块大小。崩溃点是第 5 条记录（76 B）。
/// 第一块 40 B 写完后立即自杀：头（32 B）完整、负载只写下前 8 B。
const CHUNK: usize = 40;
/// 头撕裂用例的分块大小：20 B < 头长 32 B，所以第一块只覆盖魔数与部分长度字段。
const HEADER_CHUNK: usize = 20;
/// 写完第几块之后自杀。固定为 1：夹具必须停在记录中间，
/// 写满整条记录再杀就等于把撕裂用例测成了正常关闭。
const KILL_AFTER_CHUNKS: usize = 1;
/// 崩溃点记录。取 5 使前面有完整的 4 条记录可以对照。
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
    case_synthetic_rewrite(checks, scratch.root())?;
    Ok(())
}

/// 用例 4（负向能力边界）：**协同改写长度字段与 CRC 不会被拦住**。
///
/// 这一用例期望的结果是"改写成功"，因此它同时是：
/// - 对 [`crate::wal::recovery::TailDefect`] 校验强度的能力上界声明；
/// - 对"CRC 防的是随机损坏/撕裂，不防蓄意改写"这一设计取舍的**可复现证据**。
///
/// 如果哪天实现加上了独立校验（例如对整个前缀做哈希链），这个用例会失败——
/// 那时应当是**改断言**并在报告里更新结论，而不是删掉它。
fn case_synthetic_rewrite(checks: &mut Checks, root: &Path) -> Result<()> {
    checks.case("wal.synthetic-rewrite-boundary");
    let dir = root.join("synthetic");
    std::fs::create_dir_all(&dir).map_err(crate::error::io_context(&dir))?;
    let log = dir.join("draft.wal");
    let baseline = write_baseline(&log, 4)?;

    let mut bytes = crate::fsutil::read_file(&log)?;
    // 第 2 条记录的起点与长度（用同一套编码算出来，不靠猜偏移）。
    let start: usize = baseline[..1].iter().map(Record::encoded_len).sum();
    let record = &baseline[1];
    let payload_start = start + wal::HEADER_LEN;
    let payload_len = record.payload.len();

    // 把负载整体改成同长度的另一个值，然后**把 CRC 一起改对**。
    let replacement = vec![b'Z'; payload_len];
    bytes[payload_start..payload_start + payload_len].copy_from_slice(&replacement);
    let header_crc_range = (start + wal::CRC_INPUT.start)..(start + wal::CRC_INPUT.end);
    let new_crc = wal::record_crc(
        &bytes[header_crc_range],
        &bytes[payload_start..payload_start + payload_len],
    );
    let crc_offset = start + 28;
    bytes[crc_offset..crc_offset + 4].copy_from_slice(&new_crc.to_le_bytes());
    crate::fsutil::write_file(&log, &bytes)?;

    let recovered = crate::wal::recovery::recover(&log)?;
    checks.note(
        "协同改写",
        &format!(
            "第 2 条负载改为 {} 字节的 'Z' 并同步重算 CRC — 恢复 {} 条，defect={:?}",
            payload_len,
            recovered.records.len(),
            recovered.defect
        ),
    );
    checks.expect(
        recovered.records.len() == 4 && recovered.defect.is_none(),
        "协同改写（含 CRC）确实不会被结构性校验拦住 —— 这是已知能力上界",
        &format!(
            "恢复 {} 条，defect={:?}",
            recovered.records.len(),
            recovered.defect
        ),
    );
    checks.expect(
        recovered.records[1].payload == replacement,
        "恢复出的负载是改写后的内容（数据确实被替换，而不是被丢弃）",
        &format!("前 8 字节={:?}", &recovered.records[1].payload[..8.min(payload_len)]),
    );
    checks.expect(
        recovered.records[0] == baseline[0] && recovered.records[2] == baseline[2],
        "同一条记录前后的记录不受影响",
        &format!(
            "第 1 条 seq={} 第 3 条 seq={}",
            recovered.records[0].seq, recovered.records[2].seq
        ),
    );
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

    // 崩溃点记录长度必须比一个分块长，且不是分块大小的整数倍，
    // 否则分块写会在记录边界上停下，"尾部撕裂"用例会退化成"正常关闭"。
    let probe = crash::fixture_record(CRASH_SEQ);
    checks.expect(
        probe.encoded_len() > CHUNK * KILL_AFTER_CHUNKS,
        "崩溃探针比写完的分块长（否则夹具会变成正常关闭）",
        &format!(
            "记录 {} B，写完 {CHUNK}×{KILL_AFTER_CHUNKS} B，残缺 {} B",
            probe.encoded_len(),
            probe.encoded_len() - CHUNK * KILL_AFTER_CHUNKS
        ),
    );
    checks.expect(
        probe.encoded_len() > HEADER_CHUNK * KILL_AFTER_CHUNKS,
        "头撕裂探针在头部中间就停下",
        &format!(
            "头 {HEADER_LEN} B，写完 {HEADER_CHUNK}×{KILL_AFTER_CHUNKS} B 后残缺 {} B",
            probe.encoded_len() - HEADER_CHUNK * KILL_AFTER_CHUNKS
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
        &format!(
            "首条 seq={:?} 末条 seq={:?}",
            recovered.records.first().map(|record| record.seq),
            recovered.records.last().map(|record| record.seq)
        ),
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
///
/// 拆成两个子用例（负载撕裂 / 头撕裂），因为两者的缺陷分类与后续处理不同，
/// 合成一个函数会同时越过项目的"单函数 60 行"上限与 clippy 的 `too_many_lines`。
fn case_tail_truncated(checks: &mut Checks, root: &Path) -> Result<()> {
    checks.case("wal.tail-truncated");
    let complete = payload_tear_case(checks, root)?;
    header_tear_case(checks, root, complete)?;
    Ok(())
}

/// 2a：撕裂落在负载中间（头完整、负载缺一截）。
///
/// 返回"崩溃前应完整落盘"的字节数，供 2b 复用。
fn payload_tear_case(checks: &mut Checks, root: &Path) -> Result<usize> {
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
            "--kill-after-chunks",
            &KILL_AFTER_CHUNKS.to_string(),
        ],
    )?;
    checks.expect(
        spawned.killed(),
        "子进程被 SIGKILL 杀死（不是返回错误）",
        &format!("code={:?} signal={:?}", spawned.output.code, spawned.output.signal),
    );
    let raw = crate::fsutil::read_file(&spawned.log)?;
    // 崩溃前应完整落盘的字节数 = 前 CRASH_SEQ-1 条记录的长度之和。
    let expected_good: usize = (1..CRASH_SEQ)
        .map(|seq| crash::fixture_record(seq).encoded_len())
        .sum();
    let complete = expected_good;
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
    Ok(complete)
}

/// 2b：撕裂落在头中间，缺陷必须是"头不完整"而不是被误判成损坏。
fn header_tear_case(checks: &mut Checks, root: &Path, complete: usize) -> Result<()> {
    let header_case = spawn_writer(
        root,
        "truncated-header",
        &[
            "--mode",
            "crash",
            "--crash-at",
            &CRASH_SEQ.to_string(),
            "--chunk",
            &HEADER_CHUNK.to_string(),
            "--kill-after-chunks",
            &KILL_AFTER_CHUNKS.to_string(),
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
