//! Scholium shell — window management and event loop.
//!
//! Wraps winit for window creation and event handling. The actual rendering
//! is delegated to `scholium-render`.
//!
//! P1 coverage: basic window with vello rendering surface, event loop setup.
//! IME handling, menus, and dialogs will be added per PLAN milestones.

/// Winit application handler driving the window and render loop.
pub mod app;
