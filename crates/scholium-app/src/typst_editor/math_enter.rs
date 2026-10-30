//! Core rules are exercised here through the UI adapter.
#[cfg(test)]
use super::*;
#[cfg(test)]
mod tests;

pub(super) fn missing_body_message() -> &'static str {
    "公式后没有可分段的正文位置。"
}
