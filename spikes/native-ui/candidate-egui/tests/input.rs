//! 无窗口的输入路径测试。
//!
//! 为什么必须这样做：`ydotool` 注入依赖窗口焦点，在共享桌面会话里会被抢焦点，
//! 我已经被它误导过——用一次不可靠的运行去断言"框架不支持输入法"。这里的测试用 egui 的
//! `Context::run_ui` 加合成输入驱动**同一段输入处理代码**，不依赖窗口、焦点或输入法进程，
//! 因此结论是确定性的。真实输入法仍需人工确认（见 `docs/spikes/0004-native-ui-egui.md`）。

use egui::{Context, Event, ImeEvent, Key, Modifiers, RawInput};
use scholium_spike_egui::SpikeApp;

/// 跑一帧，可带合成事件。
fn frame(app: &mut SpikeApp, ctx: &Context, events: Vec<Event>) {
    let input = RawInput {
        events,
        ..Default::default()
    };
    let mut output = ctx.run_ui(input, |ui| app.draw(ui));
    // 无窗口测试里没有渲染器消费纹理增量，必须显式清理，
    // 否则 epaint 会在 drop 时触发 "unapplied deltas" 断言。
    output.textures_delta.clear();
}

/// 跑两帧让"启动默认聚焦正文区"生效。
fn settle(app: &mut SpikeApp, ctx: &Context) {
    frame(app, ctx, vec![]);
    frame(app, ctx, vec![]);
}

fn new_app(ctx: &Context) -> SpikeApp {
    SpikeApp::new(ctx, None)
}

fn text_event(text: &str) -> Event {
    Event::Text(text.to_string())
}

fn key_event(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn preedit(text: &str) -> Event {
    Event::Ime(ImeEvent::Preedit {
        text: text.to_string(),
        active_range_chars: None,
    })
}

fn commit(text: &str) -> Event {
    Event::Ime(ImeEvent::Commit(text.to_string()))
}

#[test]
fn startup_focuses_the_document_area() {
    let ctx = Context::default();
    let mut app = new_app(&ctx);
    settle(&mut app, &ctx);
    assert!(
        app.structure_focused(),
        "启动后正文区应持有输入焦点，否则键盘输入无从进入"
    );
}

#[test]
fn preedit_does_not_touch_history_and_commit_adds_exactly_one_action() {
    let ctx = Context::default();
    let mut app = new_app(&ctx);
    settle(&mut app, &ctx);

    let before = app.core().history().len();

    frame(&mut app, &ctx, vec![preedit("n")]);
    frame(&mut app, &ctx, vec![preedit("ni")]);
    frame(&mut app, &ctx, vec![preedit("ni hao")]);

    assert_eq!(
        app.core().history().len(),
        before,
        "预编辑期间不得产生任何核心动作"
    );
    assert_eq!(app.preedit_events(), 3, "预编辑事件应被记录");
    assert!(
        !app.plain_text().contains("ni hao"),
        "预编辑内容不得进入正文"
    );

    frame(&mut app, &ctx, vec![commit("你好")]);

    assert_eq!(
        app.core().history().len(),
        before + 1,
        "一次提交恰好产生一个核心动作"
    );
    assert_eq!(app.commits(), 1);
    assert!(app.plain_text().contains("你好"), "提交内容应进入正文");
}

#[test]
fn typing_inserts_text_and_advances_the_caret() {
    let ctx = Context::default();
    let mut app = new_app(&ctx);
    settle(&mut app, &ctx);

    let start = app.focus();
    frame(&mut app, &ctx, vec![text_event("abc")]);
    let after = app.focus();

    assert_ne!(start, after, "输入后焦点（插入点）应前进");
    assert!(app.plain_text().contains("abc"), "输入的文本应进入正文");
}

#[test]
fn backspace_deletes_one_grapheme() {
    let ctx = Context::default();
    let mut app = new_app(&ctx);
    settle(&mut app, &ctx);

    frame(&mut app, &ctx, vec![text_event("ab")]);
    assert!(app.plain_text().contains("ab"));

    frame(
        &mut app,
        &ctx,
        vec![key_event(Key::Backspace, Modifiers::NONE)],
    );

    let text = app.plain_text();
    assert!(text.contains('a'), "退格只应删除一个字素");
    assert!(!text.contains("ab"), "退格后不应还存在 ab：{text}");
}

#[test]
fn typing_zwj_emoji_then_backspace_does_not_split_grapheme() {
    let ctx = Context::default();
    let mut app = new_app(&ctx);
    settle(&mut app, &ctx);

    let family = "👨‍👩‍👧";
    frame(&mut app, &ctx, vec![text_event(&format!("a{family}b"))]);
    frame(
        &mut app,
        &ctx,
        vec![key_event(Key::Backspace, Modifiers::NONE)],
    );

    let text = app.plain_text();
    assert!(
        text.contains(family),
        "退格应删除末尾的 b，而不是拆开 ZWJ 序列：{text}"
    );
}

#[test]
fn arrow_keys_do_not_corrupt_the_document() {
    let ctx = Context::default();
    let mut app = new_app(&ctx);
    settle(&mut app, &ctx);

    let before = app.plain_text();
    for key in [
        Key::ArrowDown,
        Key::ArrowUp,
        Key::ArrowLeft,
        Key::ArrowRight,
    ] {
        frame(&mut app, &ctx, vec![key_event(key, Modifiers::NONE)]);
    }
    assert_eq!(app.plain_text(), before, "纯导航不应改变文档内容");
}

#[test]
fn undo_reverts_the_local_typing() {
    let ctx = Context::default();
    let mut app = new_app(&ctx);
    settle(&mut app, &ctx);

    frame(&mut app, &ctx, vec![text_event("xyz")]);
    assert!(app.plain_text().contains("xyz"));

    let actions_before = app.core().history().len();
    // 注意 egui 0.36 的 `Modifiers::CTRL` 里 `command` 是 false，而 `Modifiers::COMMAND`
    // 里 `ctrl` 是 false；真机上 egui-winit 会把两者一起置位。这里要显式构造。
    let ctrl = Modifiers {
        ctrl: true,
        command: true,
        ..Modifiers::NONE
    };
    // `InputState.modifiers` 由 `ModifiersChanged` 事件更新，按键事件里的 `modifiers`
    // 字段不参与该状态，因此两个事件都要发。
    frame(
        &mut app,
        &ctx,
        vec![Event::ModifiersChanged(ctrl), key_event(Key::Z, ctrl)],
    );

    assert!(
        !app.plain_text().contains("xyz"),
        "Ctrl+Z 应撤销本地输入：{}",
        app.plain_text()
    );
    assert!(
        app.core().history().len() > actions_before,
        "撤销应产生补偿动作（历史只追加）"
    );
}

#[test]
fn clicking_in_the_document_moves_the_caret() {
    let ctx = Context::default();
    let mut app = new_app(&ctx);
    settle(&mut app, &ctx);

    let origin = app.structure_origin();
    assert_ne!(origin, egui::Pos2::ZERO, "绘制后应记录正文原点");

    let start = app.focus();
    // 点在第一行文本靠右处：应把插入点移进该文本，而不是停在原处。
    let position = egui::pos2(origin.x + 30.0, origin.y + 10.0);

    frame(
        &mut app,
        &ctx,
        vec![
            Event::PointerMoved(position),
            Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
    );
    frame(
        &mut app,
        &ctx,
        vec![Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );

    let after = app.focus();
    assert_ne!(start, after, "点击后插入点应移动");
    assert!(
        matches!(after, scholium_spike_core::Cursor::Text { .. }),
        "点击后焦点应是文本插入点"
    );
}

#[test]
fn dragging_in_the_document_selects_a_range() {
    let ctx = Context::default();
    let mut app = new_app(&ctx);
    settle(&mut app, &ctx);
    assert!(app.selection().is_collapsed(), "初始应是折叠选区");

    let origin = app.structure_origin();
    let from = egui::pos2(origin.x + 10.0, origin.y + 25.0);
    let to = egui::pos2(origin.x + 60.0, origin.y + 25.0);

    // 按下 → 拖到另一处 → 松开
    frame(
        &mut app,
        &ctx,
        vec![
            Event::PointerMoved(from),
            Event::PointerButton {
                pos: from,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
    );
    frame(&mut app, &ctx, vec![Event::PointerMoved(to)]);
    frame(
        &mut app,
        &ctx,
        vec![Event::PointerButton {
            pos: to,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );

    let selection = app.selection();
    assert!(
        !selection.is_collapsed(),
        "拖拽应产生非折叠选区（焦点 {:?}，锚点 {:?}）",
        selection.focus,
        selection.anchor
    );
    let range = selection
        .text_range(app.core().document())
        .expect("拖拽产生的选区应落在同一文本叶子内");
    assert!(range.1 < range.2, "选区应有正的字节区间：{range:?}");
}

#[test]
fn backspace_deletes_exactly_the_selected_range() {
    let ctx = Context::default();
    let mut app = new_app(&ctx);
    settle(&mut app, &ctx);

    let origin = app.structure_origin();
    let from = egui::pos2(origin.x + 10.0, origin.y + 25.0);
    let to = egui::pos2(origin.x + 60.0, origin.y + 25.0);
    frame(
        &mut app,
        &ctx,
        vec![
            Event::PointerMoved(from),
            Event::PointerButton {
                pos: from,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
    );
    frame(&mut app, &ctx, vec![Event::PointerMoved(to)]);
    frame(
        &mut app,
        &ctx,
        vec![Event::PointerButton {
            pos: to,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );

    let before = app.plain_text();
    let (_, start, end) = app
        .selection()
        .text_range(app.core().document())
        .expect("应有同叶选区");
    let selected: String = before
        .get(start..end)
        .expect("选区应落在文本内")
        .to_string();
    assert!(!selected.is_empty(), "选区不应为空");

    frame(
        &mut app,
        &ctx,
        vec![key_event(Key::Backspace, Modifiers::NONE)],
    );

    let after = app.plain_text();
    assert!(
        after.len() < before.len(),
        "删除选区后文本应变短：{before:?} → {after:?}"
    );
    assert!(
        !after.contains(&selected),
        "被选中的内容应已删除：选区={selected:?}，结果={after:?}"
    );
    assert!(app.selection().is_collapsed(), "删除后选区应折叠到起点");
}

/// 把文档结构打印成紧凑摘要，便于断言。
fn summary(document: &scholium_spike_core::Document) -> String {
    fn walk(
        document: &scholium_spike_core::Document,
        node: scholium_spike_core::NodeId,
        out: &mut String,
    ) {
        let Ok(current) = document.node(node) else {
            return;
        };
        use scholium_spike_core::NodeKind;
        match current.kind {
            NodeKind::Text | NodeKind::Raw => out.push_str(&format!(
                "{:?}({:?})",
                current.kind,
                document.text_of(node).unwrap_or_default()
            )),
            _ => {
                out.push_str(&format!("{:?}(", current.kind));
                for slot in 0..current.kind.slot_count() {
                    if let Ok(children) = document.slot(node, slot) {
                        for child in children {
                            walk(document, *child, out);
                        }
                    }
                }
                out.push(')');
            }
        }
    }
    let mut out = String::new();
    walk(document, document.root(), &mut out);
    out
}

#[test]
fn wrapping_the_focused_node_creates_a_structure() {
    use scholium_spike_core::NodeKind;

    let ctx = Context::default();
    let mut app = new_app(&ctx);
    settle(&mut app, &ctx);

    // 夹具里已经有 Fraction / Script / Matrix / Sqrt，所以用夹具中不存在的
    // Delimited 来断言，并统计出现次数而不是"包含与否"。
    let before = summary(app.core().document());
    assert!(!before.contains("Delimited("), "夹具不应含定界符：{before}");

    app.wrap(NodeKind::Delimited);

    let after = summary(app.core().document());
    assert!(after.contains("Delimited("), "包裹后应出现定界符：{after}");
    assert!(
        after.contains("结构渲染夹具："),
        "包裹不应丢失原文：{after}"
    );
    assert_eq!(
        after.matches("Delimited(").count(),
        1,
        "只应新增一个定界符：{after}"
    );
}

#[test]
fn unwrapping_removes_the_structure_and_keeps_content() {
    use scholium_spike_core::NodeKind;

    let ctx = Context::default();
    let mut app = new_app(&ctx);
    settle(&mut app, &ctx);

    app.wrap(NodeKind::Delimited);
    assert!(summary(app.core().document()).contains("Delimited("));

    app.unwrap();
    let after = summary(app.core().document());
    assert!(
        !after.contains("Delimited("),
        "解除后不应还有定界符：{after}"
    );
    assert!(
        after.contains("结构渲染夹具："),
        "解除后必须保留内容：{after}"
    );
}
