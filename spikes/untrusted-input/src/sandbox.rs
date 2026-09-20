//! 所有拒绝证据都有正常运行前提；不把启动失败当作隔离成功。
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::world::SpikeWorld;

const SECRET: &str = "SCHOLIUMPRIVATECANARY";
const TIMEOUT_SECONDS: &str = "10";

struct Fixture {
    root: PathBuf,
    input: PathBuf,
    output: PathBuf,
}

impl Fixture {
    fn new() -> io::Result<Self> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?;
        let root = std::env::temp_dir().join(format!(
            "scholium-security-{}-{}",
            std::process::id(),
            stamp.as_nanos()
        ));
        let input = root.join("input");
        let output = root.join("output");
        fs::create_dir_all(&input)?;
        fs::create_dir(&output)?;
        fs::write(root.join("private.txt"), SECRET)?;
        fs::write(input.join("allowed.txt"), "PROJECTCONTROL")?;
        std::os::unix::fs::symlink(root.join("private.txt"), input.join("escape.txt"))?;
        Ok(Self {
            root,
            input,
            output,
        })
    }

    fn run(&self, seconds: &str, args: &[&str]) -> io::Result<Output> {
        Command::new("/usr/bin/bash")
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../toolchain-sandbox.sh"))
            .args([self.input.as_os_str(), self.output.as_os_str()])
            .arg(seconds)
            .args(args)
            .output()
    }

    fn latex(&self, name: &str, body: &str) -> io::Result<Output> {
        fs::write(self.input.join(format!("{name}.tex")), document(body))?;
        let output = self.run(
            TIMEOUT_SECONDS,
            &[
                "/usr/bin/xelatex",
                "-no-shell-escape",
                "-interaction=nonstopmode",
                "-halt-on-error",
                "-output-directory=/work",
                &format!("/project/{name}.tex"),
            ],
        )?;
        fs::write(
            self.output.join(format!("{name}.process.log")),
            log(&output),
        )?;
        Ok(output)
    }
}

fn document(body: &str) -> String {
    format!(r"\documentclass{{article}}\begin{{document}}{body}\end{{document}}")
}

fn log(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn check(name: &str, ok: bool) -> io::Result<()> {
    if !ok {
        return Err(io::Error::other(name.to_owned()));
    }
    println!("[PASS] {name}");
    Ok(())
}

pub(crate) fn verify() -> io::Result<()> {
    let fixture = Fixture::new()?;
    println!("证据目录：{}", fixture.root.display());
    benign(&fixture)?;
    read_controls(&fixture)?;
    shell_controls(&fixture)?;
    write_controls(&fixture)?;
    resource_controls(&fixture)?;
    typst_controls()?;
    println!("全部安全夹具通过（Linux spike profile；不是生产沙箱认证）");
    Ok(())
}

fn benign(f: &Fixture) -> io::Result<()> {
    let result = f.latex("benign", "BENIGNCONTROL")?;
    check("沙箱内良性文档编译成功", result.status.success())?;
    let pdf = fs::read(f.output.join("benign.pdf"))?;
    check("本次生成真实 PDF", pdf.starts_with(b"%PDF-"))?;
    let text = Command::new("pdftotext")
        .arg(f.output.join("benign.pdf"))
        .arg("-")
        .output()?;
    check(
        "独立 PDF 读回包含正文",
        text.status.success() && log(&text).contains("BENIGNCONTROL"),
    )
}

fn read_body(path: &str) -> String {
    // EOF guard lets the denied case finish normally, proving TeX reached the probe.
    format!(
        r"\newread\f\openin\f={path} \ifeof\f\message{{READDENIED}}\else\read\f to \line\message{{READVALUE:\line}}\fi\closein\f PROBEFINISHED"
    )
}

fn read_controls(f: &Fixture) -> io::Result<()> {
    let private = f.root.join("private.txt");
    let control_dir = f.root.join("control");
    fs::create_dir(&control_dir)?;
    fs::write(
        control_dir.join("control.tex"),
        document(&read_body(&private.to_string_lossy())),
    )?;
    let control = Command::new("/usr/bin/timeout")
        .args([
            "10s",
            "/usr/bin/xelatex",
            "-no-shell-escape",
            "-interaction=nonstopmode",
            "-halt-on-error",
            "control.tex",
        ])
        .current_dir(&control_dir)
        .output()?;
    check(
        "无沙箱对照确实读到合成诱饵",
        control.status.success() && log(&control).contains(SECRET),
    )?;
    for (name, path) in [
        ("absolute", private.to_string_lossy().into_owned()),
        ("parent", "../private.txt".to_owned()),
        ("symlink", "/project/escape.txt".to_owned()),
    ] {
        let result = f.latex(name, &read_body(&path))?;
        check(
            &format!("{name} 越界读取被拒且探针正常结束"),
            result.status.success()
                && log(&result).contains("READDENIED")
                && !log(&result).contains(SECRET),
        )?;
    }
    let allowed = f.latex("allowed", &read_body("/project/allowed.txt"))?;
    check(
        "声明的项目文件可读",
        allowed.status.success() && log(&allowed).contains("PROJECTCONTROL"),
    )
}

fn shell_controls(f: &Fixture) -> io::Result<()> {
    let control_dir = f.root.join("shell-control");
    fs::create_dir(&control_dir)?;
    fs::write(
        control_dir.join("shell-control.tex"),
        document(r"\immediate\write18{touch shell-control.marker}CONTROL"),
    )?;
    // Trusted, synthetic negative control; arbitrary user input never uses this entry.
    let control = Command::new("/usr/bin/timeout")
        .args([
            "10s",
            "/usr/bin/xelatex",
            "-shell-escape",
            "-interaction=nonstopmode",
            "-halt-on-error",
            "shell-control.tex",
        ])
        .current_dir(&control_dir)
        .output()?;
    check(
        "启用 shell 的对照产生标记",
        control.status.success() && control_dir.join("shell-control.marker").exists(),
    )?;
    let result = f.latex(
        "shell-denied",
        r"\immediate\write18{touch forbidden.marker}SHELLPROBE",
    )?;
    check(
        "沙箱编译关闭 shell escape",
        result.status.success() && !f.output.join("forbidden.marker").exists(),
    )
}

fn write_controls(f: &Fixture) -> io::Result<()> {
    let script = "printf OK > /work/write-control; test -r /project/allowed.txt; test ! -e /project/escape.txt; test ! -e /etc/hostname";
    check(
        "沙箱进程能写输出且看不到私人文件",
        f.run(TIMEOUT_SECONDS, &["/bin/sh", "-ec", script])?
            .status
            .success(),
    )?;
    let denied = f.run(
        TIMEOUT_SECONDS,
        &["/bin/sh", "-c", "printf BAD > /project/allowed.txt"],
    )?;
    check(
        "输入挂载只读",
        !denied.status.success()
            && fs::read_to_string(f.input.join("allowed.txt"))? == "PROJECTCONTROL",
    )?;
    let target = f.root.join("outside-write");
    let denied = f.run(
        TIMEOUT_SECONDS,
        &["/usr/bin/touch", &target.to_string_lossy()],
    )?;
    check(
        "越界写入不影响宿主",
        !denied.status.success() && !target.exists(),
    )?;
    let host_net = fs::read_link("/proc/self/ns/net")?;
    let sandbox_net = f.run(TIMEOUT_SECONDS, &["/usr/bin/readlink", "/proc/self/ns/net"])?;
    check(
        "网络 namespace 与宿主隔离",
        sandbox_net.status.success() && log(&sandbox_net).trim() != host_net.to_string_lossy(),
    )
}

fn resource_controls(f: &Fixture) -> io::Result<()> {
    let timed = f.run(
        "1",
        &["/bin/sh", "-c", "sleep 3; touch /work/escaped-timeout"],
    )?;
    check("墙钟超时返回明确状态", timed.status.code() == Some(124))?;
    std::thread::sleep(std::time::Duration::from_secs(3));
    check(
        "超时后子进程不能继续写入",
        !f.output.join("escaped-timeout").exists(),
    )?;
    let limited = f.run(
        TIMEOUT_SECONDS,
        &[
            "/usr/bin/dd",
            "if=/dev/zero",
            "of=/work/oversized",
            "bs=1M",
            "count=65",
        ],
    )?;
    check(
        "单文件输出限制为 64 MiB",
        !limited.status.success()
            && fs::metadata(f.output.join("oversized"))?.len() <= 64 * 1024 * 1024,
    )?;
    fs::remove_file(f.output.join("oversized"))?;
    let limits = f.run(TIMEOUT_SECONDS, &["/usr/bin/cat", "/proc/self/limits"])?;
    fs::write(f.output.join("limits.txt"), log(&limits))?;
    check("资源限制探针启动成功", limits.status.success())
}

fn typst_controls() -> io::Result<()> {
    let benign = SpikeWorld::new("TYPSTCONTROL".to_owned());
    check(
        "Typst 良性文档可编译",
        typst::compile::<typst_layout::PagedDocument>(&benign)
            .output
            .is_ok(),
    )?;
    for source in ["#read(\"/etc/hostname\")", "#plugin(\"/bin/sh\")"] {
        let world = SpikeWorld::new(source.to_owned());
        let result = typst::compile::<typst_layout::PagedDocument>(&world).output;
        let denied = result.as_ref().err().is_some_and(|errors| {
            errors
                .iter()
                .any(|error| error.message.contains("file not found"))
        });
        check(&format!("Typst 未声明文件访问被拒：{source}"), denied)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn sandbox_compiles_benign_document_to_pdf() -> std::io::Result<()> {
        let fixture = super::Fixture::new()?;
        super::benign(&fixture)
    }
}
