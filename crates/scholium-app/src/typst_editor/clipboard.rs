//! Clipboard requests from controls: they reach the page as ordinary platform events.
use crate::command::spec::ClipboardOp;
use eframe::egui;

/// Return focus to the page and ask the platform for the clipboard operation, which
/// arrives as an ordinary Copy/Cut/Paste event in the page's input stream.
pub(crate) fn request_clipboard(ctx: &egui::Context, op: ClipboardOp) {
    ctx.memory_mut(|m| m.request_focus(super::paint::id()));
    ctx.send_viewport_cmd(match op {
        ClipboardOp::Copy => egui::ViewportCommand::RequestCopy,
        ClipboardOp::Cut => egui::ViewportCommand::RequestCut,
        ClipboardOp::Paste => egui::ViewportCommand::RequestPaste,
    });
}
