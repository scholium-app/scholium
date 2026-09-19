//! 极简参数解析。
//!
//! 不引入 clap：spike 的命令行只有 `--key value` 与位置子命令两种形态，
//! 一个 20 行的解析器足够了。解析器同时被主进程和崩溃子进程使用。

/// 取 `--key value` 形态的参数值；不存在返回 `None`。
pub fn arg_value(args: &[String], key: &str) -> Option<String> {
    let position = args.iter().position(|arg| arg == key)?;
    args.get(position + 1).cloned()
}

/// 从 argv[1] 读取子命令；未知命令交给 main 明确拒绝。
pub fn subcommand(args: &[String]) -> Option<String> {
    args.get(1).cloned()
}
