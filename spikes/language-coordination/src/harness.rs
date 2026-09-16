//! 逐用例证据 harness。
//!
//! 项目已经因为"只看总数"吃过亏，所以这里强制每个用例单独打印一行
//! 期望 / 实际 / 结果；任何一条不通过都会让进程以非零码退出。

/// 用例类型。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CaseKind {
    /// 成功夹具：期望行为成立。
    Success,
    /// 失败夹具：期望被拒绝或被检测出来。
    Failure,
    /// 对照：故意写坏的实现，用来证明断言有牙齿。
    Control,
}

impl CaseKind {
    /// 中文标签。
    const fn label(self) -> &'static str {
        match self {
            Self::Success => "成功夹具",
            Self::Failure => "失败夹具",
            Self::Control => "对照",
        }
    }
}

/// 单个证据用例。
#[derive(Clone, Debug)]
pub(crate) struct Case {
    /// 判据编号，如 `C1`。
    pub criterion: &'static str,
    /// 用例类型。
    pub kind: CaseKind,
    /// 用例名。
    pub name: String,
    /// 期望。
    pub expected: String,
    /// 实际。
    pub actual: String,
    /// 是否通过。
    pub ok: bool,
    /// 说明 / 关键数字。
    pub detail: String,
}

/// 证据收集器。
#[derive(Clone, Debug, Default)]
pub(crate) struct Harness {
    cases: Vec<Case>,
    sections: Vec<(&'static str, &'static str)>,
}

impl Harness {
    /// 新建。
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// 打印并登记一个判据分组。
    pub(crate) fn section(&mut self, criterion: &'static str, title: &'static str) {
        if !self.sections.iter().any(|(id, _)| *id == criterion) {
            self.sections.push((criterion, title));
        }
        println!("\n=== {criterion} {title} ===");
    }

    /// 登记并打印一个用例。
    pub(crate) fn case(
        &mut self,
        criterion: &'static str,
        kind: CaseKind,
        name: &str,
        expected: &str,
        actual: &str,
        ok: bool,
        detail: &str,
    ) {
        let result = if ok { "PASS" } else { "FAIL" };
        println!("[{criterion} {}] {name} -> {result}", kind.label());
        println!("    期望: {expected}");
        println!("    实际: {actual}");
        if !detail.is_empty() {
            println!("    说明: {detail}");
        }
        self.cases.push(Case {
            criterion,
            kind,
            name: name.to_owned(),
            expected: expected.to_owned(),
            actual: actual.to_owned(),
            ok,
            detail: detail.to_owned(),
        });
    }

    /// 打印一条补充说明。
    pub(crate) fn note(&mut self, criterion: &'static str, text: &str) {
        println!("[{criterion} NOTE] {text}");
    }

    /// 某判据是否全通过。
    pub(crate) fn criterion_pass(&self, criterion: &str) -> bool {
        self.cases
            .iter()
            .filter(|case| case.criterion == criterion)
            .all(|case| case.ok)
    }

    /// 打印汇总并返回总体是否通过。
    pub(crate) fn finish(&self) -> bool {
        println!("\n=== 判据汇总（逐用例） ===");
        let mut all_ok = true;
        for (criterion, title) in &self.sections {
            let cases: Vec<&Case> = self
                .cases
                .iter()
                .filter(|case| case.criterion == *criterion)
                .collect();
            let failed = cases.iter().filter(|case| !case.ok).count();
            let passed = cases.len() - failed;
            let result = if failed == 0 { "PASS" } else { "FAIL" };
            if failed > 0 {
                all_ok = false;
            }
            println!(
                "{criterion} {title}: cases={} pass={passed} fail={failed} -> {result}",
                cases.len()
            );
            for case in cases.iter().filter(|case| !case.ok) {
                println!("    FAILED: {} 期望={} 实际={}", case.name, case.expected, case.actual);
            }
        }
        let total_failed = self.cases.iter().filter(|case| !case.ok).count();
        let total = self.cases.len();
        println!(
            "总计: cases={total} pass={} fail={total_failed} -> {}",
            total - total_failed,
            if all_ok { "PASS" } else { "FAIL" }
        );
        all_ok
    }
}
