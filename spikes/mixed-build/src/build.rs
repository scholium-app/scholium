//! 混合构建驱动：有界多轮计划 + 双语工具链真实编译 + 产物读回。
//!
//! 管线对应 [混合源码与团队编辑](../../../../docs/MIXED_SOURCE_EDITING.md) 第 7 节：
//! `冻结输入快照 → 转换/依赖计划 → 外语组件构建 → 宿主排版 → 引用/布局桥接 → 最终验证`。
//!
//! 引用回流**只在轮数上限内**进行：解析表稳定即收敛；重复出现过的解析表视为振荡，
//! 立即停止并阻止正式产物发布。
//!
//! 本文件只负责"编排"：单轮组件构建见 [`build_components`]，单轮宿主装配见 [`assemble_host`]，
//! 产物读回与矢量导出在 [`crate::host`]。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::diag::{Diagnostic, Outcome};
use crate::generate::{self, Ctx, RefTable, Resolved};
use crate::generate_typst;
use crate::host::{component_files, export_component, observe_latex, observe_typst, signature};
use crate::ir::{Bridge, Dialect, Project};
use crate::latex;
use crate::plan::Plan;
use crate::typst_host;

/// 默认轮数上限。
pub(crate) const MAX_ROUNDS: usize = 4;

/// 一轮构建的记录。
pub(crate) struct Round {
    /// 轮次（1 基）。
    pub(crate) index: usize,
    /// 本轮使用的解析表签名。
    pub(crate) used: String,
    /// 本轮观测到的解析表签名。
    pub(crate) observed: String,
    /// 是否收敛。
    pub(crate) converged: bool,
}

/// 宿主编译后的观测结果。
#[derive(Default)]
pub(crate) struct Observation {
    /// 逐页文本。
    pub(crate) text_pages: Vec<String>,
    /// 标签 → 页码。
    pub(crate) label_pages: BTreeMap<String, u64>,
    /// 符号 → 编号。
    pub(crate) label_numbers: BTreeMap<String, String>,
    /// 链接：(所在页, 目标页)。
    pub(crate) links: Vec<(u64, Option<u64>)>,
    /// 外部 URL 链接：(所在页, URL)。
    pub(crate) external_links: Vec<(u64, String)>,
    /// 页数。
    pub(crate) pages: usize,
    /// 宿主源码路径。
    pub(crate) source_path: PathBuf,
    /// 宿主 PDF 路径（存在时）。
    pub(crate) pdf_path: PathBuf,
}

/// 构建结果。
pub(crate) struct BuildOutput {
    /// 结果分类。
    pub(crate) outcome: Outcome,
    /// 诊断。
    pub(crate) diagnostics: Vec<Diagnostic>,
    /// 每轮记录。
    pub(crate) rounds: Vec<Round>,
    /// 成功时的宿主观测。
    pub(crate) observation: Option<Observation>,
    /// 组件构建日志。
    pub(crate) component_log: Vec<String>,
    /// 组件独立编译后的整篇文本（用于断言外语组件语义）。
    pub(crate) component_text: BTreeMap<String, String>,
    /// 组件矢量产物路径。
    pub(crate) component_artifacts: BTreeMap<String, PathBuf>,
}

/// 组件构建的中间观测。
#[derive(Default)]
struct ComponentObserved {
    numbers: BTreeMap<String, String>,
    pages: BTreeMap<String, u64>,
    text: String,
}

/// 宿主侧失败后应归入哪个结果：普通失败，还是达到轮数/引擎收敛上限。
enum HostStop {
    Failed,
    NotConverged,
}

fn initial_output() -> BuildOutput {
    BuildOutput {
        outcome: Outcome::NotConverged,
        diagnostics: Vec::new(),
        rounds: Vec::new(),
        observation: None,
        component_log: Vec::new(),
        component_text: BTreeMap::new(),
        component_artifacts: BTreeMap::new(),
    }
}

/// 执行混合构建。
pub(crate) fn build(project: &Project, plan: &Plan, dir: &Path, max_rounds: usize) -> BuildOutput {
    let mut output = initial_output();
    if let Err(error) = std::fs::create_dir_all(dir) {
        output.diagnostics.push(Diagnostic::new(
            "io",
            format!("无法创建产物目录 {}：{error}", dir.display()),
        ));
        output.outcome = Outcome::HostFailed;
        return output;
    }

    let probes: Vec<String> = plan.symbols.keys().cloned().collect();
    let anchors: Vec<String> = project
        .components
        .iter()
        .filter(|component| matches!(component.bridge, Bridge::Vector))
        .map(|component| format!("comp:{}", component.id))
        .collect();

    let mut table: RefTable = RefTable::new();
    let mut seen: Vec<String> = Vec::new();

    for round in 1..=max_rounds {
        let used = signature(&table);
        let mut next: RefTable = RefTable::new();

        if let Err(problem) =
            build_components(project, plan, dir, &table, &probes, &mut next, &mut output)
        {
            output.diagnostics.push(problem);
            output.outcome = Outcome::HostFailed;
            return output;
        }

        let observation = match assemble_host(
            project,
            plan,
            dir,
            &table,
            &probes,
            &anchors,
            &mut next,
            &mut output,
        ) {
            Ok(observation) => observation,
            Err((stop, problem)) => {
                output.diagnostics.push(problem);
                output.outcome = match stop {
                    HostStop::Failed => Outcome::HostFailed,
                    HostStop::NotConverged => Outcome::NotConverged,
                };
                return output;
            }
        };

        let observed_signature = signature(&next);
        let converged = observed_signature == used;
        output.rounds.push(Round {
            index: round,
            used: used.clone(),
            observed: observed_signature.clone(),
            converged,
        });
        if converged {
            // 收敛：发布最终产物。
            let final_path = dir.join("final.pdf");
            let _ = std::fs::copy(&observation.pdf_path, &final_path);
            output.observation = Some(observation);
            output.outcome = Outcome::Success;
            return output;
        }
        if let Some(previous) = seen.iter().position(|entry| *entry == observed_signature) {
            output.diagnostics.push(
                Diagnostic::new(
                    "reference-oscillation",
                    format!(
                        "引用反馈振荡：第 {} 轮与第 {} 轮解析出同一张引用表，但中间一轮不同，\
                         说明存在周期性反馈回路",
                        previous + 1,
                        round
                    ),
                )
                .hint(format!(
                    "已在第 {round} 轮停止（上限 {max_rounds} 轮），不发布正式产物"
                )),
            );
            record_rounds(&mut output);
            output.outcome = Outcome::NotConverged;
            return output;
        }
        seen.push(observed_signature);
        table = next;
    }

    output.diagnostics.push(
        Diagnostic::new(
            "reference-round-limit",
            format!("达到 {max_rounds} 轮上限仍未收敛"),
        )
        .hint("正式输出被阻止；快速预览可保留带诊断的旧结果"),
    );
    record_rounds(&mut output);
    output.outcome = Outcome::NotConverged;
    output
}

/// 把每一轮的"用—观测"对照写进诊断，作为振荡的证据。
fn record_rounds(output: &mut BuildOutput) {
    for record in &output.rounds {
        output.diagnostics.push(Diagnostic::new(
            "reference-round",
            format!(
                "第 {} 轮：使用 {} / 观测 {}",
                record.index, record.used, record.observed
            ),
        ));
    }
}

/// 单轮：构建全部组件（同方言片段 + 外语独立文档），并读回组件自有符号。
fn build_components(
    project: &Project,
    plan: &Plan,
    dir: &Path,
    table: &RefTable,
    probes: &[String],
    next: &mut RefTable,
    output: &mut BuildOutput,
) -> Result<(), Diagnostic> {
    for component in &project.components {
        let ctx = Ctx {
            project,
            plan,
            table,
            dialect: component.dialect,
            owner: &component.id,
            unscoped: project.unscoped_control,
        };
        if matches!(component.bridge, Bridge::Include | Bridge::Convert) {
            let ctx = Ctx {
                dialect: project.host,
                ..ctx
            };
            // 同方言：只产出可被宿主 `\input` / `#include` 的源码片段。
            let (path, source) = match project.host {
                Dialect::Latex => (
                    dir.join(format!("{}.tex", component.id)),
                    generate::include_fragment(&ctx, component),
                ),
                Dialect::Typst => (
                    dir.join(format!("{}.typ", component.id)),
                    generate_typst::include_fragment(&ctx, component),
                ),
            };
            std::fs::write(&path, source?).map_err(|error| {
                Diagnostic::new("io", format!("写入 {} 失败：{error}", path.display()))
            })?;
            continue;
        }
        let (path, source) = match component.dialect {
            Dialect::Latex => (
                dir.join(format!("{}.tex", component.id)),
                generate::standalone_source(&ctx, component),
            ),
            Dialect::Typst => (
                dir.join(format!("{}.typ", component.id)),
                generate_typst::standalone_source(&ctx, component),
            ),
        };
        let source = source?;
        std::fs::write(&path, &source).map_err(|error| {
            Diagnostic::new("io", format!("写入 {} 失败：{error}", path.display()))
        })?;
        let observed = match component.dialect {
            Dialect::Latex => compile_latex_component(component, dir, output)?,
            Dialect::Typst => compile_typst_component(component, dir, &source, probes, output)?,
        };
        output.component_text.insert(
            component.id.clone(),
            format!("{}（{}）", observed.text, component.dialect.name()),
        );
        if matches!(component.bridge, Bridge::Vector) {
            crate::vector_pdf::overlay::write(project, &component.id, dir)?;
            output.component_artifacts.insert(
                component.id.clone(),
                dir.join(format!("{}.pdf", component.id)),
            );
        }
        for symbol in plan
            .symbols
            .values()
            .filter(|symbol| symbol.owner == component.id)
        {
            let number = observed
                .numbers
                .get(&symbol.id)
                .cloned()
                .unwrap_or_else(|| "?".to_string());
            let page = observed.pages.get(&symbol.id).copied().or(Some(1));
            next.insert(
                symbol.id.clone(),
                Resolved {
                    number,
                    page,
                    owner: component.id.clone(),
                },
            );
        }
    }
    Ok(())
}

/// 编译一个 LaTeX 组件（独立文档），拒绝跨页组件。
fn compile_latex_component(
    component: &crate::ir::Component,
    dir: &Path,
    output: &mut BuildOutput,
) -> Result<ComponentObserved, Diagnostic> {
    let run = latex::compile(dir, &component.id);
    if !run.ok {
        let reason = run.diagnostics.join("；");
        output
            .component_log
            .push(format!("组件 {} LaTeX 编译失败：{reason}", component.id));
        return Err(Diagnostic::new(
            "component-compile-failed",
            format!("外语组件 `{}`（LaTeX）编译失败：{reason}", component.id),
        ));
    }
    if run.pages > 1 {
        return Err(Diagnostic::new(
            "component-multipage-unsupported",
            format!(
                "组件 `{}` 编译出 {} 页；当前受支持载体只有单页矢量嵌入",
                component.id, run.pages
            ),
        ));
    }
    output.component_log.push(format!(
        "组件 {} LaTeX 编译成功，{} 页，产物 {}.pdf",
        component.id, run.pages, component.id
    ));
    if component.placement == crate::ir::Placement::Inline {
        crate::inline_vector::latex_depth(dir, &component.id)?;
    }
    Ok(ComponentObserved {
        numbers: run
            .labels
            .iter()
            .map(|(key, value)| (key.clone(), value.0.clone()))
            .collect(),
        pages: run
            .labels
            .iter()
            .map(|(key, value)| (key.clone(), value.1))
            .collect(),
        text: run.text_pages.join("\n"),
    })
}

/// 编译一个 Typst 组件并导出矢量载体。
fn compile_typst_component(
    component: &crate::ir::Component,
    dir: &Path,
    source: &str,
    probes: &[String],
    output: &mut BuildOutput,
) -> Result<ComponentObserved, Diagnostic> {
    let run = typst_host::compile(source, &[], probes);
    if !run.ok {
        let message = run.errors.join("；");
        output
            .component_log
            .push(format!("组件 {} Typst 编译失败：{message}", component.id));
        return Err(Diagnostic::new(
            "component-compile-failed",
            format!("外语组件 `{}`（Typst）编译失败：{message}", component.id),
        ));
    }
    let artifact = export_component(dir, component, &run)?;
    if component.placement == crate::ir::Placement::Inline {
        let depth = run
            .inline_depth
            .ok_or_else(|| Diagnostic::new("inline-metrics", "missing Typst line baseline"))?;
        std::fs::write(
            dir.join(format!("{}-depth.json", component.id)),
            depth.to_string(),
        )
        .map_err(|e| Diagnostic::new("inline-metrics", e.to_string()))?;
    }
    output
        .component_log
        .push(format!("组件 {} Typst 编译成功，{artifact}", component.id));
    Ok(ComponentObserved {
        numbers: run.numbers.clone(),
        pages: run.label_pages.clone(),
        text: run.all_text(),
    })
}

/// 单轮：生成并编译宿主，读回宿主侧符号与交叉引用页面。
#[allow(clippy::too_many_arguments)] // 单轮装配需要项目/计划/目录/解析表/探针/锚点/状态
fn assemble_host(
    project: &Project,
    plan: &Plan,
    dir: &Path,
    table: &RefTable,
    probes: &[String],
    anchors: &[String],
    next: &mut RefTable,
    output: &mut BuildOutput,
) -> Result<Observation, (HostStop, Diagnostic)> {
    let ctx = Ctx {
        project,
        plan,
        table,
        dialect: project.host,
        owner: "host",
        unscoped: project.unscoped_control,
    };
    let (source_path, source) = match plan.host {
        Dialect::Latex => (
            dir.join("main.tex"),
            generate::host_source(&ctx).map_err(|problem| vec![problem]),
        ),
        Dialect::Typst => (
            dir.join("main.typ"),
            generate_typst::host_source(&ctx).map_err(|problem| vec![problem]),
        ),
    };
    let source = source.map_err(|problems| {
        (
            HostStop::Failed,
            problems
                .into_iter()
                .next()
                .unwrap_or_else(|| Diagnostic::new("host-generate", "宿主源码生成失败")),
        )
    })?;
    let _ = std::fs::write(&source_path, &source);

    match plan.host {
        Dialect::Latex => {
            observe_latex(dir, project, plan, next).map_err(|problem| (HostStop::Failed, problem))
        }
        Dialect::Typst => {
            let mut typst_probes = probes.to_vec();
            typst_probes.extend(anchors.iter().cloned());
            // 组件产物必须进入虚拟文件系统：`image("x.pdf")` 由 World 提供字节。
            let files = component_files(dir, project);
            let run = typst_host::compile(&source, &files, &typst_probes);
            if run.non_convergent() {
                return Err((
                    HostStop::NotConverged,
                    Diagnostic::new(
                        "engine-non-convergence",
                        format!(
                            "Typst 引擎在 {} 次内省后仍未收敛：{}",
                            typst::introspection::MAX_ITERS,
                            run.warnings.join("；")
                        ),
                    )
                    .hint("引擎级不收敛被当作硬失败，不发布正式产物"),
                ));
            }
            if !run.warnings.is_empty() {
                output.component_log.push(format!(
                    "宿主 Typst 警告 {} 条：{}",
                    run.warnings.len(),
                    run.warnings.join(" | ")
                ));
            }
            if !run.ok {
                return Err((
                    HostStop::Failed,
                    Diagnostic::new(
                        "host-compile-failed",
                        format!("宿主 Typst 编译失败：{}", run.errors.join("；")),
                    ),
                ));
            }
            observe_typst(&run, dir, project, plan, next)
                .map_err(|problem| (HostStop::Failed, problem))
        }
    }
}
