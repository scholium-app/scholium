//! 证据收集与输出。
//!
//! 每个用例单独一行 `[PASS]` / `[FAIL]`，结尾汇总并列出失败清单。判据要求 "逐个用例断言"，
//! 只看总数会掩盖个别用例静默失效——本项目的报告规范明确禁止这种写法。

/// 单个用例结果。
#[derive(Debug)]
struct Case {
    id: String,
    passed: bool,
    detail: String,
}

/// 证据收集器。
#[derive(Debug, Default)]
pub(crate) struct Evidence {
    cases: Vec<Case>,
}

impl Evidence {
    /// 空收集器。
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// 打印判据小标题。
    pub(crate) fn section(&mut self, title: &str) {
        println!("\n== {title} ==");
    }

    /// 打印不计入判据的说明行（夹具、环境、耗时等）。
    pub(crate) fn note(&self, text: &str) {
        println!("   {text}");
    }

    /// 记录一个用例结果并打印。
    pub(crate) fn check(&mut self, id: &str, passed: bool, detail: impl Into<String>) {
        let detail = detail.into();
        let mark = if passed { "PASS" } else { "FAIL" };
        println!("[{mark}] {id} :: {detail}");
        self.cases.push(Case {
            id: id.to_owned(),
            passed,
            detail,
        });
    }

    /// 用例总数。
    pub(crate) fn total(&self) -> usize {
        self.cases.len()
    }

    /// 通过数。
    pub(crate) fn passed(&self) -> usize {
        self.cases.iter().filter(|case| case.passed).count()
    }

    /// 打印汇总，返回失败数（用作进程退出码）。
    pub(crate) fn summary(&self) -> usize {
        let failed: Vec<&Case> = self.cases.iter().filter(|case| !case.passed).collect();
        println!("\n== 汇总 ==");
        println!(
            "用例总数 {}，通过 {}，失败 {}",
            self.total(),
            self.passed(),
            failed.len()
        );
        for case in &failed {
            println!("[FAIL] {} :: {}", case.id, case.detail);
        }
        failed.len()
    }
}
