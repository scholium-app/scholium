//! Serialize the document AST to Typst source with a `SourceMap` for reverse lookup.
//!
//! Dependency direction: `scholium-doc → scholium-serialize`.
//! This crate has no IO, rendering, or AI dependencies.

mod serialize;
mod source_map;

pub use serialize::serialize;
pub use source_map::SourceMap;
