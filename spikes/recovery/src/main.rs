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
mod error;
mod fixture;
mod fsutil;
mod process;
mod render;
mod wal;

use std::path::PathBuf;

use build::{BuildRequest, Engine};
use error::{Result, SpikeError};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match cli::subcommand(&args) {
        // 崩溃夹具以子进程身份启动；只有这一条路径会走 writer。
        Some(command) if command == "writer" => crash::run(&args),
        Some(other) => Err(SpikeError::Core(format!("未知子命令: {other}"))),
        None => probe(),
    }
}

/// 临时探针：验证两条构建路径能跑通，不作为交付物。
fn probe() -> Result<()> {
    let (editor, _paragraph) = fixture::editor();
    let latex_source = render::latex(editor.document());
    let typst_source = render::typst(editor.document());
    println!("latex source {} B", latex_source.len());
    println!("typst source {} B", typst_source.len());

    let root = PathBuf::from("/tmp/scholium-recovery-probe");
    let _ = std::fs::remove_dir_all(&root);
    fsutil::write_file(&root.join(".probe"), b"probe")?;

    for engine in [Engine::Latex, Engine::Typst] {
        let text = match engine {
            Engine::Latex => latex_source.clone(),
            Engine::Typst => typst_source.clone(),
        };
        let out_dir = root.join(engine.slug());
        let request = BuildRequest {
            engine,
            sandbox: root.join(engine.slug()),
            out_dir: out_dir.clone(),
            job_name: format!("fixture-{}", engine.slug()),
            source_path: out_dir.join(format!("fixture-{}.{}", engine.slug(), engine.source_ext())),
            source_text: text.clone(),
            source_hash: fsutil::sha256_hex(text.as_bytes()),
        };
        let outcome = build::build(&request)?;
        println!(
            "[{}] {} ms, pages={:?}, product={} B, files={:?}",
            engine.slug(),
            outcome.elapsed_ms,
            outcome.pages,
            outcome.product_bytes,
            outcome.sandbox_files
        );
        println!("      note: {}", outcome.engine_note);
    }
    Ok(())
}
