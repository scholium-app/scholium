//! Spike 的错误类型。
//!
//! 按 `AGENT.md` 的 Rust 规范，错误用 `thiserror` 而不是裸 `String`；但这是可执行的验证夹具，
//! 失败路径只服务于断言，因此变体粒度按"证据要区分什么"划分，不按产品错误码划分。

use std::path::PathBuf;

/// Spike 运行期间的错误。
#[derive(Debug, thiserror::Error)]
pub enum SpikeError {
    /// 文件系统操作失败。
    #[error("文件操作失败 {path}: {source}")]
    Io {
        /// 出错的路径。
        path: PathBuf,
        /// 底层错误。
        source: std::io::Error,
    },

    /// 子进程无法启动或等待失败。
    #[error("子进程失败 {program}: {source}")]
    Spawn {
        /// 程序路径。
        program: String,
        /// 底层错误。
        source: std::io::Error,
    },

    /// 外部工具返回非零退出码。
    #[error("外部命令 {program} 退出码 {code:?}，stderr: {stderr}")]
    CommandFailed {
        /// 程序名。
        program: String,
        /// 退出码，被信号终止时为 `None`。
        code: Option<i32>,
        /// 捕获的 stderr（截断后）。
        stderr: String,
    },

    /// 文档核心拒绝了某个编辑。
    #[error("文档核心拒绝操作: {0}")]
    Core(String),
}

/// 文件操作的 `Result` 别名。
pub type Result<T> = std::result::Result<T, SpikeError>;

/// 给 `io::Result` 补上路径上下文。
///
/// 参数 `path` 只用于错误消息，不参与任何判断。
pub fn io_context(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> SpikeError {
    let path = path.into();
    move |source| SpikeError::Io {
        path: path.clone(),
        source,
    }
}
