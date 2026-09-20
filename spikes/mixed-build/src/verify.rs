//! 逐夹具核验：每个夹具单独断言，不汇总成总数。
//!
//! 核验分成两类来源：
//! - **引擎读回**：LaTeX 的 `.aux`、Typst 的编号探针与内省；
//! - **独立证据**：逐页文本抽取（`pdftotext` / 布局 `FrameItem::Text`）、链接注解、栅格图像清单。
//!
//! 标记串（`MK<符号>`）被放进被标记元素自身，所以"标记出现的物理页"就是该元素的物理页；
//! 用它去比对引擎读回的页码，才是独立核验而不是自洽。

use std::path::Path;

use crate::build::{self, BuildOutput};
use crate::diag::{Evidence, Expect, Outcome};
use crate::fixtures::Fixture;
use crate::generate::marker_for;
use crate::ir::Dialect;
use crate::pdf_evidence::raster_images;
use crate::plan;

/// 去掉全部空白，便于跨换行匹配中文与数字。
fn compact(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

/// 某段文本出现在第几页（1 基）。
fn page_of(pages: &[String], needle: &str) -> Option<usize> {
    pages
        .iter()
        .position(|page| compact(page).contains(needle))
        .map(|index| index + 1)
}

/// 某段文本在整篇里出现多少次。
fn count_in(pages: &[String], needle: &str) -> usize {
    pages
        .iter()
        .map(|page| compact(page).matches(needle).count())
        .sum()
}

/// 运行并核验一个夹具。
#[allow(clippy::too_many_lines)] // 断言调度，集中打印证据
pub(crate) fn run(fixture: &Fixture, host: Dialect, dir: &Path) -> Evidence {
    let project = (fixture.make)(host);
    let mut evidence = Evidence {
        fixture: fixture.id.to_string(),
        host: host.name(),
        expect: fixture.expect,
        outcome: Outcome::Success,
        checks: Vec::new(),
        dir: dir.display().to_string(),
    };
    let plan = match plan::plan(&project) {
        Ok(plan) => plan,
        Err(problems) => {
            evidence.outcome = Outcome::PlanRejected;
            evidence.check("计划阶段拒绝", true, format!("{} 条诊断", problems.len()));
            evidence.check(
                "诊断可读",
                problems.iter().all(|problem| !problem.message.is_empty()),
                problems
                    .iter()
                    .map(|problem| problem.render())
                    .collect::<Vec<_>>()
                    .join(" ｜ "),
            );
            let codes: Vec<&str> = problems.iter().map(|problem| problem.code).collect();
            for code in fixture.expect_codes {
                evidence.check(
                    format!("命中预期诊断码 `{code}`"),
                    codes.contains(code),
                    format!("实际诊断码 {codes:?}"),
                );
            }
            evidence.check(
                "未发布最终产物",
                !dir.join("final.pdf").exists(),
                "final.pdf 不存在",
            );
            return evidence;
        }
    };
    evidence.check("计划接受", true, plan.steps.join(" ｜ "));

    let outcome = build::build(&project, &plan, dir, build::MAX_ROUNDS);
    evidence.outcome = outcome.outcome.clone();
    if !outcome.diagnostics.is_empty() {
        evidence.check(
            "构建诊断",
            outcome.outcome != Outcome::Success,
            outcome
                .diagnostics
                .iter()
                .map(|problem| problem.render())
                .collect::<Vec<_>>()
                .join(" ｜ "),
        );
    }
    let round_summary = outcome
        .rounds
        .iter()
        .map(|round| {
            format!(
                "第{}轮 用[{}] 观测[{}]{}",
                round.index,
                round.used,
                round.observed,
                if round.converged { " 收敛" } else { "" }
            )
        })
        .collect::<Vec<_>>()
        .join(" ｜ ");
    evidence.check("轮数记录", true, round_summary);
    if !outcome.component_log.is_empty() {
        evidence.check("组件构建", true, outcome.component_log.join(" ｜ "));
    }

    if fixture.expect == Expect::Rejected {
        evidence.check(
            "被拒绝或编译失败",
            outcome.outcome != Outcome::Success,
            format!("结果 {}", outcome.outcome.name()),
        );
        let codes: Vec<&str> = outcome
            .diagnostics
            .iter()
            .map(|problem| problem.code)
            .collect();
        evidence.check(
            "诊断可读",
            outcome
                .diagnostics
                .iter()
                .all(|problem| !problem.message.is_empty()),
            outcome
                .diagnostics
                .iter()
                .map(|problem| problem.render())
                .collect::<Vec<_>>()
                .join(" ｜ "),
        );
        for code in fixture.expect_codes {
            evidence.check(
                format!("命中预期诊断码 `{code}`"),
                codes.contains(code),
                format!("实际诊断码 {codes:?}"),
            );
        }
        evidence.check(
            "未发布最终产物",
            !dir.join("final.pdf").exists(),
            "final.pdf 不存在",
        );
        return evidence;
    }

    // ---- 成功夹具 ----
    let Some(observation) = outcome.observation.as_ref() else {
        evidence.check("存在宿主观测", false, "没有观测结果");
        return evidence;
    };
    let final_pdf = dir.join("final.pdf");
    evidence.check(
        "发布最终产物",
        final_pdf.exists()
            && std::fs::metadata(&final_pdf)
                .map(|meta| meta.len() > 0)
                .unwrap_or(false),
        format!(
            "{} 字节 {}",
            std::fs::metadata(&final_pdf)
                .map(|meta| meta.len())
                .unwrap_or(0),
            final_pdf.display()
        ),
    );
    evidence.check(
        "宿主源码存在",
        observation.source_path.exists(),
        format!("{}", observation.source_path.display()),
    );
    evidence.check(
        "页数",
        observation.pages >= 1,
        format!("{} 页", observation.pages),
    );

    match fixture.id {
        "T1-longtable" => check_longtable(&mut evidence, observation),
        "E1-equation" => check_equations(&mut evidence, observation),
        "G1-plot-native" => check_plot(&mut evidence, observation, "fig:native", host, dir),
        "G2-plot-foreign" => {
            check_plot(&mut evidence, observation, "fig:foreign", host, dir);
            check_foreign_vector(&mut evidence, &outcome, dir);
        }
        "M1-macro" => check_macros(&mut evidence, observation, &outcome),
        "S1-scope" => check_scope(&mut evidence, observation, false),
        "S1-scope-control" => check_scope(&mut evidence, observation, fixture.leak_expected),
        "R1-refs" => check_refs(&mut evidence, observation, &outcome),
        _ => {
            evidence.check("已知夹具", false, "核验分支缺失");
        }
    }
    let _ = host;
    evidence
}

/// 跨页表格：跨页、表头重复、编号与页码。
fn check_longtable(evidence: &mut Evidence, observation: &crate::build::Observation) {
    let pages = &observation.text_pages;
    let filler_pages = pages
        .iter()
        .filter(|page| compact(page).contains("填充"))
        .count();
    evidence.check(
        "表格跨页",
        filler_pages >= 2,
        format!("含填充行的页数 {filler_pages} / 总页数 {}", pages.len()),
    );
    let header_pages = pages
        .iter()
        .filter(|page| compact(page).contains("名称"))
        .count();
    evidence.check(
        "表头在续页重复",
        header_pages >= 2,
        format!("含表头的页数 {header_pages}"),
    );
    let marker = marker_for("tab:long");
    let marker_page = page_of(pages, &marker);
    let label_page = observation.label_pages.get("tab:long").copied();
    evidence.check(
        "表格页码：读回值 = 标记实际所在页",
        marker_page.is_some() && marker_page.map(|page| page as u64) == label_page,
        format!("标记 `{marker}` 在 {marker_page:?}，读回 {label_page:?}"),
    );
    let number = observation
        .label_numbers
        .get("tab:long")
        .cloned()
        .unwrap_or_default();
    evidence.check("表格编号读回", number == "1", format!("编号 {number:?}"));
    let text = compact(&pages.join(""));
    evidence.check(
        "正文引用出现在产物中",
        text.contains(&format!("表{number}")),
        format!("查找 `表{number}`"),
    );
    if let Some(page) = marker_page {
        evidence.check(
            "页码引用与真实页一致",
            text.contains(&format!("第{page}页")),
            format!("查找 `第{page}页`"),
        );
    }
}

/// 公式：编号、引用、页码。
fn check_equations(evidence: &mut Evidence, observation: &crate::build::Observation) {
    let pages = &observation.text_pages;
    let mass = observation
        .label_numbers
        .get("eq:mass")
        .cloned()
        .unwrap_or_default();
    let integral = observation
        .label_numbers
        .get("eq:int")
        .cloned()
        .unwrap_or_default();
    evidence.check(
        "两个公式编号正确且互异",
        mass == "1" && integral == "2",
        format!("eq:mass={mass:?} eq:int={integral:?}"),
    );
    for (label, expected) in [("eq:mass", &mass), ("eq:int", &integral)] {
        let marker = marker_for(label);
        let marker_page = page_of(pages, &marker);
        let label_page = observation.label_pages.get(label).copied();
        evidence.check(
            format!("{label} 页码：读回值 = 标记实际所在页"),
            marker_page.is_some() && marker_page.map(|page| page as u64) == label_page,
            format!("标记 `{marker}` 在 {marker_page:?}，读回 {label_page:?}"),
        );
        let text = compact(&pages.join(""));
        evidence.check(
            format!("{label} 标记与编号都在产物里"),
            marker_page.is_some() && text.contains(expected.as_str()),
            format!("标记 {marker_page:?}，编号 {expected:?}"),
        );
    }
    let text = compact(&pages.join(""));
    if let Some(page) = page_of(pages, &marker_for("eq:int")) {
        evidence.check(
            "eq:int 的页码引用与真实页一致",
            text.contains(&format!("第{page}页")),
            format!("查找 `第{page}页`"),
        );
    }
}

/// 图：编号、页码、矢量性。
fn check_plot(
    evidence: &mut Evidence,
    observation: &crate::build::Observation,
    label: &str,
    host: Dialect,
    dir: &Path,
) {
    let pages = &observation.text_pages;
    let marker = marker_for(label);
    let marker_page = page_of(pages, &marker);
    let label_page = observation.label_pages.get(label).copied();
    evidence.check(
        format!("{label} 页码：读回值 = 标记实际所在页"),
        marker_page.is_some() && marker_page.map(|page| page as u64) == label_page,
        format!("标记 `{marker}` 在 {marker_page:?}，读回 {label_page:?}"),
    );
    let number = observation
        .label_numbers
        .get(label)
        .cloned()
        .unwrap_or_default();
    evidence.check(
        format!("{label} 编号读回"),
        number == "1",
        format!("编号 {number:?}"),
    );
    let text = compact(&pages.join(""));
    evidence.check(
        format!("{label} 引用出现在产物中"),
        text.contains(&format!("图{number}")),
        format!("查找 `图{number}`"),
    );
    // 矢量性：最终件里不得出现整页位图。
    let raster = raster_images(&dir.join("final.pdf"));
    evidence.check(
        format!("{label} 最终件为矢量（无栅格图像）"),
        matches!(raster, Ok(0)),
        format!(
            "pdfimages 列出 {raster:?} 个栅格图像（宿主 {}）",
            host.name()
        ),
    );
}

/// 跨引擎嵌入：组件是独立权威源码 + 矢量载体。
fn check_foreign_vector(evidence: &mut Evidence, outcome: &BuildOutput, dir: &Path) {
    let Some(path) = outcome.component_artifacts.get("g2-draw") else {
        evidence.check("组件有矢量产物", false, "没有 g2-draw 产物");
        return;
    };
    let size = std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
    evidence.check(
        "组件有非空矢量产物",
        size > 0,
        format!("{} 字节 {}", size, path.display()),
    );
    let svg = dir.join("g2-draw.svg");
    if svg.exists() {
        let has_paths = std::fs::read_to_string(&svg)
            .map(|text| text.matches("<path").count())
            .unwrap_or(0);
        evidence.check(
            "Typst 组件 SVG 含路径数据（不是位图）",
            has_paths > 0,
            format!("{has_paths} 条 <path>"),
        );
    } else {
        evidence.check(
            "LaTeX 组件产物为矢量（无 SVG 中间件，直接 PDF）",
            matches!(raster_images(path), Ok(0)),
            "由 xelatex 直接产出矢量 PDF",
        );
    }
    evidence.check(
        "组件独立编译产物是矢量",
        matches!(raster_images(path), Ok(0)),
        format!("pdfimages 结果 {:?}", raster_images(path)),
    );
}

/// 宏：宿主自有宏与跨语言宏都按语义展开。
fn check_macros(
    evidence: &mut Evidence,
    observation: &crate::build::Observation,
    outcome: &BuildOutput,
) {
    let text = compact(&observation.text_pages.join(""));
    evidence.check(
        "宿主自有宏 doubles(21) = 42",
        text.matches("42").count() >= 2,
        format!("产物中 42 出现 {} 次", text.matches("42").count()),
    );
    evidence.check(
        "跨语言宏在宿主侧按合约生效",
        text.contains("宿主自有宏：42；跨语言宏：42"),
        compact(&text),
    );
    evidence.check(
        "宏名未被原样输出（确实展开）",
        !text.contains("doubles") && !text.contains("triple"),
        "查找 `doubles`/`triple`".to_string(),
    );
    let foreign = outcome
        .component_text
        .get("m-lib")
        .cloned()
        .unwrap_or_default();
    evidence.check(
        "外语组件侧独立编译也得到 42（语义一致）",
        compact(&foreign).contains("外语侧结果：42"),
        foreign,
    );
}

/// 作用域：两个取值各自出现一次；对照实现应当出现泄漏。
fn check_scope(
    evidence: &mut Evidence,
    observation: &crate::build::Observation,
    leak_expected: bool,
) {
    let pages = &observation.text_pages;
    let a = count_in(pages, "取值-A");
    let b = count_in(pages, "取值-B");
    if leak_expected {
        evidence.check(
            "对照实现确实泄漏（s-b 取到了 s-a 的值）",
            a == 2,
            format!("取值-A 出现 {a} 次，取值-B 出现 {b} 次"),
        );
    } else {
        evidence.check(
            "两个作用域取值各自出现一次（无泄漏）",
            a == 1 && b == 1,
            format!("取值-A {a} 次，取值-B {b} 次"),
        );
    }
}

/// 双向引用：编号、最终页码、链接。
fn check_refs(
    evidence: &mut Evidence,
    observation: &crate::build::Observation,
    outcome: &BuildOutput,
) {
    let pages = &observation.text_pages;
    let host_number = observation
        .label_numbers
        .get("eq:host")
        .cloned()
        .unwrap_or_default();
    let _ = &host_number;
    evidence.check(
        "宿主公式编号读回",
        host_number == "1",
        format!("eq:host={host_number:?}"),
    );
    let host_page = page_of(pages, &marker_for("eq:host"));
    evidence.check(
        "宿主公式页码：读回值 = 标记实际所在页",
        host_page.is_some()
            && host_page.map(|page| page as u64) == observation.label_pages.get("eq:host").copied(),
        format!(
            "标记在 {host_page:?}，读回 {:?}",
            observation.label_pages.get("eq:host")
        ),
    );
    let foreign_page = page_of(pages, &marker_for("fig:bridge"));
    evidence.check(
        "跨引擎图页码：读回值 = 标记实际所在页",
        foreign_page.is_some()
            && foreign_page.map(|page| page as u64)
                == observation.label_pages.get("fig:bridge").copied(),
        format!(
            "标记在 {foreign_page:?}，读回 {:?}",
            observation.label_pages.get("fig:bridge")
        ),
    );
    let component_text = compact(
        &outcome
            .component_text
            .get("r-lib")
            .cloned()
            .unwrap_or_default(),
    );
    let component_number = parenthesized_number(&component_text).unwrap_or_default();
    evidence.check(
        "跨引擎符号编号：宿主引用值 = 组件自身引擎分配的编号",
        !component_number.is_empty()
            && compact(&pages.join(""))
                .contains(&format!("外语组件中的公式编号{component_number}")),
        format!(
            "组件自身编号 ({}), 宿主引用文本查找 `外语组件中的公式编号{}`",
            component_number, component_number
        ),
    );
    // 外语组件内部引用了宿主符号：组件独立编译的文本里应当出现宿主编号。
    let foreign_text = outcome
        .component_text
        .get("r-lib")
        .cloned()
        .unwrap_or_default();
    evidence.check(
        "外语组件内部引用宿主编号（反向引用）",
        compact(&foreign_text).contains(&format!("外语组件引用宿主：{host_number}")),
        foreign_text,
    );
    // 链接：必须存在指向宿主公式所在页的链接。
    let links = &observation.links;
    let target = observation.label_pages.get("eq:host").copied();
    evidence.check(
        "交付件里存在链接",
        !links.is_empty(),
        format!("{} 条链接：{links:?}", links.len()),
    );
    evidence.check(
        "存在指向宿主公式页的链接",
        target.is_some() && links.iter().any(|(_, page)| *page == target),
        format!("目标页 {target:?}，链接 {links:?}"),
    );
    let bad = links.iter().filter(|(_, page)| page.is_none()).count();
    evidence.check(
        "链接目标均可解析到交付件内的页",
        bad == 0,
        format!("无法解析的链接 {bad} 条"),
    );
    let external = observation
        .external_links
        .iter()
        .map(|(page, url)| format!("第{page}页→{url}"))
        .collect::<Vec<_>>();
    evidence.check(
        "外部 URL 链接（若有）也被记录",
        true,
        if external.is_empty() {
            "本夹具无外部 URL 链接".to_string()
        } else {
            external.join("，")
        },
    );
}

/// 取文本里第一个 `(数字)` 形式的编号（组件自身引擎分配的编号）。
fn parenthesized_number(text: &str) -> Option<String> {
    let bytes: Vec<char> = text.chars().collect();
    for (index, character) in bytes.iter().enumerate() {
        if *character != '(' {
            continue;
        }
        let mut digits = String::new();
        for next in bytes.iter().skip(index + 1) {
            if next.is_ascii_digit() {
                digits.push(*next);
            } else {
                break;
            }
        }
        if !digits.is_empty() && bytes.get(index + 1 + digits.len()) == Some(&')') {
            return Some(digits);
        }
    }
    None
}
