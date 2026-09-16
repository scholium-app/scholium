//! Linux 不受信源码验证：正常编译、负对照、路径隔离与资源护栏。
mod sandbox;
mod world;

use std::process::ExitCode;

fn main() -> ExitCode {
    let result = sandbox::verify();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("[FAIL] {error}");
            ExitCode::FAILURE
        }
    }
}
