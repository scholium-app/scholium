//! 拒绝原因与恢复错误。
//!
//! `RejectReason` 的每个变体都对应报告中的一条独立断言；`Display` 直接输出
//! "原因码(字段=值)" 形式，便于把真实运行输出贴进证据。

use thiserror::Error;

use crate::model::Dialect;

/// 写入被拒绝的原因。
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub(crate) enum RejectReason {
    /// 客户端协议版本不受支持。
    #[error("ProtocolVersionUnsupported(got={got}, supported={supported})")]
    ProtocolVersionUnsupported {
        /// 收到的版本。
        got: u16,
        /// 支持的版本。
        supported: u16,
    },
    /// 写入包目标范围与协调者范围不一致。
    #[error("ScopeMismatch(expected={expected}, got={got})")]
    ScopeMismatch {
        /// 协调者范围。
        expected: String,
        /// 包声明范围。
        got: String,
    },
    /// 写集为空。
    #[error("EmptyWriteSet")]
    EmptyWriteSet,
    /// 写集条数超限。
    #[error("TooManyOps(got={got}, max={max})")]
    TooManyOps {
        /// 实际条数。
        got: usize,
        /// 上限。
        max: usize,
    },
    /// 负载超限。
    #[error("PayloadTooLarge(got={got}, max={max})")]
    PayloadTooLarge {
        /// 实际字节数。
        got: usize,
        /// 上限。
        max: usize,
    },
    /// 路径字符串形态非法。
    #[error("MalformedPath(path={path:?})")]
    MalformedPath {
        /// 非法路径。
        path: String,
    },
    /// 路径越出共享范围。
    #[error("PathOutOfScope(path={path:?})")]
    PathOutOfScope {
        /// 越界路径。
        path: String,
    },
    /// 路径扩展名与该方言不符。
    #[error("DialectExtensionMismatch(path={path:?}, expected_ext={expected_ext})")]
    DialectExtensionMismatch {
        /// 路径。
        path: String,
        /// 期望扩展名。
        expected_ext: String,
    },
    /// 包声明的语言与某个操作的实际语言不符。
    #[error("DeclaredDialectMismatch(declared={declared}, op={op})")]
    DeclaredDialectMismatch {
        /// 包声明语言。
        declared: Dialect,
        /// 操作实际语言。
        op: Dialect,
    },
    /// 同一包内混入两种语言的写集。
    #[error("MixedDialectWriteSet(dialects={dialects:?})")]
    MixedDialectWriteSet {
        /// 出现的语言集合。
        dialects: Vec<String>,
    },
    /// 声明语言与实际内容特征矛盾。
    #[error("ContentDialectMismatch(declared={declared}, foreign_marker={marker:?})")]
    ContentDialectMismatch {
        /// 声明语言。
        declared: Dialect,
        /// 命中的异语言结构标记。
        marker: &'static str,
    },
    /// 负载包含控制字节等畸形内容。
    #[error("MalformedPayload(detail={detail})")]
    MalformedPayload {
        /// 细节。
        detail: String,
    },
    /// 引用了未知许可。
    #[error("PermitUnknown(id={id})")]
    PermitUnknown {
        /// 许可 ID。
        id: u64,
    },
    /// 许可不属于该 actor。
    #[error("PermitNotOwner(id={id}, owner={owner}, actor={actor})")]
    PermitNotOwner {
        /// 许可 ID。
        id: u64,
        /// 许可真正持证人。
        owner: String,
        /// 提交者。
        actor: String,
    },
    /// 许可已被撤销。
    #[error("PermitRevoked(id={id})")]
    PermitRevoked {
        /// 许可 ID。
        id: u64,
    },
    /// 许可已过期。
    #[error("PermitExpired(id={id}, expires_at={expires_at}, tick={tick})")]
    PermitExpired {
        /// 许可 ID。
        id: u64,
        /// 失效时刻。
        expires_at: u64,
        /// 当前时刻。
        tick: u64,
    },
    /// 许可的语言与包声明不符。
    #[error("PermitDialectMismatch(id={id}, permit={permit}, packet={packet})")]
    PermitDialectMismatch {
        /// 许可 ID。
        id: u64,
        /// 许可语言。
        permit: Dialect,
        /// 包声明语言。
        packet: Dialect,
    },
    /// 声明 epoch 高于当前 epoch（伪造未来代数）。
    #[error("EpochForged(got={got}, current={current})")]
    EpochForged {
        /// 声明值。
        got: u64,
        /// 当前值。
        current: u64,
    },
    /// 声明 epoch 旧于当前 epoch。
    #[error("SourceEpochStale(got={got}, current={current})")]
    SourceEpochStale {
        /// 声明值。
        got: u64,
        /// 当前值。
        current: u64,
    },
    /// 写入语言不是当前活动语言。
    #[error("DialectNotActive(active={active}, requested={requested})")]
    DialectNotActive {
        /// 活动语言。
        active: Dialect,
        /// 请求语言。
        requested: Dialect,
    },
    /// 屏障进行中，目标语言尚未可写。
    #[error("TargetDialectNotYetActive(target={target})")]
    TargetDialectNotYetActive {
        /// 目标语言。
        target: Dialect,
    },
    /// 序号回退。
    #[error("SequenceRollback(seq={seq}, last_accepted={last_accepted})")]
    SequenceRollback {
        /// 提交序号。
        seq: u64,
        /// 已接受的最大序号。
        last_accepted: u64,
    },
    /// 同一序号被用于不同内容。
    #[error("DuplicateSeqConflict(seq={seq})")]
    DuplicateSeqConflict {
        /// 冲突序号。
        seq: u64,
    },
    /// 网络分区导致无法到达协调者。
    #[error("NetworkUnreachable(actor={actor})")]
    NetworkUnreachable {
        /// 被隔离的 actor。
        actor: String,
    },
    /// 提交者不是团队成员。
    #[error("NotAMember(actor={actor})")]
    NotAMember {
        /// 提交者。
        actor: String,
    },
    /// 已有切换在进行中。
    #[error("SwitchInProgress")]
    SwitchInProgress,
    /// 目标语言与当前活动语言相同。
    #[error("DialectUnchanged(active={active})")]
    DialectUnchanged {
        /// 当前语言。
        active: Dialect,
    },
    /// 屏障未完成：仍有成员未确认 drain。
    #[error("DrainIncomplete(missing={missing:?})")]
    DrainIncomplete {
        /// 未确认的成员。
        missing: Vec<String>,
    },
    /// 当前不在屏障中。
    #[error("NoSwitchInProgress")]
    NoSwitchInProgress,
}

/// 控制记录恢复失败。
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub(crate) enum RecoveryError {
    /// 日志中部损坏（校验和不匹配或记录无法解码）。
    #[error("MidLogCorruption(offset={offset}, detail={detail})")]
    MidLogCorruption {
        /// 损坏记录起始偏移。
        offset: usize,
        /// 细节。
        detail: String,
    },
    /// 日志没有任何控制记录。
    ///
    /// 恢复时不允许回退到"默认打开 LaTeX"这种可写状态，必须显式 bootstrap。
    #[error("NoControlRecord")]
    NoControlRecord,
    /// 缺少 bootstrap 记录，无法重建初始语言与 epoch。
    #[error("MissingBootstrap")]
    MissingBootstrap,
}
