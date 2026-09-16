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
fn run_all() -> Result<bool> {
    let workspace = PathBuf::from(
        std::env::var("SCHOLIUM_SPIKE_WORKSPACE")
            .unwrap_or_else(|_| "/tmp/scholium-recovery-workspace".to_string()),
    );
    let _ = std::fs::remove_dir_all(&workspace);
    fsutil::write_file(&workspace.join(".keep"), b"workspace root\n")?;

    let mut checks = Checks::new();
    println!("Scholium 阶段 0 第 5 项 spike：构建/恢复");
    println!("工作目录: {}\n", workspace.display());

    criteria::build_isolation::run(&mut checks, &workspace)?;
    criteria::wal_recovery::run(&mut checks, &workspace)?;
    criteria::source_conflict::run(&mut checks, &workspace)?;
    criteria::random_truncate::run(&mut checks, &workspace)?;

    let all_pass = checks.finish();
    println!(
        "\n总结论: {}（用例 {} 个独立判定）",
        if all_pass { "PASS" } else { "FAIL" },
        checks.case_count()
    );
    Ok(!all_pass)
}
