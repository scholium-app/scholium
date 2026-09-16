//! 两种引擎的隔离构建：同一个构建请求要么进 LaTeX，要么进 Typst，
//! 每条路径都有**独占的沙箱目录**，中间文件只出现在沙箱里，产物原子落回输出目录。
//!
//! 隔离模型（三条都必须在判据里被断言，不能只在注释里声称）：
//!
//! 1. **沙箱分发**：源文件复制进沙箱，编译在沙箱里跑，因此 `.aux` / `.fls` / `.log`
//!    这类中间文件不可能出现在别的引擎的目录里。
//! 2. **排他锁**：输出目录的写操作要拿 `LOCK`，第二个写者立即失败而不是排队覆盖。
//! 3. **输入只读**：A 的编译只读 A 的沙箱，B 沙箱即使放着同名文件也不会被读到。
//!    这一条靠"给 B 放毒饵，断言 A 的产物不变"来证明，而不是靠代码走查。

pub mod latex;
pub mod plan;
pub mod sandbox;
pub mod typst;

use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::error::Result;
use crate::fsutil;

/// 构建引擎。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    /// LaTeX，经 TeX Live 的 `latexmk` + `xelatex` 子进程。
    Latex,
    /// Typst，经 `typst` crate 在进程内编译。
    Typst,
}

impl Engine {
    /// 引擎短名，用于目录名与报告。
    pub fn slug(self) -> &'static str {
        match self {
            Self::Latex => "latex",
            Self::Typst => "typst",
        }
    }

    /// 产物扩展名。
    ///
    /// Typst 路径不产 PDF（依赖里没有 `typst-pdf`），因此扩展名是 `.out` 而不是 `.pdf`：
    /// 名字必须反映内容，否则报告里出现 `typst.pdf` 而实际是文本摘要，就是误导。
    pub fn product_ext(self) -> &'static str {
        match self {
            Self::Latex => "pdf",
            Self::Typst => "out",
        }
    }

    /// 源码扩展名：分发进沙箱时用，也决定引擎怎么解释这份源码。
    pub fn source_ext(self) -> &'static str {
        match self {
            Self::Latex => "tex",
            Self::Typst => "typ",
        }
    }
}

/// 一次构建请求。
#[derive(Debug)]
pub struct BuildRequest {
    /// 引擎。
    pub engine: Engine,
    /// 引擎独有的输出目录（沙箱内的子目录）。
    pub out_dir: PathBuf,
    /// `-jobname`，让不同引擎的中间文件名本身也不同名。
    pub job_name: String,
    /// 源码路径。
    pub source_path: PathBuf,
    /// 源码内容。
    pub source_text: String,
    /// 源码内容哈希，随产物一起记录，供冲突检测比对。
    pub source_hash: String,
}

/// 构建结果。
#[derive(Debug, Clone)]
pub struct BuildOutcome {
    /// 源码内容哈希（输入侧）。
    pub source_hash: String,
    /// 产物文件哈希（输出侧，逐字节）。注意：xelatex 会写入时间戳与随机 PDF ID，
    /// **跨运行不稳定**，因此判据用 [`BuildOutcome::product_signature`] 而不是它。
    pub product_hash: String,
    /// 产物内容签名（跨运行稳定）；`None` 表示缺少提取工具，报告里要算缺口。
    pub product_signature: Option<String>,
    /// 产物路径。
    pub product_path: PathBuf,
    /// 产物字节数。
    pub product_bytes: u64,
    /// 产物页数；只有能解析页数时才有值。
    pub pages: Option<usize>,
    /// 沙箱输出目录里的文件名，用于证明中间文件落在隔离目录。
    pub sandbox_files: Vec<String>,
    /// 输出目录内容哈希：`(文件名, SHA-256)`，用于"产物被并发覆盖"的断言。
    pub output_digest: Vec<(String, String)>,
    /// 引擎自报的版本/来源行。
    pub engine_note: String,
    /// 构建耗时（毫秒）。
    pub elapsed_ms: u128,
}

/// 编译产物（引擎适配层返回，`build` 负责补齐哈希与目录摘要）。
#[derive(Debug, Clone)]
pub struct CompiledProduct {
    /// 产物路径。
    pub product_path: PathBuf,
    /// 页数；读不到时为 `None`，报告里要如实写。
    pub pages: Option<usize>,
    /// 引擎自报版本/来源。
    pub engine_note: String,
}

/// 执行一次隔离构建。
///
/// # Errors
///
/// 沙箱创建失败、源码写不进去、引擎可执行文件缺失或编译失败、
/// 输出目录已被另一个写者占用。
pub fn build(request: &BuildRequest) -> Result<BuildOutcome> {
    // 先建目录再拿锁：DirLock 是 `create_new` 一个文件，目录不存在时错误会是
    // "No such file or directory"，与"锁被占用"混在一起，不利于诊断。
    std::fs::create_dir_all(&request.out_dir)
        .map_err(crate::error::io_context(&request.out_dir))?;
    let _lock = fsutil::DirLock::acquire(&request.out_dir, "LOCK")?;
    fsutil::write_file(&request.source_path, request.source_text.as_bytes())?;

    let started = Instant::now();
    let product = match request.engine {
        Engine::Latex => latex::compile(request)?,
        Engine::Typst => typst::compile(request)?,
    };
    let elapsed_ms = started.elapsed().as_millis();

    let product_bytes = fsutil::read_file(&product.product_path)?;
    let product_hash = fsutil::sha256_hex(&product_bytes);
    let product_signature =
        product_signature(request.engine, &product.product_path, product.pages)?;
    let sandbox_files = fsutil::list_names(&request.out_dir)?;
    let output_digest = digest_dir(&request.out_dir)?;

    Ok(BuildOutcome {
        source_hash: request.source_hash.clone(),
        product_hash,
        product_signature,
        product_path: product.product_path,
        product_bytes: product_bytes.len() as u64,
        pages: product.pages,
        sandbox_files,
        output_digest,
        engine_note: product.engine_note,
        elapsed_ms,
    })
}

/// 产物内容签名：**跨运行稳定**的那部分内容哈希。
///
/// 为什么不能直接用产物文件的 SHA-256：xelatex 会把运行时间与一个随机 PDF ID 写进
/// PDF，同一份源码两次编译的字节不同，用它当"隔离证据"会逼出两种坏断言——
/// 要么永远失败，要么退化成"两次都成功"这种空证据。签名只取内容：
///
/// - PDF：`pdftotext` 抽出的文本 + 页数；
/// - 其它（Typst 的文本产物）：文件字节本身（它已经是确定性的）。
///
/// `pdftotext` 缺失时返回 `None`：调用方必须把"没有签名"当成缺口，而不是当成通过。
///
/// # Errors
///
/// 产物不可读。
pub fn product_signature(
    engine: Engine,
    path: &Path,
    pages: Option<usize>,
) -> Result<Option<String>> {
    match engine {
        Engine::Latex => {
            let output = match crate::process::run(
                "pdftotext",
                &["-q".to_string(), path.display().to_string(), "-".to_string()],
                Path::new("."),
            ) {
                Ok(output) if output.succeeded() => output,
                _ => return Ok(None),
            };
            let material = format!("pages={pages:?}\n{}", output.stdout);
            Ok(Some(fsutil::sha256_hex(material.as_bytes())))
        }
        Engine::Typst => {
            let bytes = fsutil::read_file(path)?;
            Ok(Some(fsutil::sha256_hex(&bytes)))
        }
    }
}

/// 目录内容摘要：文件名到内容哈希。空目录返回空表。
///
/// # Errors
///
/// 目录不可读或其中文件不可读。
pub fn digest_dir(dir: &Path) -> Result<Vec<(String, String)>> {
    let names = fsutil::list_names(dir)?;
    let mut out = Vec::with_capacity(names.len());
    for name in names {
        if name == "LOCK" {
            continue;
        }
        let hash = fsutil::hash_file(&dir.join(&name))?;
        out.push((name, hash));
    }
    Ok(out)
}
