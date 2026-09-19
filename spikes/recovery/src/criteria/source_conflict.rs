//! 判据 3：外部源码修改冲突。
//!
//! 场景：应用把 `.tex` 交给用户编辑（源码工作台），磁盘上另有人/另一个工具改了同一个文件。
//! 保存时应用必须检测到内容哈希不符，**拒绝写入**并给出可读报告。
//!
//! 这里不实现"接受外部改动"的合并路径——那需要源码 reconcile（阶段 0 第 3 项）。
//! 本判据只回答"会不会静默覆盖"，答案是必须先拒绝，再看下一步。

use std::path::Path;

use crate::build::plan;
use crate::check::Checks;
use crate::error::Result;
use crate::fsutil;
use crate::fsutil::Scratch;
use crate::{fixture, render};

/// 应用侧的一次保存尝试及其报告。
struct Attempt {
    attempt: plan::SaveAttempt,
    report: Option<plan::ConflictReport>,
    disk_text: String,
}

/// 跑判据 3 的全部用例。
///
/// # Errors
///
/// 夹具或文件系统操作失败。
pub fn run(checks: &mut Checks, workspace: &Path) -> Result<()> {
    println!("\n## 判据 3：外部源码修改冲突");
    let scratch = Scratch::create(workspace, "c3")?;
    let source_text = fixture_source();

    case_external_change(checks, scratch.root(), &source_text)?;
    case_no_conflict_writes(checks, scratch.root(), &source_text)?;
    case_unchanged_is_rejected(checks, scratch.root(), &source_text)?;
    case_removed_file(checks, scratch.root(), &source_text)?;
    Ok(())
}

/// 用例 1：外部改动后保存被拒，磁盘内容不被覆盖。
fn case_external_change(checks: &mut Checks, root: &Path, source_text: &str) -> Result<()> {
    checks.case("conflict.external-change");
    let path = root.join("conflict/external/main.tex");
    let loaded = stage(&path, source_text)?;

    // 外部改动：追加注释行。内容哈希必然变化。
    let external = format!("{source_text}% external edit by another tool\n");
    std::fs::write(&path, external.as_bytes()).map_err(crate::error::io_context(&path))?;
    let external_hash = fsutil::sha256_hex(external.as_bytes());

    let app_text = format!("{source_text}% app edit\n");
    let result = attempt_save(&loaded, &app_text)?;

    checks.expect(
        matches!(
            result.attempt,
            plan::SaveAttempt::Rejected(plan::Rejection::ExternalChange { .. })
        ),
        "保存被拒绝，原因是外部改动",
        &format!("attempt={:?}", result.attempt),
    );
    checks.expect(
        result.disk_text == external,
        "磁盘内容仍是外部版本（未被静默覆盖）",
        &format!(
            "磁盘 {} B / app 想写 {} B",
            result.disk_text.len(),
            app_text.len()
        ),
    );
    checks.expect(
        fsutil::hash_file(&path)? == external_hash,
        "磁盘哈希等于外部版本哈希",
        &external_hash[..16],
    );
    checks.expect(
        !result.disk_text.contains("app edit"),
        "磁盘上找不到 app 的改动",
        "磁盘文本不含 'app edit'",
    );

    let report = result
        .report
        .ok_or_else(|| crate::error::SpikeError::Core("拒写却没有报告".to_string()))?;
    let rendered = report.render();
    println!("{rendered}");
    checks.expect(
        rendered.contains("external-change"),
        "报告写明判定为 external-change",
        "报告含判定标签",
    );
    checks.expect(
        rendered.contains(&loaded.loaded_hash[..16]) && rendered.contains(&external_hash[..16]),
        "报告同时给出加载哈希与当前哈希",
        &format!("{} / {}", &loaded.loaded_hash[..16], &external_hash[..16]),
    );
    checks.expect(
        !report.diff_lines.is_empty(),
        "报告列出差异行",
        &format!("差异 {} 行", report.diff_lines.len()),
    );
    checks.expect(
        report
            .diff_lines
            .iter()
            .any(|diff| diff.current.contains("external edit")),
        "差异行内容指出了外部改动的那一行",
        &report
            .diff_lines
            .iter()
            .map(|diff| format!("L{}: {}", diff.line, diff.current))
            .collect::<Vec<_>>()
            .join(" | "),
    );
    Ok(())
}

/// 用例 2：无人改动时保存成功——拒绝逻辑不能把正常保存也拦掉。
fn case_no_conflict_writes(checks: &mut Checks, root: &Path, source_text: &str) -> Result<()> {
    checks.case("conflict.no-external-change");
    let path = root.join("conflict/clean/main.tex");
    let loaded = stage(&path, source_text)?;
    let app_text = format!("{source_text}% app edit committed\n");
    let result = attempt_save(&loaded, &app_text)?;

    checks.expect(
        result.attempt.wrote(),
        "无人改动时保存成功写盘",
        &format!("{:?}", result.attempt),
    );
    checks.expect(
        result.disk_text == app_text,
        "磁盘内容等于应用写入的内容",
        &format!("磁盘 {} B", result.disk_text.len()),
    );
    checks.expect(
        result.report.is_none(),
        "成功路径不产生冲突报告",
        "report=None",
    );
    match result.attempt {
        plan::SaveAttempt::Written { new_hash, bytes } => {
            checks.expect(
                new_hash == fsutil::sha256_hex(app_text.as_bytes()) && bytes == app_text.len(),
                "返回值报告的哈希与字节数与实际一致",
                &format!("{} B / {}", bytes, &new_hash[..16]),
            );
        }
        other => {
            checks.expect(false, "写入路径返回 Written", &format!("{other:?}"));
        }
    }
    Ok(())
}

/// 用例 3：编辑后内容与磁盘相同 → 拒写但归类为"无需写入"，不是冲突。
fn case_unchanged_is_rejected(checks: &mut Checks, root: &Path, source_text: &str) -> Result<()> {
    checks.case("conflict.unchanged");
    let path = root.join("conflict/unchanged/main.tex");
    let loaded = stage(&path, source_text)?;
    let result = attempt_save(&loaded, source_text)?;
    checks.expect(
        matches!(
            result.attempt,
            plan::SaveAttempt::Rejected(plan::Rejection::Unchanged)
        ),
        "内容未变时归类为 unchanged",
        &format!("{:?}", result.attempt),
    );
    checks.expect(
        result.disk_text == source_text,
        "磁盘内容未被动过",
        &format!("磁盘 {} B", result.disk_text.len()),
    );
    Ok(())
}

/// 用例 4：编辑期间文件被外部删除 → 拒写并报告 removed。
fn case_removed_file(checks: &mut Checks, root: &Path, source_text: &str) -> Result<()> {
    checks.case("conflict.removed");
    let path = root.join("conflict/removed/main.tex");
    let loaded = stage(&path, source_text)?;
    std::fs::remove_file(&path).map_err(crate::error::io_context(&path))?;

    let result = attempt_save(&loaded, &format!("{source_text}% app\n"))?;
    checks.expect(
        matches!(
            result.attempt,
            plan::SaveAttempt::Rejected(plan::Rejection::Removed)
        ),
        "文件被删除时归类为 removed",
        &format!("{:?}", result.attempt),
    );
    checks.expect(
        !path.exists(),
        "应用没有偷偷把文件重建出来",
        &format!("exists={}", path.exists()),
    );
    let rendered = result
        .report
        .map(|report| report.render())
        .unwrap_or_default();
    checks.expect(
        rendered.contains("removed"),
        "报告写明判定为 removed",
        &rendered.trim().replace('\n', " | "),
    );
    Ok(())
}

/// 把源码写入磁盘并加载为受跟踪源码。
fn stage(path: &Path, text: &str) -> Result<plan::TrackedSource> {
    fsutil::write_file(path, text.as_bytes())?;
    plan::TrackedSource::load(path)
}

/// 尝试保存并收集报告与磁盘内容。
fn attempt_save(loaded: &plan::TrackedSource, new_text: &str) -> Result<Attempt> {
    let attempt = loaded.try_save(new_text)?;
    let report = match &attempt {
        plan::SaveAttempt::Rejected(rejection) => {
            Some(plan::ConflictReport::build(loaded, rejection.clone())?)
        }
        plan::SaveAttempt::Written { .. } => None,
    };
    let disk_text = fsutil::read_file(&loaded.path)
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default();
    Ok(Attempt {
        attempt,
        report,
        disk_text,
    })
}

/// 判据 3 用的源码：直接复用判据 1 的 LaTeX 渲染结果，保证"外部改动的是同一份源码"。
fn fixture_source() -> String {
    let (editor, _) = fixture::editor();
    render::latex(editor.document())
}
