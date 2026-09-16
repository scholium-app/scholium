//! 判据 J4：两副本混合 100 000 次用户动作。
//!
//! 计量的动作是**用户动作**（一次插入 / 删除 / 属性变更 / 包裹 / 解除），不是 CRDT 内部操作；
//! 每 500 个动作同步一次，模拟真实会话的周期性同步。峰值内存取自主分配器计数器的区间增量，
//! 不依赖外部工具。

use std::time::Instant;

use crate::alloc_counter;
use crate::codec::fnv1a;
use crate::error::CrdtError;
use crate::evidence::{ACTOR_A, ACTOR_B};
use crate::fixture::{FIXTURE_SEED, bootstrap};
use crate::report::Evidence;
use crate::rng::Rng;
use crate::workload::Workload;

/// 混合用户动作总数（两副本之和）。
const TOTAL_ACTIONS: usize = 100_000;
/// 每多少个动作做一次双向同步。
const SYNC_EVERY: usize = 500;
/// 动作调度种子：决定每一步落在哪个副本。
const SCHEDULE_SEED: u64 = 0x5CA1_E001;
/// 副本 A 的工作负载种子。
const WORKLOAD_SEED_A: u64 = 0x5CA1_E002;
/// 副本 B 的工作负载种子。
const WORKLOAD_SEED_B: u64 = 0x5CA1_E003;
/// 堆峰值增量上限：256 MiB。
const MEMORY_LIMIT_BYTES: usize = 256 * 1024 * 1024;

/// 规模判据的运行结果。
struct ScaleReport {
    converged: bool,
    summary: String,
    peak_delta: usize,
    heap_after: usize,
    canonical_bytes: usize,
}

/// 判据 J4 入口。
pub(crate) fn run(ev: &mut Evidence) {
    ev.section("判据 J4：规模（两副本混合 100 000 次用户动作）");
    match scale_run() {
        Ok(report) => {
            ev.check("J4.1", report.converged, report.summary);
            ev.check(
                "J4.2",
                report.peak_delta < MEMORY_LIMIT_BYTES,
                format!(
                    "堆峰值增量 {:.1} MiB < 上限 {:.0} MiB；结束时堆占用增量 {:.1} MiB；规范状态 {} 字节",
                    mib(report.peak_delta),
                    mib(MEMORY_LIMIT_BYTES),
                    mib(report.heap_after),
                    report.canonical_bytes
                ),
            );
        }
        Err(error) => {
            ev.check("J4.1", false, format!("运行失败：{error}"));
            ev.check("J4.2", false, format!("运行失败：{error}"));
        }
    }
}

fn scale_run() -> Result<ScaleReport, CrdtError> {
    let (bootstrap_replica, fixture) = bootstrap(ACTOR_A, FIXTURE_SEED)?;
    let mut a = bootstrap_replica;
    let duplicate = a.fork(ACTOR_B, SCHEDULE_SEED);
    let mut b = duplicate;
    let base_ops = a.op_count();
    let mut workload_a = Workload::new(WORKLOAD_SEED_A);
    let mut workload_b = Workload::new(WORKLOAD_SEED_B);
    let mut schedule = Rng::new(SCHEDULE_SEED ^ 0x51);

    let baseline = alloc_counter::current();
    alloc_counter::reset_peak();
    let start = Instant::now();
    let mut side_a = 0usize;
    let mut side_b = 0usize;
    for round in 0..TOTAL_ACTIONS {
        if schedule.chance(50) {
            workload_a.apply_one(&mut a, &fixture)?;
            side_a += 1;
        } else {
            workload_b.apply_one(&mut b, &fixture)?;
            side_b += 1;
        }
        if (round + 1) % SYNC_EVERY == 0 {
            a.merge_from(&b);
            b.merge_from(&a);
        }
    }
    a.merge_from(&b);
    b.merge_from(&a);
    let elapsed = start.elapsed();
    let peak = alloc_counter::peak();
    let heap_after = alloc_counter::current();

    let bytes_a = a.canonical();
    let bytes_b = b.canonical();
    let converged = bytes_a == bytes_b;
    let (entries, visible, chunks) = a.doc().text_totals();
    let (ins_a, del_a, attr_a, wrap_a, unwrap_a) = workload_a.counts();
    let (ins_b, del_b, attr_b, wrap_b, unwrap_b) = workload_b.counts();
    let total_ops = a.op_count();
    let nodes = a.doc().tree().node_count();
    let units = TOTAL_ACTIONS as u128;
    let ns_per_action = elapsed.as_nanos() / units;
    let summary = format!(
        "用户动作 {TOTAL_ACTIONS}（A={side_a} / B={side_b}：插入 {} 删除 {} 属性 {} 包裹 {} 解除 {}）；\
         操作总数 {total_ops}（夹具引导 {base_ops}）；节点 {nodes}；文本条目 {entries}（可见 {visible}、墓碑 {}），最大块 {chunks}；\
         总耗时 {elapsed:?}（{ns_per_action} ns/动作）；两端收敛 {converged}（hash {:#018x}）",
        ins_a + ins_b,
        del_a + del_b,
        attr_a + attr_b,
        wrap_a + wrap_b,
        unwrap_a + unwrap_b,
        entries - visible,
        fnv1a(&bytes_a)
    );
    Ok(ScaleReport {
        converged,
        summary,
        peak_delta: peak.saturating_sub(baseline),
        heap_after: heap_after.saturating_sub(baseline),
        canonical_bytes: bytes_a.len(),
    })
}

fn mib(bytes: usize) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}
