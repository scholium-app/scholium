//! 子进程封装：运行外部工具、捕获输出、判断是否被信号杀死。
//!
//! 编译工具与 PDF 探针只由隔离 worker 调用；崩溃夹具另有继承输出的启动路径。

use std::path::Path;
use std::process::Command;

use crate::error::{Result, SpikeError};

/// 捕获到的子进程结果。
#[derive(Debug, Clone)]
pub struct ProcessOutput {
    /// 退出码；被信号杀死时为 `None`。
    pub code: Option<i32>,
    /// 终止信号；正常退出时为 `None`。
    pub signal: Option<i32>,
    /// 标准输出。
    pub stdout: String,
    /// 标准错误。
    pub stderr: String,
}

impl ProcessOutput {
    /// 是否正常退出且退出码为 0。
    pub fn succeeded(&self) -> bool {
        self.code == Some(0)
    }

    /// 是否被 `SIGKILL` 杀死。Linux 上 `wait` 上报的信号号是 9。
    pub fn killed_by_sigkill(&self) -> bool {
        self.signal == Some(libc::SIGKILL)
    }

    /// 一段可进报告的简短摘要（stdout+stderr 末尾若干行）。
    pub fn summary(&self, max_lines: usize) -> String {
        let mut lines: Vec<&str> = self
            .stdout
            .lines()
            .chain(self.stderr.lines())
            .filter(|line| !line.trim().is_empty())
            .collect();
        if lines.len() > max_lines {
            lines = lines.split_off(lines.len() - max_lines);
        }
        lines.join(" | ")
    }
}

/// 在 `cwd` 下运行程序，捕获 stdout/stderr。
///
/// # Errors
///
/// 程序无法启动（不存在、无执行权限）。
pub fn run(program: &str, args: &[String], cwd: &Path) -> Result<ProcessOutput> {
    let output = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|source| SpikeError::Spawn {
            program: program.to_string(),
            source,
        })?;

    #[cfg(unix)]
    let (code, signal) = {
        use std::os::unix::process::ExitStatusExt;
        (output.status.code(), output.status.signal())
    };
    #[cfg(not(unix))]
    let (code, signal) = (output.status.code(), None);

    Ok(ProcessOutput {
        code,
        signal,
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

/// 运行程序，非零退出码即错误。
///
/// # Errors
///
/// 启动失败，或退出码非 0（被信号杀死时 `code` 为 `None`）。
pub fn run_checked(program: &str, args: &[String], cwd: &Path) -> Result<ProcessOutput> {
    let output = run(program, args, cwd)?;
    if !output.succeeded() {
        return Err(SpikeError::CommandFailed {
            program: program.to_string(),
            code: output.code,
            stderr: output.summary(12),
        });
    }
    Ok(output)
}

/// 启动子进程并等待其结束，`stdout`/`stderr` 直接继承父进程。
///
/// 与 [`run`] 的区别：这里不捕获输出。崩溃夹具会打印少量证据然后 `SIGKILL` 自己，
/// 捕获输出反而会丢掉"进程被信号杀死"的现场顺序；让它直接写到同一个终端更可信。
///
/// 返回 [`ProcessOutput`]，其中 `stdout`/`stderr` 为空，只有退出状态有意义。
///
/// # Errors
///
/// 程序无法启动，或等待失败。
pub fn spawn_inherit(program: &str, args: &[String], cwd: &Path) -> Result<ProcessOutput> {
    let mut child = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .spawn()
        .map_err(|source| SpikeError::Spawn {
            program: program.to_string(),
            source,
        })?;
    let status = child.wait().map_err(|source| SpikeError::Spawn {
        program: program.to_string(),
        source,
    })?;

    #[cfg(unix)]
    let (code, signal) = {
        use std::os::unix::process::ExitStatusExt;
        (status.code(), status.signal())
    };
    #[cfg(not(unix))]
    let (code, signal) = (status.code(), None);

    Ok(ProcessOutput {
        code,
        signal,
        stdout: String::new(),
        stderr: String::new(),
    })
}

/// 取程序版本行的可读形式：优先第一行，没有则取 stderr 第一行，都没有则 `"未知"`。
///
/// 版本要进报告，所以必须是**程序自己报的版本**，不能由我们拼一个看起来像版本的字符串。
pub fn version_line(program: &str, args: &[&str]) -> String {
    let owned: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
    let output = match run(program, &owned, Path::new(".")) {
        Ok(output) => output,
        Err(error) => return format!("{program} 不可用 ({error})"),
    };
    let source = if output.stdout.trim().is_empty() {
        &output.stderr
    } else {
        &output.stdout
    };
    source
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("未知")
        .to_string()
}

/// 从 `pdfinfo` 输出里解析页数。
pub fn parse_pdf_pages(text: &str) -> Option<usize> {
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("Pages:") {
            return rest.trim().parse::<usize>().ok();
        }
    }
    None
}
