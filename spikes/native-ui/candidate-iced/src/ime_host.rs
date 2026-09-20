//! 让自绘区域自己请求输入法的透传包装控件。
//!
//! iced 只在控件显式调用 `Shell::request_input_method` 时才开启输入法
//! （`set_ime_allowed(true)`）。因此**自绘的结构编辑器必须自己请求输入法**，否则窗口收不到
//! `Preedit` / `Commit`。这是候选验证中实测确认的行为，不是推测。
//!
//! 本控件只请求输入法，不改变布局与绘制：`tree` 直接透传给内容控件。

use iced::advanced::input_method::Purpose;
use iced::advanced::widget::{Operation, Tree, Widget, tree};
use iced::advanced::{Clipboard, InputMethod, Layout, Shell, layout, mouse, renderer};
use iced::{Element, Event, Length, Rectangle, Size, Theme};

/// 透传包装：在 `enabled` 时代表内容请求输入法。
pub struct ImeHost<'a, Message> {
    content: Element<'a, Message>,
    enabled: bool,
    cursor: Rectangle,
}

impl<'a, Message> ImeHost<'a, Message> {
    /// 包装内容。
    ///
    /// `cursor` 是输入法候选窗应避开的矩形（窗口坐标）。真编辑器会传入实际光标几何；
    /// 验证工程里用的是近似值。
    pub fn new(content: impl Into<Element<'a, Message>>, enabled: bool, cursor: Rectangle) -> Self {
        Self {
            content: content.into(),
            enabled,
            cursor,
        }
    }
}

impl<Message> Widget<Message, Theme, iced::Renderer> for ImeHost<'_, Message> {
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn tag(&self) -> tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<Tree> {
        self.content.as_widget().children()
    }

    fn diff(&self, tree: &mut Tree) {
        self.content.as_widget().diff(tree);
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content.as_widget_mut().layout(tree, renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(tree, layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
        let method: InputMethod<String> = if self.enabled {
            InputMethod::Enabled {
                cursor: self.cursor,
                purpose: Purpose::Normal,
                preedit: None,
            }
        } else {
            InputMethod::Disabled
        };
        shell.request_input_method(&method);
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }
}

impl<'a, Message: 'a> From<ImeHost<'a, Message>> for Element<'a, Message> {
    fn from(host: ImeHost<'a, Message>) -> Self {
        Self::new(host)
    }
}
