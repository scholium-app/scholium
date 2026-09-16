//! 阶段 0 第 6 项：团队语言协调验证（`cargo run --release` 打印全部证据）。
//!
//! 内存模拟 + 确定性逻辑时钟；无真实网络、无真实多进程。
//! 判据与夹具见 `docs/spikes/0009-team-language.md`。

mod coordinator;
mod crdt;
mod error;
mod fixtures;
mod harness;
mod model;
mod scenarios;
mod timeline;
mod validate;
mod wal;

use harness::Harness;

fn main() {
    println!("Scholium spike：阶段 0 第 6 项 — 团队语言协调");
    println!(
        "协议版本={} 许可 TTL={} tick 负载上限={} 字节 单包操作上限={}",
        model::PROTOCOL_VERSION,
        model::PERMIT_TTL_TICKS,
        model::MAX_PAYLOAD_BYTES,
        model::MAX_OPS
    );
    println!("模型：内存协调者 + 确定性逻辑时钟；无真实网络 / 无真实多进程");

    let mut harness = Harness::new();
    scenarios::c1_same_language::run(&mut harness);
    scenarios::c2_barrier::run(&mut harness);
    scenarios::c3_epoch::run(&mut harness);
    scenarios::c4_restart::run(&mut harness);
    scenarios::c5_partition::run(&mut harness);
    scenarios::c6_stale::run(&mut harness);
    scenarios::c7_malicious::run(&mut harness);

    let passed = harness.finish();
    println!(
        "\n未验证范围：真实网络分区、真实多进程、真实磁盘 fsync、真实 CRDT 合并算法、\
         真实 LaTeX/Typst 解析器；见报告 0009 的“失败与不确定性”。"
    );
    if !passed {
        std::process::exit(1);
    }
}
