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

/// 项目布局：两套**完全独立**的目录，源码、中间文件、产物各占一份。
#[derive(Debug)]
struct Project {
    latex_dir: PathBuf,
    typst_dir: PathBuf,
    latex_source: String,
    typst_source: String,
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
        })
    }

    fn source_dir(&self, engine: Engine) -> &Path {
        match engine {
            Engine::Latex => &self.latex_dir,
            Engine::Typst => &self.typst_dir,
        }
    }

    fn sandbox(&self, engine: Engine) -> PathBuf {
        self.source_dir(engine).join(".build")
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

    fn request(&self, engine: Engine) -> BuildRequest {
        let out_dir = self.sandbox(engine);
        let text = self.source_text(engine).to_string();
        BuildRequest {
            engine,
            sandbox: out_dir.clone(),
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

    let baseline = case_isolated_outputs(checks, &project)?;
    case_decoys_do_not_leak(checks, &project, &baseline)?;
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
                "{} ms / {} B / 页数 {:?} / {}",
                outcome.elapsed_ms,
                outcome.product_bytes,
                outcome.pages,
                outcome.engine_note
            ),
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

        // 源目录只应有源码，不应有任何引擎中间文件或产物。
        let source_dir = project.source_dir(engine);
        let leaked: Vec<String> = fsutil::list_names(source_dir)?
            .into_iter()
            .filter(|name| name != &project.source_name(engine))
            .collect();
        checks.expect(
            leaked.is_empty(),
            &format!("{}.源目录无中间文件泄漏", engine.slug()),
            &format!("目录={} 多余项={leaked:?}", source_dir.display()),
        );

        let intermediates = intermediate_files(engine, &outcome.sandbox_files);
        match engine {
            Engine::Latex => checks.expect(
                intermediates.len() >= 3,
                "latex.确实产生了中间文件（否则隔离无从验证）",
                &format!("{:?}", intermediates),
            ),
            Engine::Typst => checks.expect(
                intermediates.is_empty(),
                "typst.不产生中间文件",
                &format!("沙箱文件={:?}", outcome.sandbox_files),
            ),
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
/// 毒饵是真的 A 的 aux，把前 64 字节替换为垃圾但保持长度与 mtime：
/// 如果 A 真的读了 B 目录里的同名 aux，LaTeX 会吐出不同的 PDF（或直接报错）。
fn case_decoys_do_not_leak(
    checks: &mut Checks,
    project: &Project,
    baseline: &[(Engine, BuildOutcome)],
) -> Result<()> {
    checks.case("build.decoy-isolation");
    let baseline_latex = &baseline[0].1;

    let aux = project.sandbox(Engine::Latex).join("main.aux");
    let decoy_dir = project.sandbox(Engine::Typst);
    std::fs::create_dir_all(&decoy_dir).map_err(crate::error::io_context(&decoy_dir))?;

    // 同目录、同名（含相同 jobname 前缀）、同长度、同 mtime 的毒饵。
    if let Ok(original) = fsutil::read_file(&aux) {
        let decoy = decoy_dir.join("main.aux");
        let mut poisoned = original.clone();
        for byte in poisoned.iter_mut().take(64) {
            *byte = b'%';
        }
        fsutil::write_file(&decoy, &poisoned)?;
        let meta_source = std::fs::metadata(&aux).map_err(crate::error::io_context(&aux))?;
        if let Ok(mtime) = meta_source.modified() {
            let times = std::fs::FileTimes::new().set_modified(mtime);
            if let Ok(handle) = std::fs::OpenOptions::new().write(true).open(&decoy) {
                let _ = handle.set_times(times);
            }
        }
        checks.expect(
            poisoned.len() == original.len() && poisoned[..64] != original[..64],
            "毒饵已构造（同长度、前 64 字节不同）",
            &format!(
                "原 aux {} B，毒饵 {} B，毒饵前 8 字节={:?}",
                original.len(),
                poisoned.len(),
                &poisoned[..8.min(poisoned.len())]
            ),
        );
        checks.note("毒饵位置", &decoy.display().to_string());
    } else {
        checks.expect(false, "毒饵前置条件：A 的 aux 存在", &aux.display().to_string());
    }

    let after = build::build(&project.request(Engine::Latex))?;
    checks.expect(
        after.product_hash == baseline_latex.product_hash,
        "A 重编产物与干净基线完全一致（没读到 B 的毒饵）",
        &format!(
            "基线 {} vs 有饵 {}",
            &baseline_latex.product_hash[..16],
            &after.product_hash[..16]
        ),
    );
    checks.expect(
        after.product_bytes == baseline_latex.product_bytes,
        "A 产物字节数一致",
        &format!("{} vs {}", baseline_latex.product_bytes, after.product_bytes),
    );

    // 对照组：让 A 自己的输入真的变一次，产物必须变——证明上面的一致性不是因为产物恒定。
    let mut request = project.request(Engine::Latex);
    request.source_text.push_str("\\par CONTROL-PROBE-9117\n");
    request.source_hash = fsutil::sha256_hex(request.source_text.as_bytes());
    let control = build::build(&request)?;
    checks.expect(
        control.product_hash != baseline_latex.product_hash,
        "对照：A 自己的源码变化会改变产物（一致性不是恒等）",
        &format!(
            "对照 {} vs 基线 {}",
            &control.product_hash[..16],
            &baseline_latex.product_hash[..16]
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
        latex_outcome.product_hash == baseline[0].1.product_hash,
        "并发 latex 产物与串行基线逐字节一致",
        &format!("{} vs {}", &latex_outcome.product_hash[..16], &baseline[0].1.product_hash[..16]),
    );
    checks.expect(
        typst_outcome.product_hash == baseline[1].1.product_hash,
        "并发 typst 产物与串行基线逐字节一致",
        &format!("{} vs {}", &typst_outcome.product_hash[..16], &baseline[1].1.product_hash[..16]),
    );
    checks.expect(
        latex_outcome.product_hash != typst_outcome.product_hash,
        "两条路径的产物互不相同（没有互相写进对方文件）",
        &format!("{} vs {}", &latex_outcome.product_hash[..16], &typst_outcome.product_hash[..16]),
    );
    checks.expect(
        latex_outcome.sandbox_files.iter().any(|name| name.ends_with(".aux"))
            && !typst_outcome.sandbox_files.iter().any(|name| name.ends_with(".aux")),
        "并发后中间文件仍只落在 latex 沙箱",
        &format!("latex={:?} typst={:?}", latex_outcome.sandbox_files, typst_outcome.sandbox_files),
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
        recovered.product_hash == baseline[0].1.product_hash,
        "锁释放后可重建且产物与基线一致",
        &format!("{}", &recovered.product_hash[..16]),
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
fn case_output_dir_lock(checks: &mut Checks, project: &Project) -> Result<()> {
    checks.case("build.output-lock");
    let out_dir = project.source_dir(Engine::Typst).join("locked-build");
    std::fs::create_dir_all(&out_dir).map_err(crate::error::io_context(&out_dir))?;

    let held = fsutil::DirLock::acquire(&out_dir, "LOCK")?;
    let request = BuildRequest {
        out_dir: out_dir.clone(),
        sandbox: out_dir.clone(),
        ..project.request(Engine::Typst)
    };
    let outcome = build::build(&request);
    let message = outcome.as_ref().err().map(|error| error.to_string());
    checks.expect(
        outcome.is_err(),
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
        ok.product_bytes > 0,
        "释放锁后构建成功",
        &format!("{} B", ok.product_bytes),
    );
    Ok(())
}

/// 从沙箱文件清单里挑出引擎中间文件。
fn intermediate_files(engine: Engine, names: &[String]) -> Vec<String> {
    match engine {
        Engine::Latex => crate::build::latex::intermediates(names),
        Engine::Typst => names
            .iter()
            .filter(|name| {
                matches!(
                    Path::new(name).extension().and_then(|ext| ext.to_str()),
                    Some("aux" | "log" | "fls" | "toc" | "out" | "xdv")
                )
            })
            .cloned()
            .collect(),
    }
}
