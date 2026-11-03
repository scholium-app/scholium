//! Native regression for upstream's linked project snapshot across /project and /work mounts.
use scholium_build::{ResourceLimits, RtexToolchain, Sandbox};
use scholium_latex::{CompileOutcome, LiveConfig, LiveEvent, LiveSession, SourceSession};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::process::Command;

#[tokio::test]
#[ignore = "requires pinned rtex, LuaLaTeX, bwrap and systemd user cgroups"]
async fn native_snapshot_links_resolve_but_cannot_write_back_to_readonly_project() {
    let sandbox = sandbox().await;
    let project = tempfile::tempdir().expect("project");
    let work = tempfile::tempdir().expect("build");
    resources(project.path());
    std::fs::write(project.path().join("main.tex"), "\\documentclass{article}\n\\begin{document}\nOrdinary paragraph for a snapshot with linked resources.\n\\end{document}\n").expect("source");
    let source = SourceSession::open(project.path(), "main.tex").expect("source session");
    let mut live = LiveSession::start(source, sandbox.clone(), work.path(), LiveConfig::default())
        .await
        .expect("live");
    first_layout(&mut live).await;
    let snapshot = work.path().join("build/src");
    assert_eq!(
        std::fs::read_link(snapshot.join("plain.txt")).expect("upstream linked file"),
        Path::new("/project/plain.txt")
    );
    assert_eq!(
        std::fs::read_link(snapshot.join("assets")).expect("upstream linked directory"),
        Path::new("/project/assets")
    );
    live.cancel()
        .await
        .expect("cancel live before isolated probe");

    host_positive_control().await;
    std::fs::write(
        project.path().join("probe.tex"),
        probe("/work/build/src", false),
    )
    .expect("sandbox probe");
    let output = sandbox
        .run(
            project.path(),
            work.path(),
            &[
                "/usr/bin/lualatex".into(),
                "-interaction=nonstopmode".into(),
                "-halt-on-error".into(),
                "-output-directory=/work".into(),
                "/project/probe.tex".into(),
            ],
            Duration::from_secs(30),
        )
        .await
        .expect("read must work and write must fail through real upstream snapshot links");
    assert!(String::from_utf8_lossy(&output).contains("T38-LINK-PROBE-PASS"));
    for resource in ["plain.txt", "assets/protected.txt"] {
        assert_eq!(
            std::fs::read_to_string(project.path().join(resource)).expect("readonly original"),
            "protected"
        );
    }
    eprintln!(
        "product sandbox: upstream file/dir links resolved across mounts; both writebacks denied; project bytes unchanged"
    );
}
async fn sandbox() -> Arc<Sandbox> {
    let required =
        |key| PathBuf::from(std::env::var_os(key).unwrap_or_else(|| panic!("未运行：缺少 {key}")));
    let toolchain = RtexToolchain {
        binary: required("SCHOLIUM_RTEX_BIN"),
        tex_directory: required("SCHOLIUM_RTEX_TEXDIR"),
        font_cache: required("SCHOLIUM_RTEX_FONT_CACHE"),
    };
    Arc::new(
        Sandbox::prepare(toolchain, ResourceLimits::default())
            .await
            .unwrap_or_else(|e| panic!("未运行：沙箱探测失败：{e}")),
    )
}

async fn first_layout(live: &mut LiveSession) {
    tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            let event = live.next_event().await.expect("event");
            if matches!(
                event,
                LiveEvent::Pages {
                    outcome: CompileOutcome::Complete,
                    ..
                }
            ) {
                break;
            }
        }
    })
    .await
    .expect("initial layout");
}

async fn host_positive_control() {
    // The same read/write attack must succeed outside isolation. Mirror both actual upstream link
    // forms with host-resolvable absolute paths, then run the same fixed Lua probe against them.
    let host_project = tempfile::tempdir().expect("host project");
    let host_work = tempfile::tempdir().expect("host links");
    resources(host_project.path());
    std::os::unix::fs::symlink(
        host_project.path().join("plain.txt"),
        host_work.path().join("plain.txt"),
    )
    .expect("host file link");
    std::os::unix::fs::symlink(
        host_project.path().join("assets"),
        host_work.path().join("assets"),
    )
    .expect("host dir link");
    let host_probe = host_work.path().join("probe.tex");
    std::fs::write(
        &host_probe,
        probe(&host_work.path().to_string_lossy(), true),
    )
    .expect("host probe");
    let output = Command::new("/usr/bin/lualatex")
        .args([
            "-interaction=nonstopmode",
            "-halt-on-error",
            "-output-directory",
        ])
        .arg(host_work.path())
        .arg(host_probe)
        .output()
        .await
        .expect("host positive control");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    for resource in ["plain.txt", "assets/protected.txt"] {
        assert_eq!(
            std::fs::read_to_string(host_project.path().join(resource)).expect("host writeback"),
            "escaped"
        );
    }
    eprintln!("host: both symbolic-link forms resolved and wrote back to project");
}

fn resources(project: &Path) {
    std::fs::create_dir(project.join("assets")).expect("assets");
    for resource in ["plain.txt", "assets/protected.txt"] {
        std::fs::write(project.join(resource), "protected").expect("resource");
    }
}
fn probe(root: &str, writable: bool) -> String {
    let expected = if writable { "true" } else { "false" };
    format!(
        "\\documentclass{{article}}\n\\begin{{document}}Probe.\n\\directlua{{for _,name in ipairs({{'plain.txt','assets/protected.txt'}}) do local p='{root}/'..name local f=assert(io.open(p,'r')) assert(f:read('a')=='protected') f:close() local w=io.open(p,'w') assert((w and true or false)=={expected}) if w then w:write('escaped') w:close() end end texio.write_nl('T38-LINK-PROBE-PASS')}}\n\\end{{document}}\n"
    )
}
