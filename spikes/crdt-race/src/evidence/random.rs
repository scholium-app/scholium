//! 判据 J5：固定种子下的随机化收敛（含乱序投递）。
//!
//! 每轮：建立 N 个同源副本，各自随机编辑；随后把每个副本**本轮新产生的操作**洗牌后投递给
//! 其它副本。洗牌意味着远端操作可能先于它的因果前驱到达（例如删除先于被删字符的插入、
//! 放置先于节点创建），这正是 "同步只做集合并集" 的必要条件。
//!
//! 每轮单独一行断言，种子写进证据；任何一轮失败都能用该种子复跑。

use crate::codec::fnv1a;
use crate::error::CrdtError;
use crate::evidence::{ACTOR_A, ACTOR_B, ACTOR_C, check_result};
use crate::fixture::{FIXTURE_SEED, bootstrap};
use crate::op::Op;
use crate::replica::Replica;
use crate::report::Evidence;
use crate::rng::Rng;
use crate::workload::Workload;

/// 两副本轮数。
const TWO_REPLICA_ROUNDS: usize = 100;
/// 两副本每轮的随机动作数（每副本）。
const TWO_REPLICA_OPS: usize = 200;
/// 两副本轮次的种子基准。
const TWO_REPLICA_BASE_SEED: u64 = 0x5EED_1234;
/// 三副本轮数。
const THREE_REPLICA_ROUNDS: usize = 10;
/// 三副本每轮的随机动作数（每副本）。
const THREE_REPLICA_OPS: usize = 150;
/// 三副本轮次的种子基准。
const THREE_REPLICA_BASE_SEED: u64 = 0x5EED_9876;

/// 判据 J5 入口。
pub(crate) fn run(ev: &mut Evidence) {
    ev.section("判据 J5：随机化收敛（固定种子 + 乱序投递）");
    ev.note(&format!(
        "J5.1 种子基准 {TWO_REPLICA_BASE_SEED:#x}：{TWO_REPLICA_ROUNDS} 轮 × 2 副本 × 每副本 {TWO_REPLICA_OPS} 个随机动作"
    ));
    for round in 0..TWO_REPLICA_ROUNDS {
        let seed = TWO_REPLICA_BASE_SEED + round as u64;
        check_result(
            ev,
            &format!("J5.1-round{round:03}"),
            two_replica_round(seed),
        );
    }
    ev.note(&format!(
        "J5.2 种子基准 {THREE_REPLICA_BASE_SEED:#x}：{THREE_REPLICA_ROUNDS} 轮 × 3 副本 × 每副本 {THREE_REPLICA_OPS} 个随机动作"
    ));
    for round in 0..THREE_REPLICA_ROUNDS {
        let seed = THREE_REPLICA_BASE_SEED + round as u64;
        check_result(
            ev,
            &format!("J5.2-round{round:03}"),
            three_replica_round(seed),
        );
    }
}

fn two_replica_round(seed: u64) -> Result<(bool, String), CrdtError> {
    let (bootstrap_replica, fixture) = bootstrap(ACTOR_A, FIXTURE_SEED)?;
    let mut a = bootstrap_replica;
    let base_ops = a.op_count();
    let mut b = a.fork(ACTOR_B, seed ^ 0xB0);
    let mut workload_a = Workload::new(seed ^ 0x1A);
    let mut workload_b = Workload::new(seed ^ 0x1B);
    let mut schedule = Rng::new(seed ^ 0x5C);
    for _ in 0..TWO_REPLICA_OPS {
        if schedule.chance(50) {
            workload_a.apply_one(&mut a, &fixture)?;
        } else {
            workload_b.apply_one(&mut b, &fixture)?;
        }
    }
    let new_a: Vec<Op> = a.ops()[base_ops..].to_vec();
    let new_b: Vec<Op> = b.ops()[base_ops..].to_vec();
    let mut shuffle = Rng::new(seed ^ 0xD0);
    let mut to_a = new_b.clone();
    let mut to_b = new_a.clone();
    shuffle.shuffle(&mut to_a);
    shuffle.shuffle(&mut to_b);
    for op in to_a {
        a.receive(op);
    }
    for op in to_b {
        b.receive(op);
    }
    let bytes_a = a.canonical();
    let bytes_b = b.canonical();
    let passed = bytes_a == bytes_b;
    let detail = format!(
        "seed={seed:#018x}；新操作 A={} B={}（乱序投递）；字节 {} == {}：{}；hash {:#018x} / {:#018x}；文本 {:?}",
        new_a.len(),
        new_b.len(),
        bytes_a.len(),
        bytes_b.len(),
        passed,
        fnv1a(&bytes_a),
        fnv1a(&bytes_b),
        a.doc().render()
    );
    Ok((passed, detail))
}

fn three_replica_round(seed: u64) -> Result<(bool, String), CrdtError> {
    let (bootstrap_replica, fixture) = bootstrap(ACTOR_A, FIXTURE_SEED)?;
    let mut a = bootstrap_replica;
    let base_ops = a.op_count();
    let mut b = a.fork(ACTOR_B, seed ^ 0xB1);
    let mut c = a.fork(ACTOR_C, seed ^ 0xC1);
    let mut workload_a = Workload::new(seed ^ 0x2A);
    let mut workload_b = Workload::new(seed ^ 0x2B);
    let mut workload_c = Workload::new(seed ^ 0x2C);
    let mut schedule = Rng::new(seed ^ 0x5D);
    for _ in 0..THREE_REPLICA_OPS {
        match schedule.below(3) {
            0 => workload_a.apply_one(&mut a, &fixture)?,
            1 => workload_b.apply_one(&mut b, &fixture)?,
            _ => workload_c.apply_one(&mut c, &fixture)?,
        };
    }
    let new_a: Vec<Op> = a.ops()[base_ops..].to_vec();
    let new_b: Vec<Op> = b.ops()[base_ops..].to_vec();
    let new_c: Vec<Op> = c.ops()[base_ops..].to_vec();
    let mut shuffle = Rng::new(seed ^ 0xDD);
    let mut to_a = new_b.clone();
    to_a.extend_from_slice(&new_c);
    let mut to_b = new_a.clone();
    to_b.extend_from_slice(&new_c);
    let mut to_c = new_a.clone();
    to_c.extend_from_slice(&new_b);
    shuffle.shuffle(&mut to_a);
    shuffle.shuffle(&mut to_b);
    shuffle.shuffle(&mut to_c);
    receive_all(&mut a, to_a);
    receive_all(&mut b, to_b);
    receive_all(&mut c, to_c);
    let bytes_a = a.canonical();
    let bytes_b = b.canonical();
    let bytes_c = c.canonical();
    let passed = bytes_a == bytes_b && bytes_b == bytes_c;
    let detail = format!(
        "seed={seed:#018x}；新操作 A={} B={} C={}（乱序投递）；三方字节 {}/{}/{} 相等：{}；\
         hash {:#018x} / {:#018x} / {:#018x}；文本 {:?}",
        new_a.len(),
        new_b.len(),
        new_c.len(),
        bytes_a.len(),
        bytes_b.len(),
        bytes_c.len(),
        passed,
        fnv1a(&bytes_a),
        fnv1a(&bytes_b),
        fnv1a(&bytes_c),
        a.doc().render()
    );
    Ok((passed, detail))
}

fn receive_all(replica: &mut Replica, ops: Vec<Op>) {
    for op in ops {
        replica.receive(op);
    }
}
