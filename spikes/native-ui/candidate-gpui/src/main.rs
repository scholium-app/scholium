//! GPUI 候选最小工程：原生窗口 + 正文/源码/预览三区，复用 `scholium-spike-core`。
//!
//! 与 Iced 候选的差别（全部记录在验证报告里）：
//!
//! - GPUI 核心**没有内置文本输入控件**，文本编辑与输入法必须自行实现（`InputHandler`）。
//! - GPUI 0.2.2 不含 accesskit，与 iced 一样不发布可访问对象树。
//! - 结构渲染用绝对定位元素完成（iced 用 canvas），但绘制的是**同一份** `core::layout`，
//!   因此两者的结构渲染结果可比。

use gpui::{
    AnyElement, App, Application, Bounds, Context, SharedString, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, rgb, size,
};
use scholium_spike_core::{Dialect, Editor, Item, Layout, SourcePane, fixture, layout_document};

/// 本机 CJK 字体族名。GPUI 用 font-kit 枚举系统字体，可直接按族名引用。
const CJK_FAMILY: &str = "Source Han Serif CN";

const SOURCE_INITIAL: &str =
    "\\documentclass{article}\n\\begin{document}\n正文与 $a/b$ 公式\n\\end{document}\n";

struct SpikeView {
    core: Editor,
    layout: Layout,
    source: SourcePane,
}

impl SpikeView {
    fn new() -> Self {
        let mut core = Editor::new();
        fixture::build_standard(&mut core);
        let layout = layout_document(core.document());
        let source = SourcePane::new(Dialect::Latex, SOURCE_INITIAL);
        Self {
            core,
            layout,
            source,
        }
    }

    /// 三区中的一个面板。
    fn pane(&self, title: String, body: AnyElement) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .p_2()
            .gap_1()
            .child(div().text_size(px(16.)).child(SharedString::from(title)))
            .child(div().flex_1().overflow_hidden().child(body))
            .into_any_element()
    }

    /// 把共享布局渲染成绝对定位元素。
    ///
    /// `Item::Text` 变成定位文本，`Item::Rule` 变成定位实心矩形（分数线与根号横线）。
    fn structure(&self) -> AnyElement {
        let mut container = div().relative().w_full().h_full();
        for item in &self.layout.items {
            container = match item {
                Item::Text {
                    x,
                    y,
                    size,
                    content,
                } => container.child(
                    div()
                        .absolute()
                        .left(px(*x))
                        .top(px(*y))
                        .text_size(px(*size))
                        .font_family(CJK_FAMILY)
                        .child(SharedString::from(content.clone())),
                ),
                Item::Rule {
                    x,
                    y,
                    width,
                    height,
                } => container.child(
                    div()
                        .absolute()
                        .left(px(*x))
                        .top(px(*y))
                        .w(px(*width))
                        .h(px(*height))
                        .bg(rgb(0xe6e6e9)),
                ),
            };
        }
        container.into_any_element()
    }
}

impl Render for SpikeView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let focus_line = format!(
            "焦点来自 core / revision {} / 动作 {}",
            self.core.revision(),
            self.core.history().len()
        );

        let source_body = div()
            .flex()
            .flex_col()
            .child(div().text_size(px(12.)).child(format!(
                "方言 .{} / {} 字节 / {} 行",
                self.source.dialect().extension(),
                self.source.len_bytes(),
                self.source.line_count()
            )))
            .child(
                div()
                    .text_size(px(14.))
                    .font_family(CJK_FAMILY)
                    .child(SharedString::from(self.source.text().to_string())),
            )
            .into_any_element();

        let preview_body = div()
            .text_size(px(14.))
            .child(SharedString::from(self.core.document().to_plain_text()))
            .into_any_element();

        let status = format!(
            "revision {} / 核心动作 {} / 源码 {} 行 / 说明：GPUI 核心无内置文本输入控件，\
             文本编辑与输入法需自行实现（InputHandler），本骨架尚未接入",
            self.core.revision(),
            self.core.history().len(),
            self.source.line_count()
        );

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x1e1e20))
            .text_color(rgb(0xe6e6e9))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .gap_2()
                    .child(self.pane(
                        format!("正文（结构编辑 · 结构渲染）  {focus_line}"),
                        self.structure(),
                    ))
                    .child(self.pane("源码 Source Studio [只读]".to_string(), source_body))
                    .child(self.pane("预览（Typst 快速预览占位）".to_string(), preview_body)),
            )
            .child(div().text_size(px(12.)).child(SharedString::from(status)))
    }
}

/// 启动候选应用。
fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1280.), px(820.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("Scholium spike — GPUI candidate".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(|_| SpikeView::new()),
        )
        .expect("打开窗口");
        cx.activate(true);
    });
}
