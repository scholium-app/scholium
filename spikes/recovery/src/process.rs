//! 子进程封装：运行外部工具、捕获输出、判断是否被信号杀死。
//!
//! 两种构建路径都要用（LaTeX 走 TeX Live 子进程，Typst 走进程内 crate 但页数用外部
//! `pdfinfo` 读），崩溃夹具也要用（它自己就是被启动的子进程），所以这里统一。

use std::path::Path;
use std::process::Command;

use crate::error::{Result, SpikeError};

/// 捕获到的子进程结果。
#[derive(Debug, Clone)]
pub struct ProcessOutput {
    /// 程序名（用于错误消息）。
    pub program: String,
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
        program: program.to_string(),
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

/// 从 `pdfinfo` 输出里解析页数。
pub fn parse_pdf_pages(text: &str) -> Option<usize> {
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("Pages:") {
            return rest.trim().parse::<usize>().ok();
        }
    }
    None
}
