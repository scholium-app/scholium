//! 阶段 0 第 6 项：团队语言协调验证（`cargo run --release` 打印全部证据）。
//!
//! 内存模拟 + 确定性逻辑时钟；无真实网络、无真实多进程。
//! 判据与夹具见 `docs/spikes/0009-team-language.md`。

fn main() {
    println!("Scholium spike：阶段 0 第 6 项 — 团队语言协调");
    println!("模型：内存协调者 + 确定性逻辑时钟；无真实网络 / 无真实多进程");

    let passed = scholium_spike_language_coordination::run_validation();
    println!(
        "\n未验证范围：真实网络分区、真实多进程、真实磁盘 fsync、真实 CRDT 合并算法、\
         真实 LaTeX/Typst 解析器；见报告 0009 的“失败与不确定性”。"
    );
    if !passed {
        std::process::exit(1);
    }
}
