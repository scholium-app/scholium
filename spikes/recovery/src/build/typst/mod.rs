//! Typst 引擎：**进程内**编译，不落中间文件。
//!
//! 与 LaTeX 路径的差异必须在报告里写清：Typst 没有外部子进程，因此
//! "中间文件隔离"对它而言是**结构性的**（编译在内存里，唯一落盘的是产物），
//! 而 LaTeX 的隔离是**靠沙箱目录实现的**。判据要求"中间文件与产物分目录隔离"，
//! 对 Typst 的验证方式是断言它的隔离目录里只有源文件与产物，没有任何引擎中间文件。
//!
//! 另一个真实差异：**Typst 的持久化表示不是字节流**。本 spike 的依赖清单里没有
//! `typst-pdf`，因此产物是"编译成功 + 页数 + 页内容哈希"的确定性文本。
//! 这个哈希来自 `PagedDocument` 的每一页，页数或页面内容一变哈希就变，
//! 足以支撑隔离判据；但它**不是 PDF**，报告里把这一点列为缺口。

pub mod world;

use std::hash::{Hash, Hasher};

use typst::World as _;
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

    // 库哈希把 Typst 版本/标准库内容钉进产物：换 typst 版本时产物必然变化，
    // 不会出现"A 仍然产出同样的字节"这种假隔离通过。
    let library_hash = {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        world.library().hash(&mut hasher);
        hasher.finish()
    };

    let product_path = request
        .out_dir
        .join(format!("{}.{}", request.job_name, request.engine.product_ext()));
    let product = format!(
        "typst-paged\npages={pages}\npage-hash={page_hash}\nlibrary-hash={library_hash:016x}\n"
    );
    fsutil::write_file(&product_path, product.as_bytes())?;

    Ok(CompiledProduct {
        product_path,
        pages: Some(pages),
        engine_note: format!(
            "typst crate 进程内编译；warnings={}；page-hash={page_hash}",
            warnings.len()
        ),
    })
}

/// 页内容的确定性哈希：`PagedDocument` 自己实现了 `Hash`（页与文档信息都参与，
/// introspector 由页派生因此不重复计入）。
fn page_content_hash(document: &PagedDocument) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    document.hash(&mut hasher);
    fsutil::sha256_hex(format!("typst-paged:{:016x}", hasher.finish()).as_bytes())[..16].to_string()
}
