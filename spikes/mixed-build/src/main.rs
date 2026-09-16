//! 阶段 0 第 7 项验证：混合构建 spike。
//!
//! `cargo run --release` 会依次运行全部夹具（两种宿主），逐夹具打印断言与真实运行证据，
//! 最后打印按夹具汇总的通过/失败表。产物（`.tex` / `.typ` / `.svg` / `.pdf`）写在 `out/`。
//!
//! 命令：
//! - `cargo run --release`            全部夹具
//! - `cargo run --release -- <id>`    只跑某个夹具（id 见夹具列表）

mod build;
mod diag;
mod fixtures;
mod generate;
mod generate_typst;
mod host;
mod ir;
mod latex;
mod plan;
mod typst_host;
mod verify;
mod world;

use std::path::PathBuf;

use diag::Evidence;
use ir::Dialect;

fn main() {
    let filter = std::env::args().nth(1);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out = root.join("out");

    println!("=== 混合构建 spike（阶段 0 第 7 项）===");
    println!(
        "仓库：{}\n产物目录：{}\n宿主：xelatex（TeX Live 2026）+ typst crate 0.15.1\n",
        root.display(),
        out.display()
    );

    let all = fixtures::all();
    let mut evidence: Vec<Evidence> = Vec::new();
    let mut ran = 0usize;
    for fixture in &all {
        if let Some(filter) = &filter
            && fixture.id != *filter
        {
            continue;
        }
        for host in fixture.hosts {
            let dir = out.join(format!("{}-{}", fixture.id, host.name()));
            let _ = std::fs::remove_dir_all(&dir);
            println!(
                "---- 夹具 {} [{}] / 宿主 {} / 期望 {} ----",
                fixture.id,
                fixture.category,
                host.name(),
                match fixture.expect {
                    diag::Expect::Success => "成功",
                    diag::Expect::Rejected => "被拒绝",
                }
            );
            println!("  意图：{}", fixture.note);
            let result = verify::run(fixture, *host, &dir);
            print_evidence(&result);
            evidence.push(result);
            ran += 1;
        }
    }
    if ran == 0 {
        println!("没有匹配的夹具：{filter:?}");
        return;
    }
    print_summary(&evidence);
}

/// 打印单个夹具的全部断言。
fn print_evidence(evidence: &Evidence) {
    println!("  产物目录：{}", evidence.dir);
    println!(
        "  结果：{}（期望 {}）",
        evidence.outcome.name(),
        match evidence.expect {
            diag::Expect::Success => "成功",
            diag::Expect::Rejected => "被拒绝",
        }
    );
    for check in &evidence.checks {
        println!(
            "    [{}] {}：{}",
            if check.ok { "PASS" } else { "FAIL" },
            check.name,
            check.detail
        );
    }
    println!(
        "  → 夹具判定：{}\n",
        if evidence.passed() { "Pass" } else { "Fail" }
    );
}

/// 打印逐夹具汇总表。
fn print_summary(evidence: &[Evidence]) {
    println!("=== 逐夹具汇总（每个夹具单独判定，不看总数）===");
    let mut passed = 0usize;
    let mut failed = Vec::new();
    for item in evidence {
        let ok = item.passed();
        if ok {
            passed += 1;
        } else {
            failed.push(format!(
                "{}-{}（{}）",
                item.fixture,
                item.host,
                item.outcome.name()
            ));
        }
        let failed_checks: Vec<String> = item
            .checks
            .iter()
            .filter(|check| !check.ok)
            .map(|check| check.name.clone())
            .collect();
        println!(
            "  {:<20} {:<6} {:<14} {:>3} 条断言  {}",
            item.fixture,
            item.host,
            item.outcome.name(),
            item.checks.len(),
            if ok {
                "Pass".to_string()
            } else {
                format!("Fail：{}", failed_checks.join("，"))
            }
        );
    }
    println!("\n汇总：{} / {} 个夹具运行通过", passed, evidence.len());
    if failed.is_empty() {
        println!("全部夹具通过。");
    } else {
        println!("失败：{}", failed.join("；"));
    }
    let latex_count = evidence
        .iter()
        .filter(|item| item.host == Dialect::Latex.name())
        .count();
    println!(
        "覆盖：LaTeX 宿主 {} 次、Typst 宿主 {} 次",
        latex_count,
        evidence.len() - latex_count
    );
}
