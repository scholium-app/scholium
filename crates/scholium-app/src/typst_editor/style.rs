//! Core rules are exercised here through the UI adapter.
#[cfg(test)]
use super::*;
pub(crate) use scholium_session::typst::style::ActiveStyles;
#[cfg(test)]
mod tests;

pub(super) fn rejection_message(reason: scholium_session::typst::Rejection) -> &'static str {
    match reason {
        scholium_session::typst::Rejection::MathStyle => "公式内不支持文字样式。",
        _ => "选区端点无效；未应用格式。",
    }
}
