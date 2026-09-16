//! 阶段 0 第 4 项 "CRDT 赛马" 验证 spike 的入口。
//!
//! 自研最小 CRDT，不依赖任何现成 CRDT 库：
//!
//! - 文本：Logoot 类序列（每字符全局唯一标识 + 不可变位置标识）+ LWW 存活位；
//! - 结构：操作日志 + Lamport 时间戳，节点标识稳定，父子 / 兄弟位置与属性都是 LWW 寄存器；
//! - 撤销：按 actor 独立的动作栈 + 稳定标识重定位 + 时间戳指纹校验，undo 生成新的补偿操作。
//!
//! `cargo run --release` 会把判据 J1–J5 的逐用例结论打印到 stdout，并以失败数作为退出码。

mod alloc_counter;
mod codec;
mod doc;
mod error;
mod evidence;
mod fixture;
mod ids;
mod model;
mod op;
mod position;
mod replica;
mod report;
mod rng;
mod seq;
mod text;
mod tree;
mod undo;
mod workload;

/// 统计堆占用的全局分配器，用于规模判据的内存证据。
#[global_allocator]
static ALLOCATOR: alloc_counter::CountingAllocator = alloc_counter::CountingAllocator;

fn main() {
    println!("=== Scholium 阶段 0 第 4 项验证：CRDT 赛马 ===");
    println!("实现：自研 Logoot 序列 + LWW 寄存器 + Lamport 时间戳（无第三方 CRDT 库）");
    let wall = std::time::Instant::now();
    let mut evidence = report::Evidence::new();
    evidence::convergence::run(&mut evidence);
    evidence::undo::run(&mut evidence);
    evidence::snapshot::run(&mut evidence);
    evidence::scale::run(&mut evidence);
    evidence::random::run(&mut evidence);
    let failed = evidence.summary();
    println!("全部场景墙钟时间 {:?}", wall.elapsed());
    std::process::exit(if failed == 0 { 0 } else { 1 });
}
