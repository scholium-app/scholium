//! Command registry: the single source for menus, toolbars, the Ribbon, shortcuts and
//! (later) command search. `spec`, `registry`, `avail`, `context` and the domain modules
//! hold no egui types; `keys`, `shortcuts` and `ui` are the toolkit edge.
#[macro_use]
mod macros;
mod avail;
mod context;
mod document;
mod edit;
mod exec;
mod file;
mod format;
mod help;
mod insert;
pub(crate) mod keys;
pub(crate) mod registry;
pub(crate) mod shortcuts;
pub(crate) mod spec;
#[cfg(test)]
pub(crate) mod tests;
mod tools;
pub(crate) mod ui;
mod view;
