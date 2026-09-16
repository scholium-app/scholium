//! Scholium 阶段 0 第 5 项验证 spike：构建隔离、WAL 恢复、外部源码冲突、随机截断。
//!
//! 用法：
//!
//! ```text
//! cargo run --release                 # 跑全部判据并打印证据
//! cargo run --release -- writer ...   # 崩溃夹具子进程（主进程用）
//! ```

mod build;
mod check;
mod cli;
mod crash;
mod criteria;
mod error;
mod fixture;
mod fsutil;
mod process;
mod render;
mod wal;

use std::path::PathBuf;

use check::Checks;
use error::{Result, SpikeError};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match cli::subcommand(&args) {
        // 崩溃夹具以子进程身份启动；只有这一条路径会走 writer。
        Some(command) if command == "writer" => crash::run(&args),
        Some(other) => Err(SpikeError::Core(format!("未知子命令: {other}"))),
        None => {
            let failed = run_all()?;
            if failed {
                // 非零退出码让 CI 直接失败，而不是让人去读日志判断。
                std::process::exit(1);
            }
            Ok(())
        }
    }
}

/// 跑完全部判据，返回是否存在失败。
///
/// `SCHOLIUM_SPIKE_REPEAT=2` 让整轮判据再跑一遍：第二轮用**全新的目录与全新的进程**，
/// 用它的逐用例汇总行与第一轮逐行对比，证明结果不是靠残留状态或运气得到的。
fn run_all() -> Result<bool> {
    let repeat: usize = std::env::var("SCHOLIUM_SPIKE_REPEAT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1)
        .max(1);

    let mut rounds = Vec::new();
    let mut all_pass = true;
    for round in 0..repeat {
        let (passed, summary) = run_round(round)?;
        all_pass &= passed;
        rounds.push(summary);
    }

    if rounds.len() > 1 {
        println!("\n== 两轮一致性 ==");
        let identical = rounds[0] == rounds[1];
        println!(
            "  [{}] 第 1 轮与第 2 轮逐用例汇总完全一致",
            if identical { "PASS" } else { "FAIL" }
        );
        if !identical {
            all_pass = false;
            println!("  第一轮:\n{}", rounds[0]);
            println!("  第二轮:\n{}", rounds[1]);
        }
    }
    Ok(!all_pass)
}

/// 跑一轮判据，返回是否全过与逐用例汇总文本。
fn run_round(round: usize) -> Result<(bool, String)> {
    let base = std::env::var("SCHOLIUM_SPIKE_WORKSPACE")
        .unwrap_or_else(|_| "/tmp/scholium-recovery-workspace".to_string());
    let workspace = PathBuf::from(format!("{base}-r{round}"));
    let _ = std::fs::remove_dir_all(&workspace);
    fsutil::write_file(&workspace.join(".keep"), b"workspace root\n")?;

    let mut checks = Checks::new();
    println!("Scholium 阶段 0 第 5 项 spike：构建/恢复（第 {} 轮）", round + 1);
    println!("工作目录: {}\n", workspace.display());

    criteria::build_isolation::run(&mut checks, &workspace)?;
    criteria::wal_recovery::run(&mut checks, &workspace)?;
    criteria::source_conflict::run(&mut checks, &workspace)?;
    criteria::random_truncate::run(&mut checks, &workspace)?;

    let passed = checks.finish();
    println!(
        "\n本轮总结论: {}（用例 {} 个独立判定）",
        if passed { "PASS" } else { "FAIL" },
        checks.case_count()
    );
    Ok((passed, checks.summary_text()))
}
