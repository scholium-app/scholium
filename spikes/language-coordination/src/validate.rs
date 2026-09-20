//! 写入包的结构级校验（最小可信校验）。
//!
//! 设计前提：服务端**不信任**客户端声明的 dialect，但也不解析语言语义、不执行宏；
//! 这里只做结构级写集检查，见 `docs/MIXED_SOURCE_EDITING.md` 第 4 节。
//!
//! 明确简化：这里没有解析器。内容与声明语言的矛盾只用一小组结构标记抽检，
//! 因此注释或字符串里出现异语言标记会误报；正式实现需要真正的词法/结构校验。

use std::collections::BTreeSet;

use crate::error::RejectReason;
use crate::model::{
    Dialect, MAX_OPS, MAX_PATH_BYTES, MAX_PAYLOAD_BYTES, PROTOCOL_VERSION, WriteOp, WritePacket,
};

/// 用于"声明语言 vs 实际内容"抽检的 Typst 结构标记。不是解析器。
const TYPST_MARKERS: &[&str] = &[
    "#let ",
    "#set ",
    "#show ",
    "#import ",
    "#include ",
    "#context ",
    "#for ",
    "#if ",
];

/// 用于"声明语言 vs 实际内容"抽检的 LaTeX 结构标记。不是解析器。
const LATEX_MARKERS: &[&str] = &[
    "\\begin{",
    "\\end{",
    "\\section{",
    "\\frac{",
    "\\documentclass",
    "\\usepackage",
    "\\newcommand",
];

/// 校验一个写入包。`Err` 时返回首个命中的拒绝原因。
///
/// 检查顺序：协议 → 空写集 → 条数 → 负载 → 混语言 → 逐操作（路径 / 文本 / 内容方言）。
pub(crate) fn validate_packet(pkt: &WritePacket) -> Result<(), RejectReason> {
    if pkt.protocol != PROTOCOL_VERSION {
        return Err(RejectReason::ProtocolVersionUnsupported {
            got: pkt.protocol,
            supported: PROTOCOL_VERSION,
        });
    }
    validate_shape(pkt)?;
    for op in &pkt.ops {
        if op.dialect != pkt.declared {
            return Err(RejectReason::DeclaredDialectMismatch {
                declared: pkt.declared,
                op: op.dialect,
            });
        }
        validate_op(op)?;
    }
    Ok(())
}

/// 与具体操作无关的形态检查。
fn validate_shape(pkt: &WritePacket) -> Result<(), RejectReason> {
    if pkt.ops.is_empty() {
        return Err(RejectReason::EmptyWriteSet);
    }
    if pkt.ops.len() > MAX_OPS {
        return Err(RejectReason::TooManyOps {
            got: pkt.ops.len(),
            max: MAX_OPS,
        });
    }
    let payload: usize = pkt.ops.iter().map(|op| op.path.len() + op.text.len()).sum();
    if payload > MAX_PAYLOAD_BYTES {
        return Err(RejectReason::PayloadTooLarge {
            got: payload,
            max: MAX_PAYLOAD_BYTES,
        });
    }
    let dialects: BTreeSet<Dialect> = pkt.ops.iter().map(|op| op.dialect).collect();
    if dialects.len() > 1 {
        let names = dialects.iter().map(|d| d.name().to_owned()).collect();
        return Err(RejectReason::MixedDialectWriteSet { dialects: names });
    }
    Ok(())
}

/// 校验单条操作。
fn validate_op(op: &WriteOp) -> Result<(), RejectReason> {
    validate_path(&op.path, op.dialect)?;
    validate_text(&op.text)?;
    if let Some(marker) = content_contradicts(op.dialect, &op.text) {
        return Err(RejectReason::ContentDialectMismatch {
            declared: op.dialect,
            marker,
        });
    }
    Ok(())
}

/// 路径必须相对、局限在共享范围内，且扩展名与方言一致。
fn validate_path(path: &str, dialect: Dialect) -> Result<(), RejectReason> {
    if path.is_empty() || path.len() > MAX_PATH_BYTES || path.chars().any(char::is_control) {
        return Err(RejectReason::MalformedPath {
            path: path.to_owned(),
        });
    }
    // 绝对路径、反斜杠路径与任何 `..` / 空 / `.` 分量都视为越界。
    if path.starts_with('/') || path.contains('\\') {
        return Err(RejectReason::PathOutOfScope {
            path: path.to_owned(),
        });
    }
    for component in path.split('/') {
        if component.is_empty() || component == "." || component == ".." || component.contains(':')
        {
            return Err(RejectReason::PathOutOfScope {
                path: path.to_owned(),
            });
        }
    }
    let expected_ext = format!(".{}", dialect.extension());
    if !path.ends_with(&expected_ext) {
        return Err(RejectReason::DialectExtensionMismatch {
            path: path.to_owned(),
            expected_ext,
        });
    }
    Ok(())
}

/// 拒绝控制字节等畸形负载。
///
/// 本 spike 的 wire 类型是 `String`，无法承载非法 UTF-8，因此"畸形"用控制字节建模；
/// 报告在"失败与不确定性"里记下这一简化。
fn validate_text(text: &str) -> Result<(), RejectReason> {
    let bad = text
        .chars()
        .any(|c| c == '\0' || (c.is_control() && !matches!(c, '\n' | '\t' | '\r')));
    if bad {
        return Err(RejectReason::MalformedPayload {
            detail: "control byte in payload".to_owned(),
        });
    }
    Ok(())
}

/// 若内容出现异语言标记、且完全没有本语言标记，则判定声明与内容矛盾。
///
/// 两种标记同时出现时不判定（歧义样本宁可放过也不误报）。
fn content_contradicts(declared: Dialect, text: &str) -> Option<&'static str> {
    let foreign = markers(declared.other())
        .iter()
        .copied()
        .find(|marker| text.contains(marker))?;
    let own_present = markers(declared).iter().any(|marker| text.contains(marker));
    if own_present { None } else { Some(foreign) }
}

/// 方言对应的标记集合。
fn markers(dialect: Dialect) -> &'static [&'static str] {
    match dialect {
        Dialect::Latex => LATEX_MARKERS,
        Dialect::Typst => TYPST_MARKERS,
    }
}
