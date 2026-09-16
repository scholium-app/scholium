//! egui 候选最小工程：原生窗口 + 正文/源码/预览三区，复用 `scholium-spike-core`。
//!
//! 为什么第四个候选值得单独验证（前三者都不具备）：**`eframe` 默认启用 `accesskit`**，
//! Linux 上经 `accesskit_unix` 接到 AT-SPI；egui 还自带 `TextEdit`，并在 `egui-winit` 里
//! 处理 `Ime::Preedit` / `Ime::Commit`。
//!
//! 与其它候选相同的约束：正文权威在 core，UI 只保存投影；结构绘制的是**同一份** `core::layout`。

use std::sync::Arc;

use eframe::egui;
use scholium_spike_core::{
    ActorId, Cursor, Dialect, Direction, Editor, Intent, Item, Layout, NodeId, SemanticEdit,
    SourcePane, cursor::move_cursor, fixture, layout_document,
};

/// 本地写入者。夹具由核心用另一个 actor 注入，本地 undo scope 从空开始。
const LOCAL: ActorId = ActorId(1);

const CJK_FONT_PATH: &str = "/usr/share/fonts/adobe-source-han-serif/SourceHanSerifCN-Regular.otf";
const SOURCE_INITIAL: &str =
    "\\documentclass{article}\n\\begin{document}\n正文与 $a/b$ 公式\n\\end{document}\n";

/// egui 候选的应用状态。
pub struct SpikeApp {
    core: Editor,
    layout: Layout,
    focus: Cursor,
    source: SourcePane,
    source_buffer: String,
    /// 输入法预编辑串。只属于 UI，**不进历史**。
    preedit: String,
    last_event: String,
    preedit_events: usize,
    commits: usize,
    font_loaded: bool,
    /// 正文自绘区是否持有输入焦点。输入法事件是全局的，必须自己记住归属。
    structure_focused: bool,
    /// 是否已经做过启动时的默认聚焦。
    initial_focus_done: bool,
    /// 上一次打印到 stdout 的事件，用于让测试日志能取证。
    printed: String,
    /// 是否已经打印过"向平台请求输入法"，用于诊断。
    ime_requested: bool,
    /// 正文布局在屏幕上的原点。绘制时记录，供命中测试（点击定位）与测试使用。
    structure_origin: egui::Pos2,
    /// 诊断用：把输入焦点交给源码面板的 `TextEdit`，用于分辨"自绘区请求方式不全"与
    /// "eframe 这一层根本没接输入法"。由环境变量 `SCHOLIUM_FOCUS_SOURCE=1` 打开。
    focus_source: bool,
}

impl SpikeApp {
    /// 建立应用。`font` 为 CJK 字体字节，`None` 时使用 egui 内置字体并在界面报告缺口。
    pub fn new(egui_ctx: &egui::Context, font: Option<Vec<u8>>) -> Self {
        let font_loaded = font.is_some();
        if let Some(bytes) = font
            && let Err(error) = install_cjk_font(egui_ctx, bytes)
        {
            eprintln!("警告：加载 CJK 字体失败：{error}");
        }

        let mut core = Editor::new();
        fixture::build_standard(&mut core);
        let layout = layout_document(core.document());
        let focus_source = std::env::var("SCHOLIUM_FOCUS_SOURCE").is_ok();
        let mut source = SourcePane::new(Dialect::Latex, SOURCE_INITIAL);
        if focus_source {
            // 诊断路径需要源码面板可编辑，才会渲染出 TextEdit。
            source.set_writable(true);
        }
        let focus = {
            let document = core.document();
            document
                .first_text_descendant(document.root())
                .map(|node| Cursor::Text { node, byte: 0 })
                .expect("文档有文本叶子")
        };

        Self {
            core,
            layout,
            focus,
            source_buffer: source.text().to_string(),
            source,
            preedit: String::new(),
            last_event: "启动完成".to_string(),
            preedit_events: 0,
            commits: 0,
            font_loaded,
            structure_focused: false,
            initial_focus_done: false,
            printed: String::new(),
            ime_requested: false,
            structure_origin: egui::Pos2::ZERO,
            focus_source,
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
            self.last_event = "没有可编辑的文本槽位".to_string();
            return;
        };
        let edit = SemanticEdit::InsertText {
            node,
            at,
            text: text.to_string(),
        };
        match self.core.apply(LOCAL, intent, edit) {
            Ok(Some(_)) => {
                self.focus = Cursor::Text {
                    node,
                    byte: at + text.len(),
                };
                self.last_event = format!("core 收到 {intent:?}：\"{text}\"");
            }
            Ok(None) => self.last_event = "空操作".to_string(),
            Err(error) => self.last_event = format!("错误：{error}"),
        }
    }

    fn navigate(&mut self, direction: Direction) {
        match move_cursor(self.core.document(), self.focus, direction) {
            Ok(Some(next)) => {
                self.focus = next;
                self.last_event = format!("焦点 → {:?}", next.focus().index());
            }
            Ok(None) => self.last_event = format!("{direction:?}：没有可取位置"),
            Err(error) => self.last_event = format!("错误：{error}"),
        }
    }

    /// 处理键盘与输入法事件。
    ///
    /// 输入法事件是全局的，因此这里统一按"当前焦点在正文区"处理；源码面板只读时不参与。
    fn handle_input(&mut self, ctx: &egui::Context) {
        // 输入焦点不在正文区时，输入法与按键都不属于它；否则会与源码编辑器抢输入
        // （这正是 Iced 候选当初踩过的坑）。
        if !self.structure_focused {
            return;
        }
        let events = ctx.input(|input| input.events.clone());
        for event in events {
            match event {
                egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => {
                    self.preedit_events += 1;
                    self.preedit = text;
                    self.last_event = format!("预编辑「{}」（未进历史）", self.preedit);
                }
                egui::Event::Ime(egui::ImeEvent::Commit(text)) => {
                    self.preedit.clear();
                    if text.is_empty() {
                        self.last_event = "IME 提交空串，忽略".to_string();
                    } else {
                        self.commits += 1;
                        self.insert_text(&text, Intent::ImeCommit);
                    }
                }
                egui::Event::Text(text) => {
                    // 非输入法路径的直接文本输入。
                    if self.preedit.is_empty() && !text.is_empty() {
                        self.insert_text(&text, Intent::Typing);
                    }
                }
                _ => {}
            }
        }

        let (down, up, left, right, backspace, undo) = ctx.input(|input| {
            (
                input.key_pressed(egui::Key::ArrowDown),
                input.key_pressed(egui::Key::ArrowUp),
                input.key_pressed(egui::Key::ArrowLeft),
                input.key_pressed(egui::Key::ArrowRight),
                input.key_pressed(egui::Key::Backspace),
                input.modifiers.command && input.key_pressed(egui::Key::Z),
            )
        });

        if down {
            self.navigate(Direction::Down);
        }
        if up {
            self.navigate(Direction::Up);
        }
        if left {
            self.navigate(Direction::Parent);
        }
        if right {
            self.navigate(Direction::FirstChild);
        }
        if backspace {
            self.delete_backward();
        }
        if undo {
            self.undo();
        }
    }

    fn delete_backward(&mut self) {
        let Some((node, at)) = self.target() else {
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
                self.last_event = "core：删除一个字素".to_string();
            }
            Ok(None) => self.last_event = "退格：已在开头".to_string(),
            Err(error) => self.last_event = format!("错误：{error}"),
        }
    }

    fn undo(&mut self) {
        match self.core.undo(LOCAL) {
            Ok(Some(outcome)) => {
                self.last_event = format!(
                    "core：撤销 {:?}，删除 {} 字符",
                    outcome.undone, outcome.removed_chars
                );
            }
            Ok(None) => self.last_event = "撤销：本地没有可撤销动作".to_string(),
            Err(error) => self.last_event = format!("错误：{error}"),
        }
    }

    /// 把共享布局画到给定区域。
    fn paint_structure(painter: &egui::Painter, origin: egui::Pos2, layout: &Layout, color: egui::Color32) {
        for item in &layout.items {
            match item {
                Item::Text {
                    x,
                    baseline,
                    size,
                    content,
                    ..
                } => {
                    painter.text(
                        // 布局给的是基线，绘制需要上沿；换算走 core 的统一约定。
                        origin + egui::vec2(*x, Item::top_of(*baseline, *size)),
                        egui::Align2::LEFT_TOP,
                        content,
                        egui::FontId::proportional(*size),
                        color,
                    );
                }
                Item::Rule {
                    x,
                    y,
                    width,
                    height,
                } => {
                    painter.rect_filled(
                        egui::Rect::from_min_size(
                            origin + egui::vec2(*x, *y),
                            egui::vec2(*width, *height),
                        ),
                        0.0,
                        color,
                    );
                }
            }
        }
    }
}

impl SpikeApp {
    /// 画一帧。与 eframe 解耦，因此可以在无窗口的测试里用 `Context::run_ui` 驱动。
    pub fn draw(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.handle_input(&ctx);
        self.layout = layout_document(self.core.document());

        // 事件时间线打到 stdout，便于脚本取证而不必读截图。
        if self.last_event != self.printed {
            println!(
                "[event] {} | revision {} | 核心动作 {} | 预编辑事件 {} | IME 提交 {}",
                self.last_event,
                self.core.revision(),
                self.core.history().len(),
                self.preedit_events,
                self.commits
            );
            self.printed = self.last_event.clone();
        }

        ui.vertical(|ui| {
            let panes = ui.available_width() / 3.0;
            ui.horizontal_top(|ui| {
                // 正文：结构渲染（绘制的仍是共享核心的布局）
                ui.allocate_ui(egui::vec2(panes, ui.available_height()), |ui| {
                    ui.vertical(|ui| {
                        ui.heading("正文（结构编辑 · 结构渲染）");
                        ui.label(format!("焦点 {:?}", self.focus));
                        let size = ui.available_size();
                        let (rect, response) =
                            ui.allocate_exact_size(size, egui::Sense::click_and_drag());
                        // 启动后默认聚焦正文区：编辑器打开就该能直接打字，
                        // 而不是要求用户先点一下（也让键盘注入的自动测试有意义）。
                        if response.clicked() || (!self.initial_focus_done && !self.focus_source) {
                            response.request_focus();
                            self.initial_focus_done = true;
                        }
                        let focused = response.has_focus();
                        self.structure_focused = focused;

                        let painter = ui.painter_at(rect);
                        let origin = rect.min + egui::vec2(8.0, 8.0);
                        self.structure_origin = origin;

                        // 点哪定位哪：把点击位置映射回 (节点, 字节偏移) 并移动插入点。
                        if let Some(position) = response.interact_pointer_pos() {
                            let local = position - origin;
                            if let Some((node, byte)) = self.layout.hit_test(local.x, local.y) {
                                self.focus = Cursor::Text { node, byte };
                            }
                        }
                        SpikeApp::paint_structure(
                            &painter,
                            origin,
                            &self.layout,
                            egui::Color32::from_rgb(230, 230, 233),
                        );

                        // 光标几何由共享布局按焦点算出：既是可见光标，也是输入法候选窗的锚点。
                        let caret = match self.focus {
                            Cursor::Text { node, byte } => self.layout.caret(node, byte),
                            Cursor::Slot { .. } => None,
                        };
                        let caret_rect = caret.map(|caret| {
                            let top = Item::top_of(caret.baseline, caret.size);
                            egui::Rect::from_min_size(
                                origin + egui::vec2(caret.x, top),
                                egui::vec2(1.5, caret.size * 1.2),
                            )
                        });
                        if focused
                            && let Some(rect) = caret_rect
                        {
                            painter.rect_filled(
                                rect,
                                0.0,
                                egui::Color32::from_rgb(120, 190, 255),
                            );
                        }

                        if focused {
                            // 自绘区必须自己声明输入法归属，并把光标位置交给它。
                            //
                            // 注意：egui-winit 在 Wayland 下用 `IMEOutput.rect` 调用
                            // `set_ime_cursor_area`（不是 `cursor_rect`，见 egui-winit 的
                            // handle_platform_output_inner）。所以这里必须传**光标处的
                            // 小矩形**；传整块面板会把候选窗锚在面板左上角。
                            let anchor = caret_rect.unwrap_or_else(|| {
                                egui::Rect::from_min_size(origin, egui::vec2(1.5, 20.0))
                            });
                            ui.output_mut(|output| {
                                output.ime = Some(egui::output::IMEOutput {
                                    purpose: egui::IMEPurpose::Normal,
                                    rect: anchor,
                                    cursor_rect: anchor,
                                    should_interrupt_composition: false,
                                });
                            });
                            if !self.ime_requested {
                                self.ime_requested = true;
                                println!(
                                    "[ime] 已向平台声明输入法归属：anchor=({:.0},{:.0},{:.0},{:.0})",
                                    anchor.min.x,
                                    anchor.min.y,
                                    anchor.width(),
                                    anchor.height()
                                );
                            }

                            // 自绘内容不会自动进对象树，必须显式补一条可访问描述。
                            // `widget_info` 接收的是可多次调用的闭包，因此这里 clone。
                            let description = self.core.document().to_plain_text();
                            response.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::WidgetType::Label,
                                    true,
                                    description.clone(),
                                )
                            });
                        }
                    });
                });

                // 源码：只读时不给可编辑控件，与其它候选同一约束
                ui.allocate_ui(egui::vec2(panes, ui.available_height()), |ui| {
                    ui.vertical(|ui| {
                        ui.heading(format!(
                            "源码 Source Studio [{}]",
                            if self.source.is_writable() {
                                "可写"
                            } else {
                                "只读"
                            }
                        ));
                        ui.label(format!(
                            "方言 .{} / {} 行",
                            self.source.dialect().extension(),
                            self.source.line_count()
                        ));
                        if self.source.is_writable() {
                            let response = ui.add(
                                egui::TextEdit::multiline(&mut self.source_buffer)
                                    .id(egui::Id::new("source_editor")),
                            );
                            if response.changed() {
                                match self.source.set_text(&self.source_buffer) {
                                    Ok(()) => {
                                        self.last_event =
                                            format!("源码内容：{:?}", self.source_buffer);
                                    }
                                    Err(error) => {
                                        self.last_event = format!("源码写入被拒：{error}");
                                        self.source_buffer = self.source.text().to_string();
                                    }
                                }
                            }
                            // 诊断路径：让自带控件先拿焦点，看它能不能收到输入法。
                            if self.focus_source {
                                ui.memory_mut(|memory| {
                                    memory.request_focus(egui::Id::new("source_editor"));
                                });
                            }
                        } else {
                            ui.label(self.source.text());
                        }
                    });
                });

                // 预览：投影占位，明确标注
                ui.allocate_ui(egui::vec2(panes, ui.available_height()), |ui| {
                    ui.vertical(|ui| {
                        ui.heading("预览（Typst 快速预览占位）");
                        ui.label("占位，不冒充最终排版");
                        ui.label(self.core.document().to_plain_text());
                    });
                });
            });

            ui.separator();
            ui.label(format!(
                "revision {} / 核心动作 {} / 预编辑事件 {} / IME 提交 {} / CJK 字体 {} / 最近：{}",
                self.core.revision(),
                self.core.history().len(),
                self.preedit_events,
                self.commits,
                if self.font_loaded { "已加载" } else { "缺失" },
                self.last_event
            ));
        });
    }
}

/// 把 CJK 字体装进 egui，否则中文会渲染成缺字方块。
fn install_cjk_font(ctx: &egui::Context, bytes: Vec<u8>) -> Result<(), String> {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "cjk".to_owned(),
        Arc::new(egui::FontData::from_owned(bytes)),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "cjk".to_owned());
    ctx.set_fonts(fonts);
    Ok(())
}

impl eframe::App for SpikeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.draw(ui);
    }
}

/// 读取本机 CJK 字体；缺失时返回 `None`（应用仍启动并在界面报告缺口）。
pub fn load_cjk_font() -> Option<Vec<u8>> {
    std::fs::read(CJK_FONT_PATH).ok()
}

/// 测试与集成用的只读访问。
impl SpikeApp {
    /// 核心编辑器。
    pub fn core(&self) -> &Editor {
        &self.core
    }

    /// 当前焦点。
    pub fn focus(&self) -> Cursor {
        self.focus
    }

    /// 观察到的预编辑事件数。
    pub fn preedit_events(&self) -> usize {
        self.preedit_events
    }

    /// 观察到的 IME 提交次数。
    pub fn commits(&self) -> usize {
        self.commits
    }

    /// 正文的纯文本投影，用于断言内容。
    pub fn plain_text(&self) -> String {
        self.core.document().to_plain_text()
    }

    /// 正文区当前是否持有输入焦点。
    pub fn structure_focused(&self) -> bool {
        self.structure_focused
    }

    /// 正文布局在屏幕上的原点（绘制时记录）。
    pub fn structure_origin(&self) -> egui::Pos2 {
        self.structure_origin
    }
}
