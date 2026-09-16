//! 判据 1：两种引擎隔离构建。
//!
//! 四条独立判据：
//! 1. 每条引擎的中间文件只出现在自己的沙箱里；
//! 2. 并发构建互不覆盖（含并发锁冲突）；
//! 3. A 不会读到 B 的中间文件——用**改变 aux 语义的毒饵**证明，而不是靠代码走查；
//! 4. 一条路径的产物随自己的源码变化，证明编译确实读了这份输入（否则"隔离"是空话）。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::build::{self, BuildOutcome, BuildRequest, Engine};
use crate::check::Checks;
use crate::error::Result;
use crate::fsutil::{self, Scratch};
use crate::{fixture, render};

/// 每条引擎的沙箱子目录名。放在源码目录**内部**但独立成目录：
/// 源码在 `project/<engine>/main.tex`，中间文件与产物在 `project/<engine>/.build/`。
const SANDBOX_DIR: &str = ".build";

/// 项目布局：两套**完全独立**的目录，源码、中间文件、产物各占一份。
#[derive(Debug)]
struct Project {
    latex_dir: PathBuf,
    typst_dir: PathBuf,
    latex_source: String,
    typst_source: String,
    /// 为验证"读到别人中间文件"而**故意**放进 B 沙箱的文件名。
    /// 它们必须在所有"沙箱内容"断言里被排除，否则隔离通过/失败会被自己的夹具搅乱。
    planted: Vec<String>,
}

impl Project {
    /// 在 `root` 下建立布局并写入两套源码。
    fn new(root: &Path) -> Result<Self> {
        let (editor, _) = fixture::editor();
        let latex_source = render::latex(editor.document());
        let typst_source = render::typst(editor.document());

        let latex_dir = root.join("project/latex");
        let typst_dir = root.join("project/typst");
        fsutil::write_file(
            &latex_dir.join("main.tex"),
            latex_source.as_bytes(),
        )?;
        fsutil::write_file(
            &typst_dir.join("main.typ"),
            typst_source.as_bytes(),
        )?;

        Ok(Self {
            latex_dir,
            typst_dir,
            latex_source,
            typst_source,
            planted: Vec::new(),
        })
    }

    fn source_dir(&self, engine: Engine) -> &Path {
        match engine {
            Engine::Latex => &self.latex_dir,
            Engine::Typst => &self.typst_dir,
        }
    }

    fn sandbox(&self, engine: Engine) -> PathBuf {
        self.source_dir(engine).join(SANDBOX_DIR)
    }

    fn source_text(&self, engine: Engine) -> &str {
        match engine {
            Engine::Latex => &self.latex_source,
            Engine::Typst => &self.typst_source,
        }
    }

    fn source_name(&self, engine: Engine) -> String {
        let name = match engine {
            Engine::Latex => "main.tex",
            Engine::Typst => "main.typ",
        };
        name.to_string()
    }

    /// 记录一个故意植入的毒饵文件名。
    fn plant(&mut self, name: &str) {
        self.planted.push(name.to_string());
    }

    fn request(&self, engine: Engine) -> BuildRequest {
        let out_dir = self.sandbox(engine);
        let text = self.source_text(engine).to_string();
        BuildRequest {
            engine,
            out_dir: out_dir.clone(),
            job_name: "main".to_string(),
            source_path: out_dir.join(self.source_name(engine)),
            source_hash: fsutil::sha256_hex(text.as_bytes()),
            source_text: text,
        }
    }
}

/// 跑判据 1 的全部用例。
///
/// # Errors
///
/// 夹具、构建或文件系统操作失败（都会让判据本身失效，因此直接返回错误）。
pub fn run(checks: &mut Checks, workspace: &Path) -> Result<()> {
    println!("\n## 判据 1：两种引擎隔离构建");
    let scratch = Scratch::create(workspace, "c1")?;
    let project = Project::new(scratch.root())?;

    let mut project = project;
    let baseline = case_isolated_outputs(checks, &project)?;
    case_decoys_do_not_leak(checks, &mut project, &baseline)?;
    case_concurrent_builds(checks, &project, &baseline)?;
    case_own_source_is_read(checks, &project)?;
    case_output_dir_lock(checks, &project)?;
    Ok(())
}

/// 用例 1：两条路径各自构建成功，中间文件只落在自己的沙箱。
fn case_isolated_outputs(
    checks: &mut Checks,
    project: &Project,
) -> Result<Vec<(Engine, BuildOutcome)>> {
    checks.case("build.isolated-outputs");
    let mut outcomes = Vec::new();

    for engine in [Engine::Latex, Engine::Typst] {
        let outcome = build::build(&project.request(engine))?;
        checks.note(
            &format!("{} 构建", engine.slug()),
            &format!(
                "{} ms / {} B / 页数 {:?} / 产物 {} / {}",
                outcome.elapsed_ms,
                outcome.product_bytes,
                outcome.pages,
                outcome.product_path.display(),
                outcome.engine_note
            ),
        );
        checks.note(
            &format!("{}.输出目录内容哈希", engine.slug()),
            &digest_summary(&outcome),
        );
        checks.expect(
            outcome.product_bytes > 0,
            &format!("{}.产物非空", engine.slug()),
            &format!("{} B", outcome.product_bytes),
        );
        checks.expect(
            outcome.source_hash == project.request(engine).source_hash,
            &format!("{}.产物记录了输入哈希", engine.slug()),
            &format!("source_hash={}", &outcome.source_hash[..16]),
        );
        checks.expect(
            outcome.product_signature.is_some(),
            &format!("{}.拿到了跨运行稳定的产物签名", engine.slug()),
            &format!(
                "signature={:?}；产物文件哈希 {}（{}）",
                outcome.product_signature.as_deref().map(short),
                short(&outcome.product_hash),
                match engine {
                    // xelatex 把运行时间与随机 PDF ID 写进 PDF，文件哈希跨运行不稳定。
                    Engine::Latex => "PDF 含时间戳，文件哈希跨运行会变，判据用签名",
                    Engine::Typst => "文本产物，文件哈希与签名同值",
                }
            ),
        );

        // 源目录只应有源码与**被命名的**沙箱子目录；任何引擎中间文件出现在源码旁边
        // 都算泄漏。这里把"隔离目录本身"列入允许项，不把 `.build` 当成产物。
        let source_dir = project.source_dir(engine);
        let allowed = [project.source_name(engine), SANDBOX_DIR.to_string()];
        let leaked: Vec<String> = fsutil::list_names(source_dir)?
            .into_iter()
            .filter(|name| !allowed.contains(name))
            .collect();
        checks.expect(
            leaked.is_empty(),
            &format!("{}.源目录无中间文件泄漏", engine.slug()),
            &format!(
                "目录={} 内容={:?} 多余项={leaked:?}",
                source_dir.display(),
                fsutil::list_names(source_dir)?
            ),
        );

        let intermediates = intermediate_files(engine, &outcome.sandbox_files);
        match engine {
            Engine::Latex => {
                checks.expect(
                    intermediates.len() >= 3,
                    "latex.确实产生了中间文件（否则隔离无从验证）",
                    &format!("{:?}", intermediates),
                );
            }
            Engine::Typst => {
                checks.expect(
                    intermediates.is_empty(),
                    "typst.不产生中间文件",
                    &format!("沙箱文件={:?}", outcome.sandbox_files),
                );
            }
        }
        checks.expect(
            outcome.sandbox_files.contains(&format!("main.{}", engine.product_ext())),
            &format!("{}.产物写在沙箱里", engine.slug()),
            &format!("main.{}", engine.product_ext()),
        );
        outcomes.push((engine, outcome));
    }

    let (latex, typst) = (&outcomes[0].1, &outcomes[1].1);
    checks.expect(
        latex.sandbox_files != typst.sandbox_files,
        "两条路径的沙箱内容不同（目录确实分离）",
        &format!("latex={:?} typst={:?}", latex.sandbox_files, typst.sandbox_files),
    );
    checks.expect(
        latex.product_hash != typst.product_hash,
        "两条路径产物不同名同哈希（不是同一份文件）",
        &format!("latex={} typst={}", &latex.product_hash[..16], &typst.product_hash[..16]),
    );
    Ok(outcomes)
}

/// 用例 2：B 沙箱里放"改变 aux 语义"的毒饵，A 的产物必须不变。
///
/// 毒饵：把 B 的沙箱里的 `main.aux` 换成一份**语义明显不同**的 aux。
///
/// 为什么不是"翻转几个字节"：夹具文档很小，aux 只有 32 字节，且 xelatex 的 PDF 元数据里
/// 带时间戳，翻转垃圾字节未必改变产物，断言就退化成"两次运行都成功"这种空证据。
/// 这里改成写入 `\@setckpt` 声明**999 页**的合法 aux：如果 A 真的读了 B 的同名 aux，
/// LaTeX 会把页数写入 PDF，哈希必然变化；A 只在读自己的 aux 时才能得到干净基线。
fn case_decoys_do_not_leak(
    checks: &mut Checks,
    project: &mut Project,
    baseline: &[(Engine, BuildOutcome)],
) -> Result<()> {
    checks.case("build.decoy-isolation");
    let baseline_latex = &baseline[0].1;

    let aux = project.sandbox(Engine::Latex).join("main.aux");
    let decoy_dir = project.sandbox(Engine::Typst);
    std::fs::create_dir_all(&decoy_dir).map_err(crate::error::io_context(&decoy_dir))?;
    let original = fsutil::read_file(&aux)?;

    // 同 jobname、同名、内容语义相反（页数 999）的毒饵，并保留 A 的 aux 的 mtime，
    // 让"按时间戳决定是否重跑"的实现也无法靠 mtime 区分。
    let poisoned = b"\\relax\n\\@setckpt{999}{\\setcounter{page}{999}}\n".to_vec();
    let decoy = decoy_dir.join("main.aux");
    project.plant("main.aux");
    fsutil::write_file(&decoy, &poisoned)?;
    if let Ok(mtime) = std::fs::metadata(&aux).and_then(|meta| meta.modified()) {
        let times = std::fs::FileTimes::new().set_modified(mtime);
        if let Ok(handle) = std::fs::OpenOptions::new().write(true).open(&decoy) {
            let _ = handle.set_times(times);
        }
    }
    checks.note(
        "毒饵",
        &format!(
            "{} ← 内容 {:?}（原 aux {} B，语义声明 999 页）",
            decoy.display(),
            String::from_utf8_lossy(&poisoned).trim(),
            original.len()
        ),
    );
    checks.expect(
        poisoned != original,
        "毒饵内容与 A 自己的 aux 不同（若被读到必然改变产物）",
        &format!("毒饵 {} B / 原 {} B", poisoned.len(), original.len()),
    );

    let after = build::build(&project.request(Engine::Latex))?;
    checks.expect(
        after.product_signature == baseline_latex.product_signature,
        "A 重编产物签名与干净基线一致（没读到 B 的毒饵）",
        &format!(
            "基线 {} vs 有饵 {}；文件哈希 {} vs {}（时间戳不同属正常）",
            signature(&after),
            signature(baseline_latex),
            short(&after.product_hash),
            short(&baseline_latex.product_hash)
        ),
    );
    checks.expect(
        after.product_bytes == baseline_latex.product_bytes,
        "A 产物字节数一致",
        &format!("{} vs {}", baseline_latex.product_bytes, after.product_bytes),
    );
    checks.expect(
        fsutil::read_file(&decoy)? == poisoned,
        "B 的毒饵没有被 A 的构建改写",
        &format!("毒饵仍为 {} B", poisoned.len()),
    );

    // 对照组：让 A 自己的输入真的变一次，产物必须变——证明上面的一致性不是因为产物恒定。
    let mut request = project.request(Engine::Latex);
    // 新源文件名：强制 latexmk 真的重跑，而不是命中上一次的产物。
    request.job_name = "control".to_string();
    request.source_path = project.sandbox(Engine::Latex).join("control.tex");
    // 插在 `\end{document}` **之前**：追加到文件末尾只是把文字放到文档外，
    // 页面上什么都不出现，签名当然不变——第一次就是这么被骗过去的。
    request.source_text = request
        .source_text
        .replace(
            "\\end{document}",
            "\\par\\noindent CONTROL-PROBE-9117\n\\end{document}",
        );
    request.source_hash = fsutil::sha256_hex(request.source_text.as_bytes());
    let control = build::build(&request)?;
    checks.expect(
        control.product_signature != baseline_latex.product_signature,
        "对照：A 自己的源码变化会改变产物签名（一致性不是恒等）",
        &format!(
            "对照 {} vs 基线 {}",
            signature(&control),
            signature(baseline_latex)
        ),
    );
    checks.expect(
        !control.sandbox_files.is_empty() && control.product_bytes > 0,
        "对照构建本身成功",
        &format!("{} B", control.product_bytes),
    );

    // 恢复 A 的源码，避免污染后续用例的基线。
    build::build(&project.request(Engine::Latex))?;
    Ok(())
}

/// 用例 3：并发的两条路径各自产出与串行基线一致的字节，且锁冲突被拒绝。
fn case_concurrent_builds(
    checks: &mut Checks,
    project: &Project,
    baseline: &[(Engine, BuildOutcome)],
) -> Result<()> {
    checks.case("build.concurrent-paths");

    let latex_request = Arc::new(project.request(Engine::Latex));
    let typst_request = Arc::new(project.request(Engine::Typst));
    let (latex_handle, typst_handle) = spawn_pair(latex_request, typst_request);

    let latex_outcome = run_thread(checks, "latex", latex_handle)?;
    let typst_outcome = run_thread(checks, "typst", typst_handle)?;

    checks.note(
        "并发耗时",
        &format!("latex={} ms typst={} ms", latex_outcome.elapsed_ms, typst_outcome.elapsed_ms),
    );
    checks.expect(
        latex_outcome.product_signature == baseline[0].1.product_signature,
        "并发 latex 产物签名与串行基线一致",
        &format!("{} vs {}", signature(&latex_outcome), signature(&baseline[0].1)),
    );
    checks.expect(
        typst_outcome.product_signature == baseline[1].1.product_signature,
        "并发 typst 产物签名与串行基线一致",
        &format!("{} vs {}", signature(&typst_outcome), signature(&baseline[1].1)),
    );
    checks.expect(
        latex_outcome.product_signature != typst_outcome.product_signature,
        "两条路径的产物签名不同（没有互相写进对方文件）",
        &format!("{} vs {}", signature(&latex_outcome), signature(&typst_outcome)),
    );
    // typst 沙箱里只允许出现"我们故意植入的毒饵"；任何其它 LaTeX 中间文件都算越界。
    let typst_foreign: Vec<&String> = typst_outcome
        .sandbox_files
        .iter()
        .filter(|name| name.ends_with(".aux") && !project.planted.contains(name))
        .collect();
    checks.expect(
        latex_outcome.sandbox_files.iter().any(|name| name.ends_with(".aux"))
            && typst_foreign.is_empty(),
        "并发后中间文件仍只落在 latex 沙箱",
        &format!(
            "latex={:?} typst={:?} typst 中非植入的中间文件={typst_foreign:?}",
            latex_outcome.sandbox_files, typst_outcome.sandbox_files
        ),
    );

    // 同一输出目录的第二个写者必须被锁拒绝，而不是排队/覆盖。
    let contended = project.sandbox(Engine::Latex);
    let lock = fsutil::DirLock::acquire(&contended, "LOCK")?;
    let lock_path = lock.path().display().to_string();
    let conflict = build::build(&project.request(Engine::Latex));
    checks.expect(
        conflict.is_err(),
        "同目录并发写被锁拒绝",
        &format!("锁={lock_path} 结果={:?}", conflict.err().map(|error| error.to_string())),
    );
    drop(lock);

    let recovered = build::build(&project.request(Engine::Latex))?;
    checks.expect(
        recovered.product_signature == baseline[0].1.product_signature,
        "锁释放后可重建且产物签名与基线一致",
        &signature(&recovered),
    );
    Ok(())
}

fn spawn_pair(
    latex: Arc<BuildRequest>,
    typst: Arc<BuildRequest>,
) -> (
    std::thread::JoinHandle<Result<BuildOutcome>>,
    std::thread::JoinHandle<Result<BuildOutcome>>,
) {
    let latex_handle = std::thread::spawn(move || build::build(&latex));
    let typst_handle = std::thread::spawn(move || build::build(&typst));
    (latex_handle, typst_handle)
}

fn run_thread(
    checks: &mut Checks,
    label: &str,
    handle: std::thread::JoinHandle<Result<BuildOutcome>>,
) -> Result<BuildOutcome> {
    match handle.join() {
        Ok(result) => result,
        Err(_) => {
            checks.expect(false, &format!("{label} 线程未 panic"), "join 返回 Err");
            Err(crate::error::SpikeError::Core(format!("{label} 线程 panic")))
        }
    }
}

/// 用例 4：产物随**自己的**源码变化——证明编译真的读了这份输入。
///
/// 值用 no-op 数学宏：`\ensuremath{5}` 语义上与 `5` 相同，但源码字节不同，
/// 因此只改了"读了什么"，没改"排版结果应该是什么"。
fn case_own_source_is_read(checks: &mut Checks, project: &Project) -> Result<()> {
    checks.case("build.source-fidelity");
    let base = build::build(&project.request(Engine::Latex))?;

    let mut request = project.request(Engine::Latex);
    let before = request.source_text.clone();
    request.source_text = request
        .source_text
        .replace("\\frac{a}{b}", "\\frac{a}{\\ensuremath{b}}");
    checks.expect(
        request.source_text != before,
        "探针确实改动了源码字节",
        &format!("{} B → {} B", before.len(), request.source_text.len()),
    );
    request.source_hash = fsutil::sha256_hex(request.source_text.as_bytes());
    let after = build::build(&request)?;
    checks.expect(
        after.product_hash != base.product_hash,
        "改自己的源码 → 产物变化（输入确实被读取）",
        &format!("{} → {}", &base.product_hash[..16], &after.product_hash[..16]),
    );
    checks.expect(
        after.product_bytes > 0 && after.pages.is_some(),
        "改动后仍然构建成功",
        &format!("{} B / 页数 {:?}", after.product_bytes, after.pages),
    );
    Ok(())
}

/// 用例 5：输出目录被占用时，构建失败而不是覆盖。
///
/// 这里刻意换一个**新的源文件名**：如果沿用 `main.typ`，锁释放后的重建会命中
/// 上一次的产物缓存，构建耗时与"是否真的重跑"都说不清。换名 + 改内容之后，
/// 释放锁后的构建必然是一次真实的引擎调用，签名一致才有意义。
fn case_output_dir_lock(checks: &mut Checks, project: &Project) -> Result<()> {
    checks.case("build.output-lock");
    let out_dir = project.source_dir(Engine::Typst).join("locked-build");
    std::fs::create_dir_all(&out_dir).map_err(crate::error::io_context(&out_dir))?;

    let mut request = project.request(Engine::Typst);
    request.out_dir = out_dir.clone();
    request.job_name = "locked".to_string();
    request.source_path = out_dir.join("locked.typ");
    request.source_text.push_str("// locked-probe\n");
    request.source_hash = fsutil::sha256_hex(request.source_text.as_bytes());

    // 先在没有锁的情况下构建一次，得到"预期签名"作为对照。
    let control = build::build(&request)?;
    checks.note(
        "锁用例对照构建",
        &format!("{} ms / signature {}", control.elapsed_ms, signature(&control)),
    );
    // 清掉对照产物，确保后面"被拒绝后目录里只有锁"这一步是真的没写东西。
    for name in fsutil::list_names(&out_dir)? {
        if name != "LOCK" {
            std::fs::remove_file(out_dir.join(&name))
                .map_err(crate::error::io_context(out_dir.join(&name)))?;
        }
    }

    let held = fsutil::DirLock::acquire(&out_dir, "LOCK")?;
    let rejected = build::build(&request);
    let message = rejected.as_ref().err().map(|error| error.to_string());
    checks.expect(
        rejected.is_err(),
        "被占用的输出目录拒绝构建",
        &format!("锁={} 错误={:?}", held.path().display(), message),
    );
    let names = fsutil::list_names(&out_dir)?;
    checks.expect(
        names.iter().all(|name| name == "LOCK"),
        "被拒绝的构建没有在输出目录留下任何文件",
        &format!("{names:?}"),
    );
    drop(held);

    let ok = build::build(&request)?;
    checks.expect(
        ok.product_signature == control.product_signature,
        "释放锁后重建成功且产物签名与对照一致",
        &format!("{} vs {}", signature(&ok), signature(&control)),
    );
    Ok(())
}

/// 签名的短形式，用于证据行。
fn signature(outcome: &BuildOutcome) -> String {
    match &outcome.product_signature {
        Some(value) => short(value),
        None => "<无签名>".to_string(),
    }
}

/// 哈希前 16 位。
fn short(hash: &str) -> String {
    hash.chars().take(16).collect()
}

/// 从沙箱文件清单里挑出引擎中间文件。
fn intermediate_files(engine: Engine, names: &[String]) -> Vec<String> {
    match engine {
        Engine::Latex => crate::build::latex::intermediates(names),
        // Typst 路径**不落任何中间文件**；把它的产物扩展名 `.out` 排除在外，
        // 否则"产物"会被自己的检查误判成"中间文件"。
        Engine::Typst => names
            .iter()
            .filter(|name| {
                matches!(
                    Path::new(name).extension().and_then(|ext| ext.to_str()),
                    Some("aux" | "log" | "fls" | "toc" | "xdv")
                )
            })
            .cloned()
            .collect(),
    }
}

/// 输出目录的内容哈希摘要（文件名:哈希前 8 位），进证据行。
fn digest_summary(outcome: &BuildOutcome) -> String {
    outcome
        .output_digest
        .iter()
        .map(|(name, hash)| format!("{name}:{}", &hash[..8]))
        .collect::<Vec<_>>()
        .join(" ")
}
