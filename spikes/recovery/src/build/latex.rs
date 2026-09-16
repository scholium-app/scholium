//! LaTeX 引擎：把源码分发进沙箱，用 TeX Live 的 `latexmk -pdfxe` 编译。
//!
//! 选择 `latexmk` 而不是直接调 `xelatex`：`latexmk` 会自己决定需要跑几轮才能收敛
//! 交叉引用与页码，正是"多轮中间文件"最容易泄漏到别的目录的那类工具。
//! 用最会写中间文件的工具来验证隔离，比用一个只写一次的简单命令更有说服力。

use std::path::{Path, PathBuf};

use crate::build::{BuildRequest, CompiledProduct};
use crate::error::{Result, SpikeError};
use crate::process;

/// 编译 LaTeX 源码。
///
/// # Errors
///
/// `latexmk` 无法启动、返回非零退出码，或产物文件没有生成。
pub fn compile(request: &BuildRequest) -> Result<CompiledProduct> {
    let staged = crate::build::sandbox::stage_source(request)?;
    let args = latexmk_args(request, &staged);

    let output = process::run_checked("latexmk", &args, &request.out_dir).map_err(|error| {
        // latexmk 的失败信息在 stdout 的日志里，错误消息必须带上最后几行，
        // 否则报告里只剩一个退出码，无法判断是隔离失败还是文档本身编译不过。
        SpikeError::Core(match error {
            SpikeError::CommandFailed {
                program,
                code,
                stderr,
            } => format!("{program} 退出 {code:?}: {stderr}"),
            other => format!("{other}"),
        })
    })?;

    let product_path = request
        .out_dir
        .join(format!("{}.pdf", request.job_name));
    if !product_path.exists() {
        return Err(SpikeError::Core(format!(
            "latexmk 成功但产物缺失: {}；stdout 尾部: {}",
            product_path.display(),
            output.summary(6)
        )));
    }

    let pages = pdf_pages(&product_path)?;
    Ok(CompiledProduct {
        product_path,
        pages,
        engine_note: format!("latexmk + xelatex；{}", first_line(&output.stdout)),
    })
}

/// 组装 `latexmk` 参数。
///
/// `-outdir` 与 `-jobname` 同时给：前者让中间文件进沙箱子目录，后者让文件名带上引擎名，
/// 于是"产物是哪个引擎写的"在文件名层面就无歧义。
fn latexmk_args(request: &BuildRequest, staged: &Path) -> Vec<String> {
    vec![
        "-pdfxe".to_string(),
        "-interaction=nonstopmode".to_string(),
        "-halt-on-error".to_string(),
        "-file-line-error".to_string(),
        format!("-outdir={}", request.out_dir.display()),
        format!("-jobname={}", request.job_name),
        staged.display().to_string(),
    ]
}

/// 用 `pdfinfo` 读页数；读不到不是失败，只是没有这个证据。
fn pdf_pages(path: &Path) -> Result<Option<usize>> {
    let output = match process::run("pdfinfo", &[path.display().to_string()], Path::new(".")) {
        Ok(output) => output,
        Err(_) => return Ok(None),
    };
    Ok(process::parse_pdf_pages(&output.stdout))
}

fn first_line(text: &str) -> String {
    text.lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("无输出版本行")
        .trim()
        .to_string()
}

/// 沙箱里应出现的 LaTeX 中间文件扩展名（证据用，不参与判定逻辑）。
pub const INTERMEDIATE_SUFFIXES: &[&str] = &["aux", "fls", "fdb_latexmk", "log", "out", "toc"];

/// 从文件清单里挑出中间文件。
pub fn intermediates(names: &[String]) -> Vec<String> {
    names
        .iter()
        .filter(|name| {
            PathBuf::from(name)
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| INTERMEDIATE_SUFFIXES.contains(&ext))
        })
        .cloned()
        .collect()
}
