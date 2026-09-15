//! Iced 候选最小工程：原生窗口 + 正文/源码/预览三区，复用 `scholium-spike-core`。
//!
//! 设计约束（对照 `docs/ARCHITECTURE.md` 与 `docs/modules/app.md`）：
//!
//! - 正文的权威状态在 core；UI 只保存投影，不持有第二份可写正文。
//! - 输入法预编辑只存在于 UI 状态，**不进历史**；`Commit` 才产生一个核心动作。
//! - 源码面板在非活动语言下不可写：控件的编辑会被权威缓冲回滚，而不是留在前端。

use iced::advanced::input_method;
use iced::keyboard::key::Named;
use iced::keyboard::{self, Key};
use iced::widget::{button, column, container, row, scrollable, text, text_editor};
use iced::{Element, Font, Length, Subscription, Task};

use scholium_spike_core::cursor::move_cursor;
use scholium_spike_core::{
    ActorId, Cursor, Dialect, Direction, Editor, Intent, NodeId, NodeKind, RemoteEdit,
    SemanticEdit, SourcePane,
};

/// 本机 CJK 字体。缺失时应用仍启动，但界面会报告字体缺口。
const CJK_FONT_PATH: &str = "/usr/share/fonts/adobe-source-han-serif/SourceHanSerifCN-Regular.otf";
const CJK_FAMILY: &str = "Source Han Serif CN";

/// 本地写入者。夹具用另一个 actor 注入，避免污染本地 undo scope。
const LOCAL: ActorId = ActorId(1);
const FIXTURE: ActorId = ActorId(99);

const SOURCE_INITIAL: &str =
    "\\documentclass{article}\n\\begin{document}\n正文与 $a/b$ 公式\n\\end{document}\n";

/// 启动候选应用。读取本机 CJK 字体；字体缺失时仍启动并在界面报告缺口。
pub fn main() -> iced::Result {
    let font_bytes = std::fs::read(CJK_FONT_PATH).ok();
    let font_loaded = font_bytes.is_some();

    let mut application = iced::application(move || App::boot(font_loaded), App::update, App::view)
        .title("Scholium spike — Iced candidate")
        .window_size((1280.0, 820.0))
        .subscription(App::subscription);

    if let Some(bytes) = font_bytes {
        application = application
            .font(bytes)
            .default_font(Font::with_name(CJK_FAMILY));
    }

    application.run()
}

/// 应用状态。
struct App {
    core: Editor,
    focus: Cursor,
    /// 输入法预编辑串。只属于 UI，不进 core。
    preedit: String,
    source: SourcePane,
    source_content: text_editor::Content,
    events: usize,
    preedit_events: usize,
    commits: usize,
    last_error: Option<String>,
    log: Vec<String>,
    cjk_font: bool,
}

/// UI 消息。
#[derive(Debug, Clone)]
enum Message {
    /// 输入法预编辑变化。
    Preedit(String),
    /// 输入法提交。
    Commit(String),
    /// 键盘按键。
    Key(Key, keyboard::Modifiers),
    /// 插入示例文本。
    InsertSample,
    /// 把焦点文本包裹进结构。
    Wrap(NodeKind),
    /// 解除最内层结构。
    Unwrap,
    /// 循环结构变体。
    CycleVariant,
    /// 结构导航。
    Move(Direction),
    /// 本地撤销。
    Undo,
    /// 切换源码面板可写性（团队活动语言授权）。
    ToggleSourceWritable,
    /// 源码控件动作。
    SourceEdit(text_editor::Action),
}

impl App {
    fn boot(cjk_font: bool) -> Self {
        let mut core = Editor::new();
        let paragraph = {
            let document = core.document();
            let root = document.root();
            document
                .slot(root, 0)
                .expect("根节点有 blocks 槽位")
                .first()
                .copied()
                .expect("文档至少有一个段落")
        };
        build_fixture(&mut core, paragraph);

        let source = SourcePane::new(Dialect::Latex, SOURCE_INITIAL);
        let source_content = text_editor::Content::with_text(source.text());
        let focus = core
            .document()
            .first_text_descendant(paragraph)
            .map(|node| Cursor::Text { node, byte: 0 })
            .expect("段落有文本叶子");

        Self {
            core,
            focus,
            preedit: String::new(),
            source,
            source_content,
            events: 0,
            preedit_events: 0,
            commits: 0,
            last_error: None,
            log: vec!["启动完成：夹具已由远端 actor 注入，本地历史为空".to_string()],
            cjk_font,
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        iced::event::listen_with(|event, _status, _window| match event {
            iced::event::Event::Keyboard(keyboard::Event::KeyPressed {
                key, modifiers, ..
            }) => Some(Message::Key(key, modifiers)),
            iced::event::Event::InputMethod(input_method::Event::Preedit(text, _)) => {
                Some(Message::Preedit(text))
            }
            iced::event::Event::InputMethod(input_method::Event::Commit(text)) => {
                Some(Message::Commit(text))
            }
            _ => None,
        })
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        self.events += 1;
        match message {
            Message::Preedit(text) => self.on_preedit(text),
            Message::Commit(text) => self.on_commit(text),
            Message::Key(key, modifiers) => self.on_key(key, modifiers),
            Message::InsertSample => self.insert_text("示例文本", Intent::Typing),
            Message::Wrap(kind) => self.wrap_focus(kind),
            Message::Unwrap => self.unwrap_focus(),
            Message::CycleVariant => self.cycle_variant_focus(),
            Message::Move(direction) => self.navigate(direction),
            Message::Undo => self.undo(),
            Message::ToggleSourceWritable => self.toggle_source_writable(),
            Message::SourceEdit(action) => self.on_source_edit(action),
        }
        Task::none()
    }

    // ------------------------------------------------------------ 输入与编辑

    /// 预编辑只更新 UI 状态，不触碰 core，因此不可能进入历史。
    fn on_preedit(&mut self, text: String) {
        self.preedit_events += 1;
        self.preedit = text;
        let shown = self.preedit.clone();
        self.push_log(format!("preedit=\"{shown}\"（未进历史）"));
    }

    /// 提交整串预编辑，作为一个 `ImeCommit` 动作。
    fn on_commit(&mut self, text: String) {
        self.preedit.clear();
        self.commits += 1;
        if text.is_empty() {
            self.push_log("ime commit：空串，忽略");
            return;
        }
        self.insert_text(&text, Intent::ImeCommit);
    }

    fn on_key(&mut self, key: Key, _modifiers: keyboard::Modifiers) {
        // 预编辑期间不处理字符键：组合中的按键由输入法消费，避免与 Commit 重复插入。
        if !self.preedit.is_empty() && matches!(key, Key::Character(_)) {
            return;
        }
        match key {
            Key::Named(Named::ArrowDown) => self.navigate(Direction::Down),
            Key::Named(Named::ArrowUp) => self.navigate(Direction::Up),
            Key::Named(Named::ArrowLeft) => self.navigate(Direction::Parent),
            Key::Named(Named::ArrowRight) => self.navigate(Direction::FirstChild),
            Key::Named(Named::Backspace) => self.delete_backward(),
            Key::Character(text) => {
                let text = text.to_string();
                self.insert_text(&text, Intent::Typing);
            }
            _ => {}
        }
    }

    /// 当前焦点对应的可编辑文本位置。
    fn target(&self) -> Option<(NodeId, usize)> {
        match self.focus {
            Cursor::Text { node, byte } => Some((node, byte)),
            Cursor::Slot { node, slot, index } => {
                let document = self.core.document();
                let list = document.slot(node, slot).ok()?;
                let child = list.get(index).or_else(|| list.last())?;
                document
                    .first_text_descendant(*child)
                    .map(|text_node| (text_node, 0))
            }
        }
    }

    fn insert_text(&mut self, text: &str, intent: Intent) {
        let Some((node, at)) = self.target() else {
            self.fail("没有可编辑的文本槽位");
            return;
        };
        let edit = SemanticEdit::InsertText {
            node,
            at,
            text: text.to_string(),
        };
        match self.core.apply(LOCAL, intent, edit) {
            Ok(Some(action)) => {
                self.focus = Cursor::Text {
                    node,
                    byte: at + text.len(),
                };
                let revision = self.core.revision();
                self.push_log(format!(
                    "core: {intent:?} \"{text}\" → action {action:?} revision {revision}"
                ));
            }
            Ok(None) => self.push_log("core: 空操作，未产生动作"),
            Err(error) => self.fail(&error.to_string()),
        }
    }

    fn delete_backward(&mut self) {
        let Some((node, at)) = self.target() else {
            self.fail("没有可编辑的文本槽位");
            return;
        };
        let edit = SemanticEdit::DeleteBackward { node, at };
        match self.core.apply(LOCAL, Intent::Typing, edit) {
            Ok(Some(_)) => {
                let previous = self
                    .core
                    .document()
                    .node(node)
                    .ok()
                    .and_then(|n| n.text.prev_grapheme_boundary(at))
                    .unwrap_or(0);
                self.focus = Cursor::Text {
                    node,
                    byte: previous,
                };
                self.push_log("core: 删除一个字素".to_string());
            }
            Ok(None) => self.push_log("退格：已在文本开头"),
            Err(error) => self.fail(&error.to_string()),
        }
    }

    fn wrap_focus(&mut self, kind: NodeKind) {
        let Some((node, _)) = self.target() else {
            self.fail("没有可包裹的节点");
            return;
        };
        match self.core.apply(LOCAL, Intent::Structural, SemanticEdit::Wrap { node, kind }) {
            Ok(Some(_)) => self.push_log(format!("core: 已包裹为 {kind:?}")),
            Ok(None) => self.push_log("包裹：空操作"),
            Err(error) => self.fail(&error.to_string()),
        }
    }

    fn unwrap_focus(&mut self) {
        let Some((node, _)) = self.target() else {
            self.fail("没有可解除的节点");
            return;
        };
        let Some((parent, _, _)) = self.core.document().locate_in_parent(node) else {
            self.fail("焦点没有父结构可解除");
            return;
        };
        match self
            .core
            .apply(LOCAL, Intent::Structural, SemanticEdit::Unwrap { node: parent })
        {
            Ok(Some(_)) => self.push_log("core: 已解除结构并保留内容".to_string()),
            Ok(None) => self.push_log("解除：空操作"),
            Err(error) => self.fail(&error.to_string()),
        }
    }

    fn cycle_variant_focus(&mut self) {
        let Some((node, _)) = self.target() else {
            self.fail("没有可循环的节点");
            return;
        };
        let Some((parent, _, _)) = self.core.document().locate_in_parent(node) else {
            self.fail("焦点没有父结构");
            return;
        };
        match self.core.apply(
            LOCAL,
            Intent::Structural,
            SemanticEdit::CycleVariant { node: parent },
        ) {
            Ok(Some(_)) => self.push_log("core: 结构变体已循环".to_string()),
            Ok(None) => self.push_log("变体：空操作"),
            Err(error) => self.fail(&error.to_string()),
        }
    }

    fn navigate(&mut self, direction: Direction) {
        match move_cursor(self.core.document(), self.focus, direction) {
            Ok(Some(next)) => {
                self.focus = next;
                self.push_log(format!("焦点 → {:?}", next.focus().index()));
            }
            Ok(None) => self.push_log(format!("{direction:?}: 没有可取位置")),
            Err(error) => self.fail(&error.to_string()),
        }
    }

    fn undo(&mut self) {
        match self.core.undo(LOCAL) {
            Ok(Some(outcome)) => {
                self.push_log(format!(
                    "core: 撤销 {:?}，删除 {} 字符，补偿 {:?}",
                    outcome.undone, outcome.removed_chars, outcome.compensation
                ));
            }
            Ok(None) => self.push_log("撤销：本地没有可撤销动作"),
            Err(error) => self.fail(&error.to_string()),
        }
    }

    // ------------------------------------------------------------ 源码面板

    fn toggle_source_writable(&mut self) {
        let writable = !self.source.is_writable();
        self.source.set_writable(writable);
        self.push_log(format!(
            "团队语言授权：{} → {}",
            self.source.dialect().extension(),
            if writable { "可写" } else { "只读" }
        ));
    }

    fn on_source_edit(&mut self, _action: text_editor::Action) {
        if !self.source.is_writable() {
            // 只读：把控件内容回滚到权威缓冲，证明 UI 不是第二权威副本。
            self.restore_source_from_authority();
            self.fail("源码面板为只读：拒绝非活动语言的写入，控件内容已回滚");
            return;
        }
        let text = self.source_content.text();
        match self.source.set_text(&text) {
            Ok(()) => self.push_log(format!("source: 已写入 {} 字节", text.len())),
            Err(error) => {
                self.restore_source_from_authority();
                self.fail(&error.to_string());
            }
        }
    }

    fn restore_source_from_authority(&mut self) {
        self.source_content = text_editor::Content::with_text(self.source.text());
    }

    // ------------------------------------------------------------ 辅助

    fn fail(&mut self, message: &str) {
        self.last_error = Some(message.to_string());
        self.push_log(format!("错误：{message}"));
    }

    fn push_log(&mut self, line: impl Into<String>) {
        self.log.push(line.into());
        if self.log.len() > 200 {
            self.log.remove(0);
        }
    }

    // ------------------------------------------------------------ 视图

    fn view(&self) -> Element<'_, Message> {
        let visual_body = self.core.document().to_plain_text();
        let preview_body = self.core.document().to_plain_text();
        let focus_line = format!(
            "焦点 {:?} / revision {} / 动作 {}",
            self.focus,
            self.core.revision(),
            self.core.history().len()
        );
        let preedit_line = if self.preedit.is_empty() {
            "预编辑：（无）".to_string()
        } else {
            format!("预编辑：「{}」", self.preedit)
        };

        let visual_pane = column![
            text("正文（结构编辑）").size(18),
            text(focus_line).size(13),
            text(preedit_line).size(13),
            scrollable(container(text(visual_body).size(18)).padding(8)).height(Length::Fill),
        ]
        .spacing(6);

        let source_state = if self.source.is_writable() {
            "可写"
        } else {
            "只读"
        };
        let source_pane = column![
            text(format!("源码 Source Studio [{source_state}]")).size(18),
            text(format!(
                "方言 .{} / {} 行 / {} 字节",
                self.source.dialect().extension(),
                self.source.line_count(),
                self.source.len_bytes()
            ))
            .size(13),
            text_editor(&self.source_content)
                .on_action(Message::SourceEdit)
                .height(Length::Fill),
        ]
        .spacing(6);

        let preview_pane = column![
            text("预览（Typst 快速预览占位）").size(18),
            text("占位，不冒充最终排版").size(13),
            scrollable(container(text(preview_body).size(15)).padding(8)).height(Length::Fill),
        ]
        .spacing(6);

        let controls = row![
            button("插入文本").on_press(Message::InsertSample),
            button("包裹分数").on_press(Message::Wrap(NodeKind::Fraction)),
            button("包裹根式").on_press(Message::Wrap(NodeKind::Sqrt)),
            button("解除包裹").on_press(Message::Unwrap),
            button("循环变体").on_press(Message::CycleVariant),
            button("上").on_press(Message::Move(Direction::Up)),
            button("下").on_press(Message::Move(Direction::Down)),
            button("父级").on_press(Message::Move(Direction::Parent)),
            button("子级").on_press(Message::Move(Direction::FirstChild)),
            button("撤销").on_press(Message::Undo),
            button("切换源码可写").on_press(Message::ToggleSourceWritable),
        ]
        .spacing(6);

        let font_state = if self.cjk_font { "已加载" } else { "缺失" };
        let status_line = format!(
            "事件 {} / 预编辑事件 {} / IME 提交 {} / 核心动作 {} / CJK 字体 {font_state} / 错误 {}",
            self.events,
            self.preedit_events,
            self.commits,
            self.core.history().len(),
            self.last_error.as_deref().unwrap_or("无")
        );

        let log_lines: Vec<Element<'_, Message>> = self
            .log
            .iter()
            .rev()
            .take(6)
            .map(|line| text(line.clone()).size(12).into())
            .collect();

        let panes = row![
            container(visual_pane)
                .width(Length::FillPortion(1))
                .height(Length::Fill),
            container(source_pane)
                .width(Length::FillPortion(1))
                .height(Length::Fill),
            container(preview_pane)
                .width(Length::FillPortion(1))
                .height(Length::Fill),
        ]
        .spacing(8)
        .height(Length::Fill);

        let body = column![
            controls,
            panes,
            text(status_line).size(13),
            column(log_lines).spacing(2),
        ]
        .spacing(8);

        container(body).padding(10).into()
    }
}

/// 用远端 actor 注入夹具，保证本地 undo scope 从空开始。
fn build_fixture(core: &mut Editor, paragraph: NodeId) {
    let math = fixture_create(core, paragraph, 0, 1, NodeKind::Math);
    let fraction = fixture_create(core, math, 0, 0, NodeKind::Fraction);
    let script = fixture_create(core, math, 0, 1, NodeKind::Script);
    let matrix = fixture_create(core, math, 0, 2, NodeKind::Matrix);

    let num = slot0(core, fraction);
    let den = slot1(core, fraction);
    let base = slot0(core, script);
    let sub = slot1(core, script);
    let sup = slot2(core, script);
    let cell = slot0(core, matrix);

    fixture_type(core, num, "a");
    fixture_type(core, den, "b");
    fixture_type(core, base, "x");
    fixture_type(core, sub, "1");
    fixture_type(core, sup, "2");
    fixture_type(core, cell, "1");
}

fn fixture_create(
    core: &mut Editor,
    parent: NodeId,
    slot: usize,
    index: usize,
    kind: NodeKind,
) -> NodeId {
    let edit = SemanticEdit::InsertNode {
        parent,
        slot,
        index,
        kind,
    };
    core.apply_remote(RemoteEdit { actor: FIXTURE, edit })
        .expect("夹具插入结构")
        .created
        .expect("结构插入应返回新节点")
}

fn fixture_type(core: &mut Editor, node: NodeId, text: &str) {
    let edit = SemanticEdit::InsertText {
        node,
        at: 0,
        text: text.to_string(),
    };
    core.apply_remote(RemoteEdit { actor: FIXTURE, edit })
        .expect("夹具插入文本");
}

fn slot0(core: &Editor, node: NodeId) -> NodeId {
    child(core, node, 0, 0)
}

fn slot1(core: &Editor, node: NodeId) -> NodeId {
    child(core, node, 1, 0)
}

fn slot2(core: &Editor, node: NodeId) -> NodeId {
    child(core, node, 2, 0)
}

fn child(core: &Editor, node: NodeId, slot: usize, index: usize) -> NodeId {
    *core
        .document()
        .slot(node, slot)
        .expect("槽位存在")
        .get(index)
        .expect("子节点存在")
}
