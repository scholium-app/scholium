//! Iced 候选最小工程：原生窗口 + 正文/源码/预览三区，复用 `scholium-spike-core`。
//!
//! 设计约束（对照 `docs/ARCHITECTURE.md` 与 `docs/modules/app.md`）：
//!
//! - 正文的权威状态在 core；UI 只保存投影，不持有第二份可写正文。
//! - 输入法预编辑只存在于 UI 状态，**不进历史**；`Commit` 才产生一个核心动作。
//! - 源码面板在非活动语言下不可写：控件的编辑会被权威缓冲回滚，而不是留在前端。

mod fixture;
mod ime_host;
mod structure_view;

use iced::advanced::input_method;
use iced::keyboard::key::Named;
use iced::keyboard::{self, Key};
use iced::widget::{
    button, canvas, column, container, mouse_area, row, scrollable, text, text_editor,
};
use iced::{Color, Element, Font, Length, Point, Rectangle, Size, Subscription, Task};

use ime_host::ImeHost;
use scholium_spike_core::cursor::move_cursor;
use scholium_spike_core::{
    ActorId, Cursor, Dialect, Direction, Editor, Intent, Layout, NodeId, NodeKind, SemanticEdit,
    SourcePane, layout_document,
};
use structure_view::StructureView;

/// 本机 CJK 字体。缺失时应用仍启动，但界面会报告字体缺口。
const CJK_FONT_PATH: &str = "/usr/share/fonts/adobe-source-han-serif/SourceHanSerifCN-Regular.otf";
const CJK_FAMILY: &str = "Source Han Serif CN";

/// 本地写入者。夹具由 `fixture` 模块用另一个 actor 注入，避免污染本地 undo scope。
const LOCAL: ActorId = ActorId(1);

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

/// 当前持有键盘与输入法的区域。
///
/// 输入法事件是**窗口级**的，不带目标控件信息。应用必须自己记住输入焦点在哪个区域，
/// 否则会把源码面板的输入法提交错误地写进正文。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputArea {
    /// 正文结构编辑区。自绘，因此必须自己请求输入法。
    Visual,
    /// 源码面板。只有可写时才是真正的编辑控件。
    Source,
}

/// 应用状态。
struct App {
    core: Editor,
    focus: Cursor,
    /// 输入焦点所在区域。
    input_area: InputArea,
    /// 当前布局。每次编辑后由共享核心重算，正文区据此绘制结构。
    layout: Layout,
    /// 正文绘制使用的字体。
    view_font: Font,
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
    /// 点击正文区，取得输入焦点。
    FocusVisual,
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
        fixture::build(&mut core, paragraph);

        let source = SourcePane::new(Dialect::Latex, SOURCE_INITIAL);
        let source_content = text_editor::Content::with_text(source.text());
        let focus = core
            .document()
            .first_text_descendant(paragraph)
            .map(|node| Cursor::Text { node, byte: 0 })
            .expect("段落有文本叶子");

        Self {
            layout: layout_document(core.document()),
            view_font: if cjk_font {
                Font::with_name(CJK_FAMILY)
            } else {
                Font::default()
            },
            core,
            focus,
            input_area: InputArea::Visual,
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
            Message::FocusVisual => {
                self.input_area = InputArea::Visual;
                self.push_log("输入焦点 → 正文");
            }
            Message::Preedit(text) => {
                // 输入法事件是窗口级的：只有正文区持有输入焦点时才归属它。
                if self.input_area == InputArea::Visual {
                    self.on_preedit(text);
                }
            }
            Message::Commit(text) => {
                if self.input_area == InputArea::Visual {
                    self.on_commit(text);
                } else {
                    self.push_log("忽略输入法提交：当前输入焦点不在正文区");
                }
            }
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
        // 任何消息之后都重算布局：正文区绘制的是共享核心的真实布局，不是纯文本投影。
        self.layout = layout_document(self.core.document());
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
        self.input_area = InputArea::Source;
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
        let line = line.into();
        // 同时写 stdout，便于脚本与日志对事件时间线取证。
        println!("{line}");
        self.log.push(line);
        if self.log.len() > 200 {
            self.log.remove(0);
        }
    }

    // ------------------------------------------------------------ 视图

    fn view(&self) -> Element<'_, Message> {
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
            text("正文（结构编辑 · 结构渲染）").size(18),
            text(focus_line).size(13),
            text(preedit_line).size(13),
            canvas(StructureView {
                layout: &self.layout,
                font: self.view_font,
                color: Color::from_rgb(0.90, 0.90, 0.93),
            })
            .width(Length::Fill)
            .height(Length::Fill),
        ]
        .spacing(6);

        let source_state = if self.source.is_writable() {
            "可写"
        } else {
            "只读"
        };
        // 只读面板不应该是可编辑控件：既是 UX 正确，也避免它抢走输入法焦点。
        // （实测：把它渲染成 text_editor 时，输入法提交会落到源码控件而不是正文。）
        let source_body: Element<'_, Message> = if self.source.is_writable() {
            text_editor(&self.source_content)
                .on_action(Message::SourceEdit)
                .height(Length::Fill)
                .into()
        } else {
            scrollable(container(text(self.source.text().to_string()).size(14)).padding(8))
                .height(Length::Fill)
                .into()
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
            source_body,
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

        // 正文区是自绘的，必须自己请求输入法；点击它取得输入焦点。
        let visual_host = ImeHost::new(
            mouse_area(container(visual_pane)).on_press(Message::FocusVisual),
            self.input_area == InputArea::Visual,
            Rectangle::new(Point::new(24.0, 96.0), Size::new(2.0, 20.0)),
        );

        let panes = row![
            container(visual_host)
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

