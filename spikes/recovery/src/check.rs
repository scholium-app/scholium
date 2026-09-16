//! 断言与证据记账。
//!
//! 本 spike 的结论按**每个用例**判定，不看总数：一个用例静默失效会被总数掩盖。
//! 因此每次断言都写一行 `[PASS]` / `[FAIL]`，并单独统计所属用例的通过情况，
//! 最后逐用例（而非逐断言）汇总才算通过。

/// 一个断言的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// 条件成立。
    Pass,
    /// 条件不成立。
    Fail,
}

/// 单个用例的统计。
#[derive(Debug, Clone)]
pub struct Case {
    /// 用例标识，例如 `wal.tail-truncated`。
    pub id: String,
    /// 该用例的断言通过数。
    pub passed: usize,
    /// 该用例的断言失败数。
    pub failed: usize,
    /// 失败断言的第一条说明，用于汇总时定位。
    pub first_failure: Option<String>,
}

impl Case {
    /// 用例是否全部通过。
    pub fn is_pass(&self) -> bool {
        self.failed == 0 && self.passed > 0
    }

    /// 该用例的断言总数。
    pub fn total(&self) -> usize {
        self.passed + self.failed
    }
}

/// 断言与证据收集器。
#[derive(Debug, Default)]
pub struct Checks {
    cases: Vec<Case>,
    current: Option<usize>,
}

impl Checks {
    /// 新建收集器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 进入一个用例；后续断言都记在它名下。
    pub fn case(&mut self, id: &str) {
        self.cases.push(Case {
            id: id.to_string(),
            passed: 0,
            failed: 0,
            first_failure: None,
        });
        self.current = Some(self.cases.len() - 1);
    }

    /// 断言 `condition` 成立，并打印该断言的行内证据。
    ///
    /// `detail` 是**已经算好的证据文本**，不是结论；打印它是为了失败时能直接看到数字。
    pub fn expect(&mut self, condition: bool, label: &str, detail: &str) -> bool {
        let verdict = if condition { Verdict::Pass } else { Verdict::Fail };
        let marker = match verdict {
            Verdict::Pass => "PASS",
            Verdict::Fail => "FAIL",
        };
        println!("  [{marker}] {label} — {detail}");

        // self.current 由 Self::case 保证在 expect 前 != None；直接忽略空状态而不是 panic。
        if let Some(index) = self.current {
            let case = &mut self.cases[index];
            match verdict {
                Verdict::Pass => case.passed += 1,
                Verdict::Fail => {
                    case.failed += 1;
                    if case.first_failure.is_none() {
                        case.first_failure = Some(format!("{label} — {detail}"));
                    }
                }
            }
        }
        condition
    }

    /// 仅打印一行证据，不参与判定（用于记录规模、耗时等上下文）。
    pub fn note(&self, label: &str, detail: &str) {
        println!("  [----] {label} — {detail}");
    }

    /// 逐用例汇总并返回是否存在失败。
    pub fn finish(&self) -> bool {
        println!("\n== 逐用例汇总 ==");
        let mut all_pass = true;
        for case in &self.cases {
            let verdict = if case.is_pass() { "PASS" } else { "FAIL" };
            if !case.is_pass() {
                all_pass = false;
            }
            let detail = match &case.first_failure {
                Some(first) => format!(" 首个失败: {first}"),
                None => String::new(),
            };
            println!(
                "  [{verdict}] {:<34} {}/{} 断言通过{}",
                case.id,
                case.passed,
                case.total(),
                detail
            );
        }
        if self.cases.is_empty() {
            println!("  [FAIL] 没有任何用例");
            all_pass = false;
        }
        all_pass
    }

    /// 已登记用例数。
    pub fn case_count(&self) -> usize {
        self.cases.len()
    }

    /// 逐用例汇总的纯文本形式，用于跨轮次对比。
    pub fn summary_text(&self) -> String {
        let mut out = String::new();
        for case in &self.cases {
            let verdict = if case.is_pass() { "PASS" } else { "FAIL" };
            out.push_str(&format!(
                "{verdict} {:<34} {}/{}\n",
                case.id,
                case.passed,
                case.total()
            ));
        }
        out
    }
}
