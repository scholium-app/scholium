//! egui 候选最小工程：原生窗口 + 正文/源码/预览三区，复用 `scholium-spike-core`。
//!
//! 为什么第四个候选值得单独验证（前三者都不具备）：**`eframe` 默认启用 `accesskit`**，
//! Linux 上经 `accesskit_unix` 接到 AT-SPI；egui 还自带 `TextEdit`，并在 `egui-winit` 里
//! 处理 `Ime::Preedit` / `Ime::Commit`。
//!
//! 与其它候选相同的约束：正文权威在 core，UI 只保存投影；结构绘制的是**同一份** `core::layout`。

mod accessibility;
mod input;
mod preview;
mod source_ui;
mod structure_ui;

use std::sync::Arc;

use eframe::egui;
use scholium_spike_core::{
    ActorId, Cursor, Direction, Editor, Intent, Item, Layout, NodeId, NodeKind, Selection,
    SemanticEdit, cursor::move_cursor, fixture, layout_document,
};

/// 本地写入者。夹具由核心用另一个 actor 注入，本地 undo scope 从空开始。
const LOCAL: ActorId = ActorId(1);

const CJK_FONT_PATH: &str = "/usr/share/fonts/adobe-source-han-serif/SourceHanSerifCN-Regular.otf";
/// egui 候选的应用状态。
pub struct SpikeApp {
    core: Editor,
    layout: Layout,
    focus: Cursor,
    source: scholium_spike_reconcile::Session,
    preview: preview::Preview,
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
    /// 结构选区。折叠时等价于单个光标。
    selection: Selection,
    /// 指针按下时的锚点。自己跟踪而不依赖框架的拖拽判定，行为更可预期。
    press_anchor: Option<Cursor>,
    /// 诊断用：把输入焦点交给源码面板的 `TextEdit`，用于分辨"自绘区请求方式不全"与
    /// "eframe 这一层根本没接输入法"。由环境变量 `SCHOLIUM_FOCUS_SOURCE=1` 打开。
    focus_source: bool,
}

impl SpikeApp {
    /// # Panics
    /// 标准夹具违反“文档至少有一个文本叶子”的不变量时 panic。
    ///
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
        let source = scholium_spike_reconcile::Session::new(
            &core,
            scholium_spike_reconcile::generate::Dialect::Latex,
        );
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
            source_buffer: source.generated.text.clone(),
            preview: preview::Preview::default(),
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
            selection: Selection::collapsed(focus),
            press_anchor: None,
            focus_source,
        }
    }

    fn paint_structure(
        painter: &egui::Painter,
        origin: egui::Pos2,
        layout: &Layout,
        color: egui::Color32,
    ) {
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
            ui.columns(3, |columns| {
                self.draw_structure(&mut columns[0]);
                self.draw_source(&mut columns[1]);
                self.preview.draw(&mut columns[2], &self.core);
            });

            ui.separator();
            ui.label(format!(
                "revision {} / 核心动作 {} / 预编辑事件 {} / IME 提交 {} / CJK 字体 {} / 最近：{}",
                self.core.revision(),
                self.core.history().len(),
                self.preedit_events,
                self.commits,
                if self.font_loaded {
                    "已加载"
                } else {
                    "缺失"
                },
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

    /// 当前结构选区。
    pub fn selection(&self) -> Selection {
        self.selection
    }

    /// 测试与基准用：替换核心文档并立即重算布局。
    pub fn replace_document_for_test(&mut self, core: Editor) {
        self.core = core;
        self.layout = layout_document(self.core.document());
    }
}
