use crate::{ResourceLimits, RtexToolchain, Sandbox};
use serde_json::{Value, json};
use std::{
    net::TcpListener,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::Command,
};

fn toolchain() -> RtexToolchain {
    let required =
        |key| PathBuf::from(std::env::var_os(key).unwrap_or_else(|| panic!("未运行：缺少 {key}")));
    RtexToolchain {
        binary: required("SCHOLIUM_RTEX_BIN"),
        tex_directory: required("SCHOLIUM_RTEX_TEXDIR"),
        font_cache: required("SCHOLIUM_RTEX_FONT_CACHE"),
    }
}

#[tokio::test]
#[ignore = "native E2 controls require pinned rtex, LuaLaTeX, bwrap and systemd user cgroups"]
async fn native_rtex_e2_attacks_work_on_host_and_are_blocked_by_product_sandbox() {
    let toolchain = toolchain();
    let mut sandbox = Sandbox::prepare(toolchain.clone(), ResourceLimits::default())
        .await
        .unwrap_or_else(|e| panic!("未运行：沙箱探测失败：{e}"));
    let support = tempfile::tempdir().expect("trusted helper");
    let lifted = support.path().join("lualatex");
    let status = Command::new("rustc")
        .args(["--edition=2024", "-O", "-o"])
        .arg(&lifted)
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/lift_lualatex.rs"
        ))
        .status()
        .await
        .expect("rustc helper");
    assert!(status.success());
    // Test-only mutation amplifies c/e past TeX's own policy. launch() and every other mount,
    // cgroup and namespace remain the product implementation; no public bypass is added.
    sandbox.libraries(&lifted).await.expect("helper libraries");
    sandbox.mounts.insert("/usr/bin/lualatex".into(), lifted);
    let sandbox = Arc::new(sandbox);
    for case in [
        "read",
        "absolute-input",
        "shell",
        "write-lua",
        "write-openout",
        "write-project",
        "network",
    ] {
        let canary = tempfile::tempdir().expect("outside canary");
        let secret = format!(
            "T37SECRET{}",
            canary
                .path()
                .file_name()
                .expect("random path")
                .to_string_lossy()
                .replace('.', "")
        );
        std::fs::write(canary.path().join("secret.txt"), &secret).expect("secret");
        std::fs::write(
            canary.path().join("secret.tex"),
            format!("\\typeout{{{secret}}}\n"),
        )
        .expect("input");
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        listener
            .set_nonblocking(true)
            .expect("nonblocking listener");
        let port = listener.local_addr().expect("port").port();
        let source = source(case, canary.path(), port);
        let mut verdicts = Vec::new();
        for isolated in [false, true] {
            let project = tempfile::tempdir().expect("project");
            let work = tempfile::tempdir().expect("work");
            std::fs::write(
                project.path().join("main.tex"),
                source.replace("@PROJECT@", &project.path().to_string_lossy()),
            )
            .expect("source");
            let trace = drive(
                &toolchain,
                &sandbox,
                support.path(),
                project.path(),
                work.path(),
                isolated,
            )
            .await;
            let leaked = trace.contains(&secret) || contains(work.path(), secret.as_bytes());
            let written = canary.path().join("written.tex").is_file()
                || project.path().join("written.tex").is_file();
            let mut connected = false;
            while listener.accept().is_ok() {
                connected = true;
            }
            let effective = leaked || written || connected;
            eprintln!(
                "{}",
                json!({"case":case,"sandboxed":isolated,"leaked":leaked,"written":written,"connected":connected,"attack_effective":effective})
            );
            verdicts.push(effective);
            if canary.path().join("written.tex").is_file() {
                std::fs::remove_file(canary.path().join("written.tex"))
                    .expect("reset write canary");
            }
        }
        assert_eq!(
            verdicts,
            [true, false],
            "E2 {case}: host must prove effective before accepting sandbox rejection"
        );
    }
    assert!(
        !toolchain.font_cache.join("ready/poison").exists(),
        "user documents must not mutate persistent font cache"
    );
}

fn source(case: &str, canary: &Path, port: u16) -> String {
    let dir = canary.to_string_lossy();
    let body = match case {
        "read" => format!(
            "\\directlua{{local f=io.open('{dir}/secret.txt','r') if f then texio.write_nl(f:read('a')) f:close() end}}"
        ),
        "absolute-input" => format!("\\input{{{dir}/secret.tex}}"),
        "shell" => format!("\\directlua{{os.execute('touch {dir}/written.tex')}}"),
        "write-lua" => format!(
            "\\directlua{{local f=io.open('{dir}/written.tex','w') if f then f:write('escaped') f:close() end}}"
        ),
        "write-openout" => format!(
            "\\immediate\\openout\\probe={dir}/written.tex\n\\immediate\\write\\probe{{escaped}}\\immediate\\closeout\\probe"
        ),
        "write-project" => "\\directlua{for _,p in ipairs({'@PROJECT@/written.tex','/project/written.tex'}) do local f=io.open(p,'w') if f then f:write('escaped') f:close() end end}".into(),
        "network" => format!(
            "\\directlua{{local ok,s=pcall(require,'socket') if ok then local c=s.tcp() c:settimeout(1) c:connect('127.0.0.1',{port}) c:close() end}}"
        ),
        _ => unreachable!("fixed test case list"),
    };
    format!(
        "\\documentclass{{article}}\n\\newwrite\\probe\n\\begin{{document}}\nProbe ordinary paragraph.\n{body}\n\\directlua{{local f=io.open('/rtex/texmf-var/poison','w') if f then f:write('session only') f:close() end}}\n\\end{{document}}\n"
    )
}

async fn drive(
    toolchain: &RtexToolchain,
    sandbox: &Sandbox,
    texbin: &Path,
    project: &Path,
    work: &Path,
    isolated: bool,
) -> String {
    let mut process = if isolated {
        Some(
            sandbox
                .launch(
                    project,
                    work,
                    &[
                        "/rtex/bin/rtex".into(),
                        "serve".into(),
                        "--project".into(),
                        "/project".into(),
                        "--build".into(),
                        "/work/build".into(),
                    ],
                )
                .expect("product launch"),
        )
    } else {
        None
    };
    let host_cache = tempfile::tempdir().expect("host cache copy");
    crate::cache::copy(&toolchain.font_cache.join("ready"), host_cache.path())
        .expect("host cache copy");
    let mut host = if isolated {
        None
    } else {
        let mut command = Command::new(&toolchain.binary);
        command
            .args(["serve", "--project"])
            .arg(project)
            .arg("--build")
            .arg(work.join("build"))
            .env("RTEX_TEXDIR", &toolchain.tex_directory)
            .env("RTEX_TEXLIVE_BIN", texbin)
            .env("TEXMFVAR", host_cache.path())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        Some(command.spawn().expect("host positive control"))
    };
    let (mut input, output, error) = if let Some(child) = process.as_mut() {
        (
            child.take_stdin().expect("stdin"),
            child.take_stdout().expect("stdout"),
            child.take_stderr().expect("stderr"),
        )
    } else {
        let child = host.as_mut().expect("host");
        (
            child.stdin.take().expect("stdin"),
            child.stdout.take().expect("stdout"),
            child.stderr.take().expect("stderr"),
        )
    };
    if let Some(child) = process.as_ref() {
        verify_resources(child.id().expect("supervisor pid"), false);
    }
    let stderr = tokio::spawn(async move {
        let mut bytes = Vec::new();
        error
            .take(1024 * 1024)
            .read_to_end(&mut bytes)
            .await
            .expect("bounded stderr");
        String::from_utf8_lossy(&bytes).into_owned()
    });
    let mut reader = BufReader::new(output);
    let trace = read_layout(&mut reader).await;
    if let Some(child) = process.as_ref() {
        verify_resources(child.id().expect("supervisor pid"), true);
    }
    input
        .write_all(b"{\"cmd\":\"quit\"}\n")
        .await
        .expect("quit");
    if let Some(child) = process.as_mut() {
        child.cancel().await.expect("cancel cgroup");
    }
    if let Some(child) = host.as_mut() {
        tokio::time::timeout(Duration::from_secs(5), child.wait())
            .await
            .expect("host cleanup deadline")
            .expect("host exit");
    }
    trace + &stderr.await.expect("stderr reader")
}

async fn read_layout(reader: &mut BufReader<tokio::process::ChildStdout>) -> String {
    tokio::time::timeout(Duration::from_secs(60), async {
        let mut trace = String::new();
        loop {
            let mut line = String::new();
            assert_ne!(
                reader.read_line(&mut line).await.expect("protocol line"),
                0,
                "serve exited before layout: {trace}"
            );
            assert!(line.len() <= 16 * 1024 * 1024);
            let message: Value = serde_json::from_str(&line).expect("rtex JSON");
            trace.push_str(&line);
            assert!(trace.len() <= 64 * 1024 * 1024);
            if message["event"] == "LayoutUpdate" {
                break;
            }
        }
        trace
    })
    .await
    .expect("native E2 layout deadline")
}

fn verify_resources(pid: u32, required: bool) {
    let group = std::fs::read_to_string(format!("/proc/{pid}/cgroup")).expect("cgroup membership");
    let path = group.trim().strip_prefix("0::").expect("cgroup v2");
    if !path.contains("/scholium-rtex-") {
        assert!(!required, "supervisor not in its resource scope: {path}");
        return;
    } // Early pre-exec observation; checked again after layout.
    let root = Path::new("/sys/fs/cgroup").join(path.trim_start_matches('/'));
    let memory = std::fs::read_to_string(root.join("memory.max")).expect("memory limit");
    let tasks = std::fs::read_to_string(root.join("pids.max")).expect("task limit");
    let swap = std::fs::read_to_string(root.join("memory.swap.max")).expect("swap limit");
    assert_eq!(memory.trim(), "2147483648");
    assert_eq!(tasks.trim(), "64");
    assert_eq!(swap.trim(), "0");
    let limits = std::fs::read_to_string(format!("/proc/{pid}/limits")).expect("rlimits");
    let cpu = limits
        .lines()
        .find(|line| line.starts_with("Max cpu time"))
        .expect("cpu limit");
    assert!(cpu.contains("unlimited            unlimited"));
    eprintln!(
        "cgroup enforced: memory={}, tasks={}, swap={}; {cpu}",
        memory.trim(),
        tasks.trim(),
        swap.trim()
    );
}

fn contains(root: &Path, needle: &[u8]) -> bool {
    for entry in std::fs::read_dir(root).expect("build outputs") {
        let entry = entry.expect("output entry");
        if entry.file_type().expect("file type").is_dir() {
            if contains(&entry.path(), needle) {
                return true;
            }
        } else if entry.file_type().expect("file type").is_file()
            && let Ok(bytes) = std::fs::read(entry.path())
            && bytes.windows(needle.len()).any(|window| window == needle)
        {
            return true;
        }
    }
    false
}
