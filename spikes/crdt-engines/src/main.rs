//! 阶段 0 补充验证：**CRDT 引擎对比夹具**（Loro / Yrs / Automerge）。
//!
//! 关闭 ADR 0009。四个检查在同一份语义上跑：
//! 1. 两副本离线文本编辑后收敛；
//! 2. 本地撤销保留远端输入；
//! 3. 快照往返；
//! 4. 可移动树（创建 + 移动）是否原生支持。
//!
//! 用法：`cargo run --release`

mod automerge_engine;
mod loro_engine;
mod yrs_engine;

fn main() {
    for (label, checks) in [
        ("Loro 1.16.0", loro_engine::run()),
        ("Yrs 0.27.4", yrs_engine::run()),
        ("Automerge 0.11.0", automerge_engine::run()),
    ] {
        println!("=== {label} ===");
        let mut pass = 0;
        for (name, ok, evidence) in &checks {
            println!(
                "  [{}] {name}：{evidence}",
                if *ok { "PASS" } else { "FAIL" }
            );
            if *ok {
                pass += 1;
            }
        }
        println!("  {label}：{pass}/{} 通过", checks.len());
        println!();
    }
}
