//! 诊断与证据：所有拒绝路径都必须给出可读诊断，所有判据都必须逐条打印。

/// 一条可读诊断。`code` 是稳定短码，便于报告与测试断言。
#[derive(Clone, Debug)]
pub(crate) struct Diagnostic {
    /// 稳定短码，例如 `macro-undeclared`。
    pub(crate) code: &'static str,
    /// 人可读的问题描述（含具体符号与来源）。
    pub(crate) message: String,
    /// 修复提示。
    pub(crate) hint: Option<String>,
}

impl Diagnostic {
    /// 构造诊断。
    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            hint: None,
        }
    }

    /// 附加修复提示。
    pub(crate) fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// 单行渲染，供日志打印。
    pub(crate) fn render(&self) -> String {
        match &self.hint {
            Some(hint) => format!("[{}] {}（提示：{}）", self.code, self.message, hint),
            None => format!("[{}] {}", self.code, self.message),
        }
    }
}

/// 一条断言。
#[derive(Clone, Debug)]
pub(crate) struct Check {
    /// 断言名。
    pub(crate) name: String,
    /// 是否通过。
    pub(crate) ok: bool,
    /// 观测到的真实值。
    pub(crate) detail: String,
}

/// 一个夹具在一种宿主下的运行证据。
#[derive(Clone, Debug)]
pub(crate) struct Evidence {
    /// 夹具 id。
    pub(crate) fixture: String,
    /// 宿主名。
    pub(crate) host: &'static str,
    /// 期望结果：成功或必须失败。
    pub(crate) expect: Expect,
    /// 实际结果。
    pub(crate) outcome: Outcome,
    /// 所有断言，逐条打印。
    pub(crate) checks: Vec<Check>,
    /// 输出目录（相对仓库根）。
    pub(crate) dir: String,
}

/// 夹具的期望结果。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Expect {
    /// 必须成功产出最终件并通过全部断言。
    Success,
    /// 必须被拒绝或编译失败，并给出可读诊断。
    Rejected,
}

/// 夹具的实际结果。
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum Outcome {
    /// 产出最终件。
    Success,
    /// 被 plan 拒绝（带诊断）。
    PlanRejected,
    /// 宿主编译失败（带诊断）。
    HostFailed,
    /// 达到轮数上限仍未收敛（带诊断）。
    NotConverged,
}

impl Outcome {
    /// 供日志打印的名字。
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::PlanRejected => "plan-rejected",
            Self::HostFailed => "host-failed",
            Self::NotConverged => "not-converged",
        }
    }
}

impl Evidence {
    /// 追加一条断言。
    pub(crate) fn check(&mut self, name: impl Into<String>, ok: bool, detail: impl Into<String>) {
        self.checks.push(Check {
            name: name.into(),
            ok,
            detail: detail.into(),
        });
    }

    /// 该夹具是否整体通过。
    pub(crate) fn passed(&self) -> bool {
        let outcome_ok = match self.expect {
            Expect::Success => self.outcome == Outcome::Success,
            Expect::Rejected => self.outcome != Outcome::Success,
        };
        outcome_ok && self.checks.iter().all(|check| check.ok)
    }
}
