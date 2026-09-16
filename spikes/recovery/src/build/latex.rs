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

    // latexmk 失败时保持 `CommandFailed` 而不是包成 `Core`：错误分类是判据的一部分，
    // 调用方要能区分"引擎跑了但文档编译不过"和"隔离/锁/路径出错"。
    process::run_checked("latexmk", &args, &request.out_dir)?;

    let product_path = request
        .out_dir
        .join(format!("{}.pdf", request.job_name));
    if !product_path.exists() {
        return Err(SpikeError::Core(format!(
            "latexmk 成功但产物缺失: {}（沙箱文件: {:?}）",
            product_path.display(),
            crate::fsutil::list_names(&request.out_dir).unwrap_or_default()
        )));
    }

    let pages = pdf_pages(&product_path)?;
    Ok(CompiledProduct {
        product_path,
        pages,
        engine_note: format!(
            "{}；{}",
            process::version_line("latexmk", &["-v"]),
            process::version_line("xelatex", &["--version"])
        ),
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
