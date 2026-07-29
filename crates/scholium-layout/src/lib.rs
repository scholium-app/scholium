//! Typst layout integration for Scholium.
//!
//! Implements `typst::World` for font loading and source management.
//! Provides a compile cache: serializes the document AST, compiles it
//! with Typst, and returns the `PagedDocument` with a `SourceMap` for
//! reverse lookup.
//!
//! Dependency direction: `scholium-doc ← scholium-serialize ← scholium-layout`.
//! This is the only crate that directly depends on `typst`.

mod world;

pub use world::{CompileOutput, FontConfig, ScholiumWorld, compile};
