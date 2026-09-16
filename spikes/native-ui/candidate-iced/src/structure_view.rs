//! 用 canvas 绘制共享核心的布局结果。
//!
//! 这是"数学结构"验收的关键：正文必须真的画出分数线、上标位置与矩阵网格，
//! 而不是把文档投影成纯文本。布局由 `scholium_spike_core::layout` 提供，所有候选共用同一份，
//! 这样候选之间才可比。

use iced::advanced::text::{Alignment, LineHeight, Shaping};
use iced::widget::canvas;
use iced::{Color, Font, Point, Rectangle, Renderer, Size, Theme, mouse};
use scholium_spike_core::{Item, Layout};

/// 结构视图的绘制程序。
pub struct StructureView<'a> {
    /// 共享核心产生的布局。
    pub layout: &'a Layout,
    /// 文本字体。CJK 必须显式给字体，否则会退化成缺字方块。
    pub font: Font,
    /// 文本与线条颜色。
    pub color: Color,
}

impl<Message> canvas::Program<Message> for StructureView<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        for item in &self.layout.items {
            match item {
                Item::Text {
                    x,
                    y,
                    size,
                    content,
                } => {
                    frame.fill_text(canvas::Text {
                        content: content.clone(),
                        position: Point::new(*x, *y),
                        max_width: bounds.width,
                        color: self.color,
                        size: (*size).into(),
                        line_height: LineHeight::default(),
                        font: self.font,
                        align_x: Alignment::Left,
                        align_y: iced::alignment::Vertical::Top,
                        shaping: Shaping::Basic,
                    });
                }
                Item::Rule {
                    x,
                    y,
                    width,
                    height,
                } => {
                    frame.fill_rectangle(
                        Point::new(*x, *y),
                        Size::new(*width, *height),
                        self.color,
                    );
                }
            }
        }
        vec![frame.into_geometry()]
    }
}
