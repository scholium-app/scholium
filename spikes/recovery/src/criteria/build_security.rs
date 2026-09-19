//! 通过公开 build 路径验证良性、越界、命令注入与墙钟预算。
use crate::{
    build::{self, BuildRequest, Engine},
    check::Checks,
    error::Result,
    fsutil,
};
use std::{path::Path, time::Instant};

fn request(root: &Path, engine: Engine, source: &str) -> BuildRequest {
    let out_dir = root.join(engine.slug());
    BuildRequest {
        engine,
        source_path: out_dir.join(format!("main.{}", engine.source_ext())),
        out_dir,
        job_name: "main".into(),
        source_text: source.into(),
        source_hash: fsutil::sha256_hex(source.as_bytes()),
    }
}

fn latex(body: &str) -> String {
    format!("\\documentclass{{article}}\n\\begin{{document}}\n{body}\n\\end{{document}}\n")
}

/// 执行真实编译入口的安全正负对照，任意失败使命令失败。
pub fn run() -> Result<()> {
    let scratch = fsutil::Scratch::create(&std::env::temp_dir(), "recovery-security")?;
    let mut checks = Checks::new();
    for (engine, source) in [
        (Engine::Latex, latex("CONTROL")),
        (Engine::Typst, "CONTROL".into()),
    ] {
        checks.case(&format!("security.{}.positive", engine.slug()));
        let result = build::build(&request(scratch.root(), engine, &source))?;
        checks.expect(
            result.pages == Some(1) && result.product_signature.is_some(),
            "良性输入真实编译并提取签名",
            &result.engine_note,
        );
    }
    outside_reads(&mut checks, scratch.root())?;
    command_execution(&mut checks, scratch.root())?;
    timeouts(&mut checks, scratch.root())?;
    if checks.finish() {
        Ok(())
    } else {
        Err(crate::error::SpikeError::Core(
            "security checks failed".into(),
        ))
    }
}

fn outside_reads(checks: &mut Checks, root: &Path) -> Result<()> {
    let secret = root.join("private.tex");
    fsutil::write_file(&secret, b"PRIVATE-RECOVERY-SENTINEL")?;
    // A successful compile with EOF is stronger evidence than a generic compile error.
    let probe = latex(&format!(
        "\\newread\\probe\\openin\\probe={} \\ifeof\\probe BLOCKED\\else\\errmessage{{LEAKED}}\\fi",
        secret.display()
    ));
    checks.case("security.latex.outside-read");
    let result = build::build(&request(root, Engine::Latex, &probe))?;
    checks.expect(
        result.pages == Some(1),
        "根外文件不可见且引擎正常工作",
        "EOF branch compiled",
    );
    checks.case("security.typst.outside-read");
    let req = request(
        root,
        Engine::Typst,
        &format!("#read(\"{}\")", secret.display()),
    );
    let result = build::build(&req);
    let log = fsutil::read_file(&req.out_dir.join("worker-stderr.txt"))?;
    checks.expect(
        result.is_err() && String::from_utf8_lossy(&log).contains("file not found"),
        "Typst 明确拒绝文件读取",
        &String::from_utf8_lossy(&log),
    );
    Ok(())
}

fn command_execution(checks: &mut Checks, root: &Path) -> Result<()> {
    checks.case("security.latex.commands");
    let req = request(
        root,
        Engine::Latex,
        &latex("\\immediate\\write18{touch /work/SHELL-LEAK} CONTROL"),
    );
    fsutil::write_file(
        &req.out_dir.join("latexmkrc"),
        b"system('touch /work/RC-LEAK');\n",
    )?;
    let result = build::build(&req)?;
    checks.expect(
        result.pages == Some(1)
            && !req.out_dir.join("SHELL-LEAK").exists()
            && !req.out_dir.join("RC-LEAK").exists(),
        "shell escape 和项目 latexmkrc 均禁用",
        "successful compile; shell/config markers absent",
    );
    Ok(())
}

fn timeouts(checks: &mut Checks, root: &Path) -> Result<()> {
    for (engine, source) in [
        (Engine::Latex, latex("\\loop\\iftrue\\repeat")),
        (Engine::Typst, "#let fib(n, seed) = if n < 2 { seed } else { fib(n - 1, seed * 2) + fib(n - 2, seed * 2 + 1) }\n#fib(40, 1)".into()),
    ] {
        checks.case(&format!("security.{}.timeout", engine.slug()));
        let start = Instant::now();
        let req = request(root, engine, &source);
        let result = build::build(&req);
        let timed_out = matches!(
            &result,
            Err(crate::error::SpikeError::CommandFailed {
                code: Some(124),
                ..
            })
        );
        checks.expect(
            timed_out
                && start.elapsed().as_secs() < 20
                && !req
                    .out_dir
                    .join(format!("main.{}", engine.product_ext()))
                    .exists(),
            "循环被墙钟上限终止且无旧产物",
            &format!("{result:?}; elapsed={:?}", start.elapsed()),
        );
    }
    Ok(())
}
