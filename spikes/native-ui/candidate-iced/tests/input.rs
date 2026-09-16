//! Iced 候选的确定性测试。
//!
//! 与 egui 候选的 `tests/input.rs` 覆盖同一组输入契约，便于横向对比：
//! 直接调用 `App::update` 驱动**同一条消息处理代码**，不依赖窗口、焦点或输入法进程。
//!
//! 真实输入法仍需人工确认（见 `docs/spikes/0001-native-ui-iced.md`）。

use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};

use scholium_spike_iced::{App, Message};

fn new_app() -> App {
    App::boot(false)
}

fn key(key: Key) -> Message {
    Message::Key(key, Modifiers::default())
}

fn character(text: &str) -> Message {
    Message::Key(Key::Character(text.into()), Modifiers::default())
}

#[test]
fn preedit_does_not_touch_history_and_commit_adds_exactly_one_action() {
    let mut app = new_app();
    let before = app.core().history().len();

    let _ = app.update(Message::Preedit("n".to_string()));
    let _ = app.update(Message::Preedit("ni".to_string()));
    let _ = app.update(Message::Preedit("ni hao".to_string()));

    assert_eq!(
        app.core().history().len(),
        before,
        "预编辑期间不得产生核心动作"
    );
    assert_eq!(app.preedit_events(), 3);
    assert!(
        !app.plain_text().contains("ni hao"),
        "预编辑内容不得进入正文"
    );

    let _ = app.update(Message::Commit("你好".to_string()));

    assert_eq!(
        app.core().history().len(),
        before + 1,
        "一次提交恰好产生一个核心动作"
    );
    assert_eq!(app.commits(), 1);
    assert!(app.plain_text().contains("你好"));
}

#[test]
fn ime_commit_is_ignored_after_focus_moves_to_source_pane() {
    let mut app = new_app();
    let before = app.core().history().len();

    // 源码面板取走输入焦点后，窗口级输入法事件不应再写进正文。
    let _ = app.update(Message::SourceEdit(iced::widget::text_editor::Action::SelectAll));
    let _ = app.update(Message::Commit("你好".to_string()));

    assert_eq!(app.core().history().len(), before, "提交不得进入正文");
    assert!(
        !app.plain_text().contains("你好"),
        "焦点在源码面板时正文不应变化"
    );
}

#[test]
fn typing_inserts_text_and_advances_the_caret() {
    let mut app = new_app();
    let start = app.focus();

    let _ = app.update(character("abc"));

    assert_ne!(start, app.focus(), "输入后插入点应前进");
    assert!(app.plain_text().contains("abc"));
}

#[test]
fn backspace_deletes_one_grapheme() {
    let mut app = new_app();
    let _ = app.update(character("ab"));
    assert!(app.plain_text().contains("ab"));

    let _ = app.update(key(Key::Named(Named::Backspace)));

    let text = app.plain_text();
    assert!(text.contains('a'), "退格只应删除一个字素：{text}");
    assert!(!text.contains("ab"), "退格后不应还存在 ab：{text}");
}

#[test]
fn typing_zwj_emoji_then_backspace_does_not_split_grapheme() {
    let mut app = new_app();
    let family = "👨‍👩‍👧";
    let _ = app.update(character(&format!("a{family}b")));
    let _ = app.update(key(Key::Named(Named::Backspace)));

    let text = app.plain_text();
    assert!(
        text.contains(family),
        "退格应删除末尾的 b，而不是拆开 ZWJ 序列：{text}"
    );
}

#[test]
fn arrow_keys_do_not_corrupt_the_document() {
    let mut app = new_app();
    let before = app.plain_text();

    for named in [
        Named::ArrowDown,
        Named::ArrowUp,
        Named::ArrowLeft,
        Named::ArrowRight,
    ] {
        let _ = app.update(key(Key::Named(named)));
    }

    assert_eq!(app.plain_text(), before, "纯导航不应改变文档内容");
}

#[test]
fn undo_reverts_the_local_typing() {
    let mut app = new_app();
    let _ = app.update(character("xyz"));
    assert!(app.plain_text().contains("xyz"));

    let actions_before = app.core().history().len();
    let _ = app.update(Message::Undo);

    assert!(
        !app.plain_text().contains("xyz"),
        "撤销应移除本地输入：{}",
        app.plain_text()
    );
    assert!(
        app.core().history().len() > actions_before,
        "撤销应产生补偿动作（历史只追加）"
    );
}

#[test]
fn source_edit_action_moves_input_focus_and_read_only_write_is_rejected() {
    let mut app = new_app();

    let _ = app.update(Message::SourceEdit(
        iced::widget::text_editor::Action::SelectAll,
    ));

    assert_eq!(
        app.input_area(),
        scholium_spike_iced::InputArea::Source,
        "源码控件动作应把输入焦点切到源码区域"
    );
    assert!(
        app.last_error().is_some(),
        "只读面板应拒绝写入并给出错误，而不是静默接受"
    );
}
