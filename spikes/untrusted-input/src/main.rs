//! 阶段 0 出口条件验证：**不受信的 LaTeX/Typst 不得执行外部命令，也不得读取项目根之外的文件。**
//!
//! 判据（动手前写定）：
//!
//! | # | 判据 |
//! |---|---|
//! | 1 | 关闭 shell escape 时，`\write18{...}` 不能产生任何副作用 |
//! | 2 | 开启 shell escape 时副作用**确实产生**（证明判据 1 的测试有效，不是"命令本来就没跑"） |
//! | 3 | 限制读取策略下，`\input{/etc/passwd}` 不能把项目外文件读进产物 |
//! | 4 | 不限制时的行为要被**如实记录**（默认配置到底拦不拦） |
//! | 5 | Typst 侧任何读取项目外文件的尝试都失败，且不泄漏内容 |
//! | 6 | 我们的编译入口**从不**主动打开 shell escape |
//!
//! 用法：`cargo run --release`

use std::process::Command;

/// 项目根之外的一个真实文件，用于检验"能不能读出去"。
const OUTSIDE_FILE: &str = "/etc/hostname";
/// shell escape 的副作用目标。
const SIDE_EFFECT: &str = "/tmp/scholium-spike-pwned";

fn main() {
    println!("=== LaTeX 侧 ===");
    check_shell_escape(false);
    check_shell_escape(true);
    check_read_outside(ReadPolicy::Default);
    check_read_outside(ReadPolicy::Restricted);
    println!();
    println!("=== Typst 侧 ===");
    check_typst_read();
    check_typst_plugin();
    println!();
    check_os_sandbox();
}

/// 编译一个 LaTeX 文档，返回 (是否编译成功, 日志)。
fn run_latex(source: &str, shell_escape: bool, restricted_read: bool, dir: &std::path::Path) -> (bool, String) {
    let tex = dir.join("doc.tex");
    std::fs::write(&tex, source).expect("写 tex");
    let mut command = Command::new("xelatex");
    command
        .current_dir(dir)
        .arg("-interaction=nonstopmode")
        .arg("-halt-on-error");
    if shell_escape {
        command.arg("-shell-escape");
    } else {
        command.arg("-no-shell-escape");
    }
    if restricted_read {
        // TeX Live 支持用 -cnf-line 覆盖 texmf.cnf 的 openin_any。
        command.arg("-cnf-line=openin_any=p");
    }
    command.arg("doc.tex");
    let output = command.output().expect("启动 xelatex");
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output.status.success(), log)
}

fn fresh_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("scholium-spike-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("建目录");
    dir
}

fn check_shell_escape(enable: bool) {
    let _ = std::fs::remove_file(SIDE_EFFECT);
    let dir = fresh_dir(if enable { "shell-on" } else { "shell-off" });
    let source = format!(
        "\\documentclass{{article}}\\begin{{document}}\
         \\immediate\\write18{{touch {SIDE_EFFECT}}}done\\end{{document}}"
    );
    let (ok, _log) = run_latex(&source, enable, false, &dir);
    let created = std::path::Path::new(SIDE_EFFECT).exists();
    if enable {
        println!(
            "  [{}] 开启 shell escape：副作用{}（编译{}）",
            if created { "基准" } else { "可疑" },
            if created { "确实产生" } else { "未产生——说明测试无效" },
            if ok { "成功" } else { "失败" }
        );
    } else {
        println!(
            "  [{}] 关闭 shell escape：副作用{}（编译{}）",
            if !created { "PASS" } else { "FAIL" },
            if created { "产生了——存在命令执行" } else { "未产生" },
            if ok { "成功" } else { "失败" }
        );
    }
    let _ = std::fs::remove_file(SIDE_EFFECT);
}

/// 读取策略。
#[derive(Clone, Copy)]
enum ReadPolicy {
    /// TeX Live 默认配置。
    Default,
    /// `openin_any=p`（paranoid：只允许当前目录及子目录）。
    Restricted,
}

fn check_read_outside(policy: ReadPolicy) {
    let outside = std::fs::read_to_string(OUTSIDE_FILE).unwrap_or_default();
    let marker = outside.trim().to_string();
    let dir = fresh_dir(match policy {
        ReadPolicy::Default => "read-default",
        ReadPolicy::Restricted => "read-restricted",
    });
    // 用 \openin + \read 真正把项目外文件读进来，再 \message 到日志：
    // 这是"能不能读出去"的直接检验。\input 会给文件名补 .tex，不适合做这个测试。
    let source = format!(
        "\\documentclass{{article}}\\begin{{document}}         \\newread\\f\\openin\\f={OUTSIDE_FILE}         \\read\\f to \\line\\message{{SCHOLIUM-LEAK:[\\line]}}         \\closein\\f done\\end{{document}}"
    );
    let (ok, log) = run_latex(&source, false, matches!(policy, ReadPolicy::Restricted), &dir);
    let leaked = !marker.is_empty() && log.contains(&marker);
    let label = match policy {
        ReadPolicy::Default => "默认读取策略",
        ReadPolicy::Restricted => "限制读取（openin_any=p）",
    };
    println!(
        "  [{}] {label}：编译{}，读到项目外内容={leaked}（目标内容前 12 字符：{:?}）",
        match policy {
            ReadPolicy::Default => "FAIL(默认配置)",
            ReadPolicy::Restricted => {
                if leaked { "FAIL(该配置无效)" } else { "PASS" }
            }
        },
        if ok { "成功" } else { "失败" },
        marker.chars().take(12).collect::<String>()
    );
    if leaked {
        let line = log
            .lines()
            .find(|line| line.contains("SCHOLIUM-LEAK"))
            .unwrap_or("")
            .trim()
            .to_string();
        println!("        {line}");
    }
}

/// Typst：任何读取项目外文件的尝试都必须失败且不泄漏内容。
fn check_typst_read() {
    let document = scholium_spike_core::Editor::new();
    let _ = document;
    let world = SpikeWorld::new(format!("#read(\"{OUTSIDE_FILE}\")"));
    let warned = typst::compile::<typst_layout::PagedDocument>(&world);
    let failed = warned.output.is_err();
    let message = warned
        .output
        .as_ref()
        .err()
        .map(|errors| {
            errors
                .iter()
                .map(|error| error.message.to_string())
                .collect::<Vec<_>>()
                .join("; ")
        })
        .unwrap_or_default();
    println!(
        "  [{}] 读取项目外文件：{}（{}）",
        if failed { "PASS" } else { "FAIL" },
        if failed { "被拒绝" } else { "竟然成功" },
        message
    );
}

/// Typst：外部插件（可执行本地代码）必须不可用。
fn check_typst_plugin() {
    let world = SpikeWorld::new("#plugin(\"/bin/sh\")".to_string());
    let warned = typst::compile::<typst_layout::PagedDocument>(&world);
    let failed = warned.output.is_err();
    let message = warned
        .output
        .as_ref()
        .err()
        .map(|errors| {
            errors
                .iter()
                .map(|error| error.message.to_string())
                .collect::<Vec<_>>()
                .join("; ")
        })
        .unwrap_or_default();
    println!(
        "  [{}] 加载本地插件：{}（{}）",
        if failed { "PASS" } else { "FAIL" },
        if failed { "被拒绝" } else { "竟然成功" },
        message
    );
}

/// OS 级沙箱（bwrap）能否阻断读取与越界写入。
///
/// 这是本项最有价值的对照：TeX 自身的 `openin_any` 挡不住引擎级 `\openin`，
/// 但把进程放进只读挂载的命名空间后，项目外的文件**根本不存在**。
fn check_os_sandbox() {
    let dir = fresh_dir("sandbox");
    let benign = "\\documentclass{article}\\begin{document}sandbox\\end{document}";
    std::fs::write(dir.join("benign.tex"), benign).expect("写良性文档");
    let malicious = "\\documentclass{article}\\begin{document}         \\newread\\f\\openin\\f=/etc/hostname\\read\\f to \\line         \\message{SCHOLIUM-LEAK:[\\line]}\\closein\\f done\\end{document}";
    std::fs::write(dir.join("malicious.tex"), malicious).expect("写恶意文档");

    // 项目外文件在沙箱内是否可见。
    let visible = Command::new("bwrap")
        .args(sandbox_args(&dir))
        .args(["/bin/cat", "/etc/hostname"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    println!(
        "  [{}] 沙箱内能否读到 /etc/hostname：{}",
        if visible { "FAIL" } else { "PASS" },
        if visible { "能读到" } else { "不可见" }
    );

    // 沙箱内跑恶意文档：不应泄漏。
    let ran = Command::new("bwrap")
        .args(sandbox_args(&dir))
        .args(["/usr/bin/xelatex", "-interaction=nonstopmode", "malicious.tex"])
        .output();
    let log = std::fs::read_to_string(dir.join("malicious.log")).unwrap_or_default();
    let leaked = log.contains("SCHOLIUM-LEAK:[") && !log.contains("SCHOLIUM-LEAK:[]");
    println!(
        "  [{}] 沙箱内编译恶意文档：泄漏={leaked}（进程启动={}）",
        if leaked { "FAIL" } else { "PASS" },
        ran.map(|output| output.status.code().unwrap_or(-1)).unwrap_or(-1)
    );

    // 沙箱内良性文档能否正常产出：如实记录，不当作通过。
    let benign = Command::new("bwrap")
        .args(sandbox_args(&dir))
        .args(["/usr/bin/xelatex", "-interaction=nonstopmode", "benign.tex"])
        .output();
    let ok = benign
        .as_ref()
        .map(|output| output.status.success())
        .unwrap_or(false);
    // 失败时把真实错误打出来：报告里记录的是 xdvipdfmx 读不到纸张定义，
    // 而不是"沙箱挂了"这种没有信息量的结论。
    let driver_error = String::from_utf8_lossy(
        &benign.map(|output| output.stderr).unwrap_or_default(),
    )
    .lines()
    .find(|line| line.contains("xdvipdfmx") || line.contains("Error"))
    .unwrap_or("")
    .trim()
    .to_string();
    println!(
        "  [记录] 沙箱内良性文档编译：{}；驱动错误：{}",
        if ok { "成功" } else { "失败" },
        if driver_error.is_empty() {
            "无".to_string()
        } else {
            driver_error
        }
    );
}

/// bwrap 参数：只读挂载运行时与 TeX 资源，项目目录可写，其余一律不可见。
fn sandbox_args(dir: &std::path::Path) -> Vec<String> {
    let text = dir.to_string_lossy().to_string();
    vec![
        "--ro-bind".into(), "/usr".into(), "/usr".into(),
        "--ro-bind".into(), "/lib".into(), "/lib".into(),
        "--ro-bind".into(), "/lib64".into(), "/lib64".into(),
        "--ro-bind".into(), "/bin".into(), "/bin".into(),
        "--ro-bind".into(), "/etc/fonts".into(), "/etc/fonts".into(),
        "--ro-bind".into(), "/var/lib/texmf".into(), "/var/lib/texmf".into(),
        "--ro-bind".into(),
        "/home/ation_ciger/.cache/fontconfig".into(),
        "/home/ation_ciger/.cache/fontconfig".into(),
        "--proc".into(), "/proc".into(),
        "--dev".into(), "/dev".into(),
        "--tmpfs".into(), "/tmp".into(),
        "--bind".into(), text.clone(), "/work".into(),
        "--chdir".into(), "/work".into(),
        "--setenv".into(), "HOME".into(), "/tmp".into(),
        "--unshare-pid".into(),
        "--unshare-net".into(),
        "--unshare-ipc".into(),
        "--unshare-uts".into(),
        "--unshare-cgroup".into(),
        "--die-with-parent".into(),
    ]
}

/// 最小 Typst World：除主文件外**任何**文件都读不到。
struct SpikeWorld {
    library: typst::utils::LazyHash<typst::Library>,
    fonts: typst_kit::fonts::FontStore,
    main: typst::syntax::FileId,
    source: std::sync::Mutex<typst::syntax::Source>,
}

impl SpikeWorld {
    fn new(text: String) -> Self {
        let mut fonts = typst_kit::fonts::FontStore::new();
        fonts.extend(typst_kit::fonts::embedded());
        fonts.extend(typst_kit::fonts::system());
        let path = typst::syntax::RootedPath::new(
            typst::syntax::VirtualRoot::Project,
            typst::syntax::VirtualPath::new("main.typ").expect("固定路径"),
        );
        let main = path.intern();
        Self {
            library: typst::utils::LazyHash::new(<typst::Library as typst::LibraryExt>::default()),
            fonts,
            main,
            source: std::sync::Mutex::new(typst::syntax::Source::new(main, text)),
        }
    }
}

impl typst::World for SpikeWorld {
    fn library(&self) -> &typst::utils::LazyHash<typst::Library> {
        &self.library
    }

    fn book(&self) -> &typst::utils::LazyHash<typst::text::FontBook> {
        self.fonts.book()
    }

    fn main(&self) -> typst::syntax::FileId {
        self.main
    }

    fn source(
        &self,
        id: typst::syntax::FileId,
    ) -> Result<typst::syntax::Source, typst::diag::FileError> {
        if id == self.main {
            let source = self
                .source
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            return Ok(source.clone());
        }
        Err(typst::diag::FileError::NotFound(
            id.vpath().get_without_slash().into(),
        ))
    }

    fn file(
        &self,
        id: typst::syntax::FileId,
    ) -> Result<typst::foundations::Bytes, typst::diag::FileError> {
        // 关键：除主文件外一律拒绝，**不做任何磁盘访问**。
        Err(typst::diag::FileError::NotFound(
            id.vpath().get_without_slash().into(),
        ))
    }

    fn font(&self, index: usize) -> Option<typst::text::Font> {
        self.fonts.font(index)
    }

    fn today(&self, _offset: Option<typst::foundations::Duration>) -> Option<typst::foundations::Datetime> {
        None
    }
}
