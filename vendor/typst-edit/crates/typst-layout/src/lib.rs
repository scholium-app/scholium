//! Typst's layout engine.

mod document;
#[cfg(feature = "editor")]
mod editor_metrics;
#[cfg(feature = "editor")]
pub use editor_metrics::editor_paragraph_layouts;
mod flow;
mod grid;
mod image;
mod inline;
mod introspect;
mod lists;
mod math;
mod modifiers;
mod pad;
mod pages;
mod repeat;
mod rules;
mod shapes;
mod stack;
mod transforms;

pub use self::document::{Page, PagedDocument};
pub use self::flow::{layout_fragment, layout_frame};
pub use self::introspect::PagedIntrospector;
pub use self::pages::{layout_document, layout_document_for_bundle};
pub use self::rules::register;
