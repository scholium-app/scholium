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

struct SpikeApp {
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
}

impl SpikeApp {
    fn new(cc: &eframe::CreationContext<'_>, font: Option<Vec<u8>>) -> Self {
        let font_loaded = font.is_some();
        if let Some(bytes) = font
            && let Err(error) = install_cjk_font(&cc.egui_ctx, bytes)
        {
            eprintln!("警告：加载 CJK 字体失败：{error}");
        }

        let mut core = Editor::new();
        fixture::build_standard(&mut core);
        let layout = layout_document(core.document());
        let source = SourcePane::new(Dialect::Latex, SOURCE_INITIAL);
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

impl eframe::App for SpikeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_input(&ctx);
        self.layout = layout_document(self.core.document());

        ui.vertical(|ui| {
            let panes = ui.available_width() / 3.0;
            ui.horizontal_top(|ui| {
                // 正文：结构渲染（绘制的仍是共享核心的布局）
                ui.allocate_ui(egui::vec2(panes, ui.available_height()), |ui| {
                    ui.vertical(|ui| {
                        ui.heading("正文（结构编辑 · 结构渲染）");
                        ui.label(format!("焦点 {:?}", self.focus));
                        let size = ui.available_size();
                        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                        let painter = ui.painter_at(rect);
                        SpikeApp::paint_structure(
                            &painter,
                            rect.min + egui::vec2(8.0, 8.0),
                            &self.layout,
                            egui::Color32::from_rgb(230, 230, 233),
                        );
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
                            let response =
                                ui.add(egui::TextEdit::multiline(&mut self.source_buffer));
                            if response.changed()
                                && let Err(error) = self.source.set_text(&self.source_buffer)
                            {
                                self.last_event = format!("源码写入被拒：{error}");
                                self.source_buffer = self.source.text().to_string();
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

fn main() -> eframe::Result {
    let font = std::fs::read(CJK_FONT_PATH).ok();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_title("Scholium spike — egui candidate"),
        ..Default::default()
    };
    eframe::run_native(
        "Scholium spike — egui candidate",
        options,
        Box::new(move |cc| Ok(Box::new(SpikeApp::new(cc, font)))),
    )
}
