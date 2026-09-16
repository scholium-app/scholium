//! 沙箱分发：把源码复制进引擎独占的输出目录。
//!
//! 产品代码不会这么干（产品直接写用户的项目目录，靠 `-outdir` 与 jobname 隔离）。
//! spike 这么做是为了让"中间文件是否泄漏"这件事可以被**枚举目录**直接断言，
//! 而不是靠解析引擎日志猜测它写了哪些文件。

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
