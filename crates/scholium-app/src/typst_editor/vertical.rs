//! Test adapters for visual navigation now owned by `scholium-session`.
#![allow(unused_imports)]
#[cfg(test)]
use super::*;
#[cfg(test)]
use scholium_model::{NodeId, structured::TextStyle};
#[cfg(test)]
pub(crate) use scholium_session::typst::vertical::Motion;
#[cfg(test)]
pub(crate) use scholium_session::typst::vertical::{Span, Stop};
#[cfg(test)]
pub(crate) use scholium_session::typst::vertical::{adjacent, edge};
#[cfg(test)]
pub(crate) use scholium_typst::editor::{Affinity, Caret, Position};
#[cfg(test)]
mod tests;
