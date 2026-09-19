//! Build baseline evidence, kept separate from poisoning and concurrency scenarios.
use super::*;
/// 用例 1：两条路径各自构建成功，中间文件只落在自己的沙箱。
pub(super) fn case_isolated_outputs(
    checks: &mut Checks,
    project: &Project,
) -> Result<Vec<(Engine, BuildOutcome)>> {
    checks.case("build.isolated-outputs");
    let mut outcomes = Vec::new();

    for engine in [Engine::Latex, Engine::Typst] {
        let outcome = build::build(&project.request(engine))?;
        record_product(checks, project, engine, &outcome);
        record_paths(checks, project, engine, &outcome)?;
        outcomes.push((engine, outcome));
    }

    let (latex, typst) = (&outcomes[0].1, &outcomes[1].1);
    checks.expect(
        latex.sandbox_files != typst.sandbox_files,
        "两条路径的沙箱内容不同（目录确实分离）",
        &format!(
            "latex={:?} typst={:?}",
            latex.sandbox_files, typst.sandbox_files
        ),
    );
    checks.expect(
        latex.product_hash != typst.product_hash,
        "两条路径产物不同名同哈希（不是同一份文件）",
        &format!(
            "latex={} typst={}",
            &latex.product_hash[..16],
            &typst.product_hash[..16]
        ),
    );
    Ok(outcomes)
}

fn record_product(checks: &mut Checks, project: &Project, engine: Engine, outcome: &BuildOutcome) {
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
        &digest_summary(outcome),
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
}

fn record_paths(
    checks: &mut Checks,
    project: &Project,
    engine: Engine,
    outcome: &BuildOutcome,
) -> Result<()> {
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
        outcome
            .sandbox_files
            .contains(&format!("main.{}", engine.product_ext())),
        &format!("{}.产物写在沙箱里", engine.slug()),
        &format!("main.{}", engine.product_ext()),
    );
    Ok(())
}
