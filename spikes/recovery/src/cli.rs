//! 极简参数解析。
//!
//! 不引入 clap：spike 的命令行只有 `--key value` 与位置子命令两种形态，
//! 一个 20 行的解析器足够了。解析器同时被主进程和崩溃子进程使用。

/// 取 `--key value` 形态的参数值；不存在返回 `None`。
pub fn arg_value(args: &[String], key: &str) -> Option<String> {
    let position = args.iter().position(|arg| arg == key)?;
    args.get(position + 1).cloned()
}

/// 取第一个不以 `--` 开头的参数作为子命令。
pub fn subcommand(args: &[String]) -> Option<String> {
    args.iter()
        .find(|arg| !arg.starts_with("--") && arg.as_str() != "run")
        .cloned()
}

/// 是否带开关参数。
pub fn has_flag(args: &[String], key: &str) -> bool {
    args.iter().any(|arg| arg == key)
}
