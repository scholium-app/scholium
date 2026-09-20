//! Typst 引擎：由 OS 隔离 worker 调用，不落引擎中间文件。
//!
//! 另一个真实差异：**Typst 的持久化表示不是字节流**。本 spike 的依赖清单里没有
//! `typst-pdf`，因此产物是"编译成功 + 页数 + 页内容哈希"的确定性文本。
//! 这个哈希来自每页编译后的 SVG，页数或页面内容一变哈希就变，
//! 足以支撑隔离判据；但它**不是 PDF**，报告里把这一点列为缺口。

pub mod world;

use typst::diag::Warned;
use typst_layout::PagedDocument;

use crate::build::{BuildRequest, CompiledProduct};
use crate::error::{Result, SpikeError};
use crate::fsutil;

/// 编译 Typst 源码。
///
/// # Errors
///
/// 源码有致命错误，或产物写不出去。
pub fn compile(request: &BuildRequest) -> Result<CompiledProduct> {
    let world = world::SpikeWorld::new(request.source_text.clone());
    let Warned { output, warnings } = typst::compile::<PagedDocument>(&world);

    let document = output.map_err(|errors| {
        let detail: Vec<String> = errors
            .iter()
            .take(4)
            .map(|error| error.message.to_string())
            .collect();
        SpikeError::Core(format!("Typst 编译失败: {}", detail.join(" | ")))
    })?;

    let pages = document.pages().len();
    let page_hash = page_content_hash(&document);

    let product_path = request.out_dir.join(format!(
        "{}.{}",
        request.job_name,
        request.engine.product_ext()
    ));
    let product =
        format!("typst-paged\npages={pages}\npage-hash={page_hash}\nengine={TYPST_VERSION_NOTE}\n");
    fsutil::write_file(&product_path, product.as_bytes())?;

    Ok(CompiledProduct {
        product_path,
        pages: Some(pages),
        engine_note: format!(
            "{TYPST_VERSION_NOTE} 沙箱 worker 编译；warnings={}；page-hash={page_hash}",
            warnings.len()
        ),
    })
}

/// 写进产物与证据行的 Typst 版本。
///
/// 硬编码在这里**只用于人读的证据行**，不是锁定机制；锁定机制是 `Cargo.toml` 的
/// `=0.15.1` 与 `Cargo.lock` 的实际解析结果。Typst 是库依赖，没有独立 CLI 版本查询。
const TYPST_VERSION_NOTE: &str = "typst 0.15.1";

/// 页 SVG 的确定性内容签名，不使用含进程内标识的内部 Hash。
fn page_content_hash(document: &PagedDocument) -> String {
    // Internal Hash contains process-local identities. Rendered SVG gives independent
    // workers the same content signature and still detects glyph/layout changes.
    let hashes: Vec<_> = document
        .pages()
        .iter()
        .map(|page| fsutil::sha256_hex(typst_svg::svg(page, &Default::default()).as_bytes()))
        .collect();
    fsutil::sha256_hex(hashes.join("\n").as_bytes())[..16].to_string()
}
