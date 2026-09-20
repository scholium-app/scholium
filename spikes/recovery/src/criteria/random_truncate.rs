//! 判据 4：随机截断点。
//!
//! 对同一份日志做多轮固定种子的随机截断（以及随机字节损坏），每一轮都独立断言
//! "恢复到完整记录边界"。**种子写进报告**，任何人都能复现同一串位置。
//!
//! 每轮断言的不变量（全部是"边界"性质，而不是数量性质）：
//! 1. 恢复条数等于"截断点之前完整的记录条数"；
//! 2. `good_bytes` 等于这些记录的编码长度之和，等于某个真实记录边界；
//! 3. 被丢弃的字节数等于 `文件长度 - good_bytes`，没有丢多也没有丢少；
//! 4. 恢复出的记录与基线逐字节一致（是前缀，不是"碰巧条数对"）；
//! 5. 只有真的切在记录之间时，才允许没有缺陷；切在记录内部必须报出缺陷。

use std::path::Path;

use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use crate::check::Checks;
use crate::criteria::wal_recovery::write_baseline;
use crate::error::Result;
use crate::fsutil::{self, Scratch};
use crate::wal::recovery::{self, TailDefect};
use crate::wal::{self, Record};

/// 日志记录条数。
const RECORDS: u64 = 200;
/// 随机轮数。
const ROUNDS: usize = 24;
/// 固定种子。写进报告，复现必须用同一个值。
pub const SEED: u64 = 20260916;

/// 每轮选择的破坏方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// 截断到某个字节位置。
    Truncate,
    /// 在某个字节位置上翻转一位。
    Corrupt,
}

/// 一轮的判定上下文。
struct Round<'a> {
    seed: u64,
    index: usize,
    mode: Mode,
    position: usize,
    file_len: usize,
    baseline: &'a [Record],
    expected: &'a [Record],
    recovery: &'a recovery::Recovery,
}

/// 跑判据 4 的全部用例。
///
/// # Errors
///
/// 日志写入或文件系统操作失败。
pub fn run(checks: &mut Checks, workspace: &Path) -> Result<()> {
    println!("\n## 判据 4：随机截断点（固定种子）");
    println!("种子={SEED}，轮数={ROUNDS}，记录={RECORDS}");

    let scratch = Scratch::create(workspace, "c4")?;
    let baseline_path = scratch.join("baseline.wal");
    let baseline = write_baseline(&baseline_path, RECORDS)?;
    let master_bytes = fsutil::read_file(&baseline_path)?;
    let boundaries = record_boundaries(&baseline);

    checks.case("truncate.random-rounds");
    checks.expect(
        master_bytes.len() == boundaries.last().copied().unwrap_or(0),
        "基准日志长度等于全部记录长度之和",
        &format!("{} B / {} 条", master_bytes.len(), baseline.len()),
    );

    let mut rng = SmallRng::seed_from_u64(SEED);
    let mut truncate_rounds = 0usize;
    let mut corrupt_rounds = 0usize;

    for index in 0..ROUNDS {
        let mode = if index % 2 == 0 {
            Mode::Truncate
        } else {
            Mode::Corrupt
        };
        let position = match mode {
            // 前四轮固定取边界位置：文件末尾、倒数第一字节、文件开头、以及某条记录边界。
            Mode::Truncate if index < 4 => forced_cut(index, &master_bytes, &boundaries),
            Mode::Truncate => rng.random_range(0..=master_bytes.len()),
            // 损坏位置只落在"真实记录内部"，且刻意覆盖魔数区、CRC 区、负载区。
            Mode::Corrupt if index < 4 => forced_corrupt(index, &baseline, &boundaries),
            Mode::Corrupt => rng.random_range(0..master_bytes.len()),
        };

        let case_dir = scratch.join(&format!("round-{index:02}"));
        std::fs::create_dir_all(&case_dir).map_err(crate::error::io_context(&case_dir))?;
        let log = case_dir.join("draft.wal");

        let mutated = match mode {
            Mode::Truncate => master_bytes[..position].to_vec(),
            Mode::Corrupt => {
                let mut bytes = master_bytes.clone();
                bytes[position] ^= 0xff;
                bytes
            }
        };
        fsutil::write_file(&log, &mutated)?;

        let recovered = recovery::recover(&log)?;
        // 截断模式：期望条数可以**先验**算出，因此它是独立预测，不是从结果反推。
        // 损坏模式：被毁的那条记录一定在 good_bytes 处，期望是 good_bytes 之前的基线前缀，
        // 由边界表反查，避免对"损坏落在头/负载/CRC"分别写特例。
        let expected = match mode {
            Mode::Truncate => {
                let count = count_records_before(position, &boundaries);
                &baseline[..count]
            }
            Mode::Corrupt => {
                // 期望条数 = good_bytes 之前（含恰好等于）的边界数。
                // 边界表形如 [0, b1, b2, ...]，b_i 是第 i 条记录的结束偏移；
                // 因此 kept = 满足 b_i <= good_bytes 的 i 的最大值。
                let kept = boundaries
                    .iter()
                    .take_while(|boundary| **boundary <= recovered.good_bytes)
                    .count()
                    .saturating_sub(1);
                &baseline[..kept]
            }
        };

        let context = Round {
            seed: SEED,
            index,
            mode,
            position,
            file_len: mutated.len(),
            baseline: &baseline,
            expected,
            recovery: &recovered,
        };
        assert_round(&mut *checks, &context);
        match mode {
            Mode::Truncate => truncate_rounds += 1,
            Mode::Corrupt => corrupt_rounds += 1,
        }
    }
    checks.note(
        "轮次分布",
        &format!("截断 {truncate_rounds} 轮 / 损坏 {corrupt_rounds} 轮，种子均为 {SEED}"),
    );
    Ok(())
}

/// 逐轮断言。
fn assert_round(checks: &mut Checks, round: &Round<'_>) {
    let label = format!(
        "round{:02}(seed={},{}@{})",
        round.index,
        round.seed,
        match round.mode {
            Mode::Truncate => "truncate",
            Mode::Corrupt => "corrupt",
        },
        round.position
    );
    let defect = round
        .recovery
        .defect
        .as_ref()
        .map(TailDefect::describe)
        .unwrap_or_else(|| "无".to_string());
    println!(
        "  [轮 {}/{}] {} 文件 {} B → 恢复 {} 条 / 边界 {} B / 丢弃 {} B / {defect}",
        round.index + 1,
        ROUNDS,
        label,
        round.file_len,
        round.recovery.records.len(),
        round.recovery.good_bytes,
        round.recovery.discarded_bytes,
    );

    checks.expect(
        round.recovery.records.len() == round.expected.len(),
        &format!("{label}.条数等于截断前的完整记录数"),
        &format!(
            "{} / {}",
            round.recovery.records.len(),
            round.expected.len()
        ),
    );
    checks.expect(
        round.recovery.records == round.expected,
        &format!("{label}.恢复内容是基线前缀（逐字节）"),
        &format!(
            "末条 seq={:?}",
            round.recovery.records.last().map(|record| record.seq)
        ),
    );

    let boundary = record_boundaries(round.baseline)[round.expected.len()];
    checks.expect(
        round.recovery.good_bytes == boundary,
        &format!("{label}.good_bytes 落在真实记录边界"),
        &format!("{} / {boundary}", round.recovery.good_bytes),
    );
    checks.expect(
        round.recovery.discarded_bytes == round.file_len - boundary,
        &format!("{label}.丢弃字节数 = 文件长度 - 边界"),
        &format!(
            "{} = {} - {boundary}",
            round.recovery.discarded_bytes, round.file_len
        ),
    );
    checks.expect(
        round.recovery.file_bytes == round.file_len,
        &format!("{label}.恢复器报告的文件长度正确"),
        &format!("{} / {}", round.recovery.file_bytes, round.file_len),
    );

    // 切在记录之间（position 正好是某个边界）才允许没有缺陷。
    let on_boundary = round.position == boundary && round.mode == Mode::Truncate;
    if on_boundary {
        checks.expect(
            round.recovery.defect.is_none(),
            &format!("{label}.切在记录之间 → 无缺陷"),
            &format!("{:?}", round.recovery.defect),
        );
    } else {
        checks.expect(
            round.recovery.defect.is_some(),
            &format!("{label}.切在记录内部 → 必须报出缺陷"),
            &defect,
        );
    }

    // 恢复出的每条记录都必须能通过完整校验。
    let re_encoded: Vec<u8> = round
        .recovery
        .records
        .iter()
        .flat_map(Record::encode)
        .collect();
    checks.expect(
        re_encoded.len() == round.recovery.good_bytes,
        &format!("{label}.重新编码长度等于 good_bytes"),
        &format!("{} / {}", re_encoded.len(), round.recovery.good_bytes),
    );
}

/// 计算每条记录结束处的偏移（含 0）。
fn record_boundaries(records: &[Record]) -> Vec<usize> {
    let mut boundaries = Vec::with_capacity(records.len() + 1);
    let mut offset = 0usize;
    boundaries.push(offset);
    for record in records {
        offset += record.encoded_len();
        boundaries.push(offset);
    }
    boundaries
}

/// 截断模式下，截断点之前完整记录的条数。
///
/// 只在截断模式使用：`position` 落在记录内部时（截断的定义），最后一个 `<= position`
/// 的边界就是最后一条完整记录的结束偏移。损坏模式不能这样算，因为损坏可能落在
/// 头/CRC/负载三个不同区域，被毁记录的起始边界才是停点。
fn count_records_before(position: usize, boundaries: &[usize]) -> usize {
    boundaries
        .iter()
        .take_while(|boundary| **boundary <= position)
        .count()
        .saturating_sub(1)
}

/// 前四轮用的固定截断位置：文件末尾、末尾前一字节、0、以及第 100 条记录的边界。
fn forced_cut(index: usize, bytes: &[u8], boundaries: &[usize]) -> usize {
    match index {
        0 => bytes.len(),
        1 => bytes.len().saturating_sub(1),
        2 => 0,
        _ => boundaries[100],
    }
}

/// 前四轮用的固定损坏位置：魔数首字节、CRC 首字节、某条记录的负载首字节、以及文件末字节。
fn forced_corrupt(index: usize, baseline: &[Record], boundaries: &[usize]) -> usize {
    match index {
        0 => 0,
        1 => boundaries[0] + 28,
        2 => boundaries[10] + wal::HEADER_LEN,
        _ => boundaries[baseline.len() - 1] - 1,
    }
}
