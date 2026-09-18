//! Exercise the public AccessKit action path, including Unicode and cross-leaf selection.
use egui::{
    Context, Event, RawInput,
    accesskit::{self, Action, ActionData, Role, TextPosition, TextSelection},
};
use scholium_spike_core::Cursor;
use scholium_spike_egui::SpikeApp;

fn frame(app: &mut SpikeApp, ctx: &Context, events: Vec<Event>) -> accesskit::TreeUpdate {
    let mut output = ctx.run_ui(
        RawInput {
            events,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(844.0, 1011.0),
            )),
            ..Default::default()
        },
        |ui| app.draw(ui),
    );
    output.textures_delta.clear();
    output
        .platform_output
        .accesskit_update
        .expect("enabled accessibility")
}

fn select(body: accesskit::NodeId, anchor: TextPosition, focus: TextPosition) -> Event {
    Event::AccessKitActionRequest(accesskit::ActionRequest {
        action: Action::SetTextSelection,
        target_tree: accesskit::TreeId::ROOT,
        target_node: body,
        data: Some(ActionData::SetTextSelection(TextSelection {
            anchor,
            focus,
        })),
    })
}

fn undo(app: &mut SpikeApp, ctx: &Context) {
    let modifiers = egui::Modifiers {
        ctrl: true,
        command: true,
        ..egui::Modifiers::NONE
    };
    frame(
        app,
        ctx,
        vec![
            Event::ModifiersChanged(modifiers),
            Event::Key {
                key: egui::Key::Z,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            },
        ],
    );
}

#[test]
fn accessible_runs_follow_model_order_instead_of_math_paint_order() {
    let ctx = Context::default();
    ctx.enable_accesskit();
    let mut app = SpikeApp::new_layout_probe(&ctx, None);
    let tree = frame(&mut app, &ctx, vec![]);
    let (_, body) = tree
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("正文结构编辑器"))
        .expect("body");
    // The same run must not also be attached to the root or another parent.
    assert_eq!(
        tree.nodes
            .iter()
            .filter(|(_, n)| n.children().contains(&body.children()[0]))
            .count(),
        1
    );
    let text: String = body
        .children()
        .iter()
        .map(|id| {
            tree.nodes
                .iter()
                .find(|(candidate, _)| candidate == id)
                .expect("text child")
                .1
                .value()
                .expect("text")
        })
        .collect();
    assert!(
        text.ends_with("abx12123y"),
        "subscript precedes superscript in the model: {text}"
    );
}

#[test]
fn accessible_unicode_selection_cuts_and_undoes_across_leaves() {
    let ctx = Context::default();
    ctx.enable_accesskit();
    let mut app = SpikeApp::new_layout_probe(&ctx, None);
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    let tree = frame(&mut app, &ctx, vec![Event::Text("中😀e\u{301}".into())]);
    let (body, node) = tree
        .nodes
        .iter()
        .find(|(_, n)| n.role() == Role::MultilineTextInput && n.label() == Some("正文结构编辑器"))
        .expect("body text input");
    let first = node.children()[0];
    let second = node.children()[1];
    assert!(tree.nodes.iter().any(|(id, n)| *id == first && n.value().is_some_and(|v| v.starts_with("中😀e\u{301}"))));
    let at = |node, character_index| TextPosition {
        node,
        character_index,
    };
    frame(
        &mut app,
        &ctx,
        vec![select(*body, at(first, 1), at(first, 2))],
    );
    let Cursor::Text { byte, .. } = app.selection().focus else {
        panic!("text focus")
    };
    assert_eq!(byte, 7, "scalar two maps past the four-byte emoji");
    let before = app.selection();
    frame(
        &mut app,
        &ctx,
        vec![select(*body, at(first, 3), at(first, 3))],
    );
    assert_eq!(
        app.selection(),
        before,
        "reject position inside combining grapheme"
    );
    let text = app.plain_text();
    frame(
        &mut app,
        &ctx,
        vec![select(*body, at(first, 0), at(second, 1))],
    );
    assert_ne!(
        app.selection().anchor.focus(),
        app.selection().focus.focus()
    );
    frame(&mut app, &ctx, vec![Event::Cut]);
    assert_ne!(app.plain_text(), text);
    undo(&mut app, &ctx);
    assert_eq!(app.plain_text(), text);
}

#[test]
fn horizontal_scroll_moves_long_document_origin() {
    let ctx = Context::default();
    ctx.enable_accesskit();
    let mut app = SpikeApp::new_layout_probe(&ctx, None);
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![Event::Text("long text ".repeat(100))]);
    let before = app.structure_origin();
    frame(
        &mut app,
        &ctx,
        vec![
            Event::PointerMoved(before + egui::vec2(40.0, 40.0)),
            Event::MouseWheel {
                phase: egui::TouchPhase::Move,
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(-300.0, 0.0),
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    for _ in 0..8 {
        frame(&mut app, &ctx, vec![]);
    }
    assert!(
        app.structure_origin().x < before.x - 50.0,
        "long canvas must actually scroll"
    );
}
