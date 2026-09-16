//! 极简参数解析。
//!
//! 不引入 clap：spike 的命令行只有 `--key value` 与位置子命令两种形态，
//! 一个 20 行的解析器足够了。解析器同时被主进程和崩溃子进程使用。

/// 取 `--key value` 形态的参数值；不存在返回 `None`。
pub fn arg_value(args: &[String], key: &str) -> Option<String> {
    let position = args.iter().position(|arg| arg == key)?;
    args.get(position + 1).cloned()
}

/// 已知子命令。只有 `writer` 一个：它是崩溃夹具子进程的入口。
pub const SUBCOMMANDS: &[&str] = &["writer"];

/// 取第一个参数作为子命令，且必须落在 [`SUBCOMMANDS`] 里。
///
/// 不把任意首个非 `--` 参数当子命令：`cargo run` 会把二进制路径放在 `argv[0]`，
/// 若只判断"非 `--` 开头"，`cargo run --release` 会被误判成子命令
/// `target/release/scholium-spike-recovery`。
pub fn subcommand(args: &[String]) -> Option<String> {
    let candidate = args.get(1)?;
    SUBCOMMANDS
        .contains(&candidate.as_str())
        .then(|| candidate.clone())
}
