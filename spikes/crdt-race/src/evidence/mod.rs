//! 判据 J1–J5 的证据场景。
//!
//! 每个场景只做三件事：构造状态、执行动作、把**逐用例**的结论交给 [`Evidence`]。
//! 场景之间不共享可变状态，因此任何一条失败都能单独复跑。

pub(crate) mod convergence;
pub(crate) mod random;
pub(crate) mod scale;
pub(crate) mod snapshot;
pub(crate) mod undo;

use crate::error::CrdtError;
use crate::fixture::{FIXTURE_SEED, Fixture, bootstrap};
use crate::ids::ActorId;
use crate::replica::Replica;
use crate::report::Evidence;

/// 副本 A 的参与者身份。
pub(crate) const ACTOR_A: ActorId = ActorId(1);
/// 副本 B 的参与者身份。
pub(crate) const ACTOR_B: ActorId = ActorId(2);
/// 副本 C 的参与者身份（三副本随机化判据使用）。
pub(crate) const ACTOR_C: ActorId = ActorId(3);

/// 建立一对同源副本：A 是引导副本，B 有独立随机流。
///
/// # Errors
///
/// 夹具构造失败时返回错误。
pub(crate) fn pair(seed_b: u64) -> Result<(Replica, Replica, Fixture), CrdtError> {
    let (a, fixture) = bootstrap(ACTOR_A, FIXTURE_SEED)?;
    let b = a.fork(ACTOR_B, seed_b);
    Ok((a, b, fixture))
}

/// 交换两个副本的操作集。`a_first` 决定先合并哪一边，用于检验合并顺序不影响结果。
pub(crate) fn exchange(a: &mut Replica, b: &mut Replica, a_first: bool) -> (usize, usize) {
    if a_first {
        let ab = a.merge_from(b);
        let ba = b.merge_from(a);
        (ab, ba)
    } else {
        let ba = b.merge_from(a);
        let ab = a.merge_from(b);
        (ab, ba)
    }
}

/// 执行一个返回判据结论的闭包，把错误也记成失败用例。
pub(crate) fn check_result(ev: &mut Evidence, id: &str, result: Result<(bool, String), CrdtError>) {
    match result {
        Ok((passed, detail)) => ev.check(id, passed, detail),
        Err(error) => ev.check(id, false, format!("运行失败：{error}")),
    }
}

/// 让 `target` 接收 `source` 中尚未见过的前 `limit` 个操作。
pub(crate) fn receive_prefix(target: &mut Replica, source: &Replica, limit: usize) -> usize {
    let mut applied = 0usize;
    for op in source.ops() {
        if applied >= limit {
            break;
        }
        if target.receive(op.clone()) {
            applied += 1;
        }
    }
    applied
}
