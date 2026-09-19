//! 沙箱分发：把源码复制进引擎独占的输出目录。
//!
//! 这里只负责暂存；OS 隔离和预算由 worker 的受信 launcher 强制实施。

use std::path::PathBuf;

use crate::build::BuildRequest;
use crate::error::Result;
use crate::fsutil;

/// 把请求里的源码写进沙箱输出目录；返回沙箱内的源码路径。
///
/// # Errors
///
/// 目录创建失败或源码写入失败。
pub fn stage_source(request: &BuildRequest) -> Result<PathBuf> {
    std::fs::create_dir_all(&request.out_dir)
        .map_err(crate::error::io_context(&request.out_dir))?;
    let staged = request.out_dir.join(format!(
        "{}.{}",
        request.job_name,
        request.engine.source_ext()
    ));
    fsutil::write_file(&staged, request.source_text.as_bytes())?;
    Ok(staged)
}
