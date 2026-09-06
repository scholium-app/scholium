use scholium_doc::{Cursor, Document, EditOp, History, NodeKind, Origin, Selection, Transaction};
use scholium_layout::{FontConfig, ScholiumWorld, compile};
use scholium_render::frame_scene::{self, CursorScreenPos, FontCache};
use scholium_render::{ScholiumRenderer, fill_background};
use vello::Scene;
use vello::peniko::color::{AlphaColor, Srgb};
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu;
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, Modifiers, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

mod input;
mod viewport;

use viewport::{PageView, fit_page};

/// The main application state.
pub struct App {
    window: Option<std::sync::Arc<Window>>,
    surface: Option<RenderSurface<'static>>,
    renderer: Option<ScholiumRenderer>,
    scene: Scene,

    doc: Document,
    world: ScholiumWorld,
    compile_output: Option<scholium_layout::CompileOutput>,
    cursor: Cursor,
    cursor_screen: Option<CursorScreenPos>,
    font_cache: FontCache,

    history: History,
    cached_mods: Modifiers,
    ime_active: bool,
    pending_text: String,
    pending_cursor: Option<(usize, usize)>,
    mouse_position: Option<PhysicalPosition<f64>>,
    cursor_past: Vec<Cursor>,
    cursor_future: Vec<Cursor>,
}

impl App {
    /// Create a new App with empty document and default font config.
    pub fn new() -> Self {
        let font_config = FontConfig::default_cjk();
        let world = ScholiumWorld::new(&font_config);
        let mut doc = Document::new();
        let paragraph = doc.append_child(doc.root(), NodeKind::Paragraph, None);
        doc.append_child(paragraph, NodeKind::Text, Some(String::new()));
        Self {
            window: None,
            surface: None,
            renderer: None,
            scene: Scene::new(),
            doc,
            world,
            compile_output: None,
            cursor: Cursor::new(vec![0, 0], 0),
            cursor_screen: None,
            font_cache: FontCache::new(),
            history: History::new(),
            cached_mods: Modifiers::from(ModifiersState::empty()),
            ime_active: false,
            pending_text: String::new(),
            pending_cursor: None,
            mouse_position: None,
            cursor_past: Vec::new(),
            cursor_future: Vec::new(),
        }
    }

    /// Apply a transaction to the document and recompile.
    fn edit_and_recompile(&mut self, tx: Transaction) {
        let before = self.doc.clone();
        let before_cursor = self.cursor.clone();
        if self.doc.apply_transaction(&tx).is_err() {
            return;
        }

        self.update_cursor_after_ops(&tx.ops);

        match compile(&mut self.world, &self.doc) {
            Ok(output) => {
                self.history.commit(before);
                self.cursor_past.push(before_cursor);
                if self.cursor_past.len() > History::DEFAULT_LIMIT {
                    self.cursor_past.remove(0);
                }
                self.cursor_future.clear();
                self.compile_output = Some(output);
                self.update_cursor_screen();
            }
            Err(errors) => {
                eprintln!("compile errors: {errors:?}");
                self.doc = before;
            }
        }

        if let Some(ref w) = self.window {
            w.request_redraw();
        }
    }

    /// Update the cursor position after applying a batch of edit operations.
    fn update_cursor_after_ops(&mut self, ops: &[EditOp]) {
        for op in ops {
            match op {
                EditOp::InsertText { at, text } => {
                    self.cursor.path = at.path.clone();
                    self.cursor.offset = at.offset + text.len();
                }
                EditOp::DeleteRange { range } => {
                    self.cursor.path = range.anchor.path.clone();
                    self.cursor.offset = range.anchor.offset.min(range.focus.offset);
                }
                _ => {}
            }
        }
    }

    /// Insert text at the current cursor position and recompile.
    fn insert_text(&mut self, text: &str) {
        let at = self.cursor.clone();
        let tx = Transaction::new(
            vec![EditOp::InsertText {
                at,
                text: text.to_string(),
            }],
            Origin::User,
        );
        self.edit_and_recompile(tx);
    }

    /// Delete the character before the cursor and recompile.
    fn delete_backward(&mut self) {
        if self.cursor.path.is_empty() || self.cursor.offset == 0 {
            return;
        }
        let Some(node_id) = self.doc.resolve_path(&self.cursor.path) else {
            return;
        };
        let node = self.doc.node(node_id);
        let Some(ref text) = node.text else { return };

        if let Some((prev_start, _)) = text[..self.cursor.offset].char_indices().next_back() {
            let range = Selection::new(
                Cursor::new(self.cursor.path.clone(), prev_start),
                Cursor::new(self.cursor.path.clone(), self.cursor.offset),
            );
            let tx = Transaction::new(vec![EditOp::DeleteRange { range }], Origin::User);
            self.edit_and_recompile(tx);
        }
    }

    /// Delete the character after the cursor.
    fn delete_forward(&mut self) {
        let Some(node_id) = self.doc.resolve_path(&self.cursor.path) else {
            return;
        };
        let Some(text) = self.doc.node(node_id).text.as_ref() else {
            return;
        };
        if self.cursor.offset >= text.len() {
            return;
        }
        let Some(ch) = text[self.cursor.offset..].chars().next() else {
            return;
        };
        let range = Selection::new(
            self.cursor.clone(),
            Cursor::new(self.cursor.path.clone(), self.cursor.offset + ch.len_utf8()),
        );
        self.edit_and_recompile(Transaction::new(
            vec![EditOp::DeleteRange { range }],
            Origin::User,
        ));
    }

    fn move_horizontal(&mut self, forward: bool) {
        let Some(node_id) = self.doc.resolve_path(&self.cursor.path) else {
            return;
        };
        let Some(text) = self.doc.node(node_id).text.as_ref() else {
            return;
        };
        self.cursor.offset = if forward {
            text[self.cursor.offset..]
                .chars()
                .next()
                .map_or(self.cursor.offset, |ch| self.cursor.offset + ch.len_utf8())
        } else {
            text[..self.cursor.offset]
                .char_indices()
                .next_back()
                .map_or(0, |(offset, _)| offset)
        };
        self.update_cursor_screen();
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    fn move_to_text_edge(&mut self, end: bool) {
        let Some(node_id) = self.doc.resolve_path(&self.cursor.path) else {
            return;
        };
        let Some(text) = self.doc.node(node_id).text.as_ref() else {
            return;
        };
        self.cursor.offset = if end { text.len() } else { 0 };
        self.update_cursor_screen();
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    /// Undo the last transaction.
    fn undo(&mut self) {
        if !self.history.can_undo() {
            return;
        }
        if let Some(prev) = self.history.undo(self.doc.clone()) {
            self.cursor_future.push(self.cursor.clone());
            self.doc = prev;
            self.cursor = self.cursor_past.pop().unwrap_or_default();
            self.recompile();
        }
    }

    /// Redo the last undone transaction.
    fn redo(&mut self) {
        if !self.history.can_redo() {
            return;
        }
        if let Some(next) = self.history.redo(self.doc.clone()) {
            self.cursor_past.push(self.cursor.clone());
            self.doc = next;
            self.cursor = self.cursor_future.pop().unwrap_or_default();
            self.recompile();
        }
    }

    /// Recompile and request a redraw.
    fn recompile(&mut self) {
        let visible = self.visible_document();
        match compile(&mut self.world, &visible) {
            Ok(output) => {
                self.compile_output = Some(output);
                self.update_cursor_screen();
            }
            Err(e) => eprintln!("recompile failed: {e:?}"),
        }
        if let Some(ref w) = self.window {
            w.request_redraw();
        }
    }

    /// Recompute the cursor screen position from the compiled frame.
    fn update_cursor_screen(&mut self) {
        let Some(ref output) = self.compile_output else {
            self.cursor_screen = None;
            return;
        };
        let page = match output.doc.pages().first() {
            Some(p) => p,
            None => {
                self.cursor_screen = None;
                return;
            }
        };
        let cursor = self.visible_cursor();
        self.cursor_screen = output
            .interaction
            .caret_for_cursor(&self.visible_document(), 0, &cursor)
            .map(|pos| CursorScreenPos {
                x: pos.x,
                y: pos.y,
                height: pos.height,
            })
            .or_else(|| frame_scene::find_last_glyph_position(&page.frame))
            .or(Some(CursorScreenPos {
                x: 72.0,
                y: 72.0,
                height: 14.0,
            }));
    }

    fn visible_cursor(&self) -> Cursor {
        if self.pending_text.is_empty() {
            return self.cursor.clone();
        }
        let mut cursor = self.cursor.clone();
        cursor.offset += self
            .pending_cursor
            .map_or(self.pending_text.len(), |(_, end)| end);
        cursor
    }

    fn visible_document(&self) -> Document {
        if self.pending_text.is_empty() {
            return self.doc.clone();
        }
        let mut visible = self.doc.clone();
        let tx = Transaction::new(
            vec![EditOp::InsertText {
                at: self.cursor.clone(),
                text: self.pending_text.clone(),
            }],
            Origin::User,
        );
        let _ = visible.apply_transaction(&tx);
        visible
    }

    fn page_view(&self, width: u32, height: u32) -> Option<PageView> {
        let output = self.compile_output.as_ref()?;
        let page = output.doc.pages().first()?;
        Some(fit_page(
            width,
            height,
            page.frame.width().to_pt(),
            page.frame.height().to_pt(),
        ))
    }

    fn click_at(&mut self, position: PhysicalPosition<f64>) {
        if !self.pending_text.is_empty() {
            return;
        }
        let Some(surface) = self.surface.as_ref() else {
            return;
        };
        let Some(view) = self.page_view(surface.config.width, surface.config.height) else {
            return;
        };
        let Some(output) = self.compile_output.as_ref() else {
            return;
        };
        let visible = self.visible_document();
        let Some(cursor) = output.interaction.hit_test(
            &visible,
            0,
            (position.x - view.offset_x) / view.scale,
            (position.y - view.offset_y) / view.scale,
        ) else {
            return;
        };
        self.cursor = cursor;
        self.update_cursor_screen();
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    fn update_preedit(&mut self, text: String, cursor: Option<(usize, usize)>) {
        self.pending_text = text;
        self.pending_cursor = cursor;
        self.recompile();
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = Window::default_attributes()
            .with_title("Scholium")
            .with_inner_size(PhysicalSize::new(900, 700));
        let window = std::sync::Arc::new(el.create_window(attrs).expect("window creation failed"));

        window.set_ime_allowed(true);

        let size = window.inner_size();
        let mut ctx = RenderContext::new();
        let mut surface = pollster::block_on(ctx.create_surface(
            window.clone(),
            size.width.max(1),
            size.height.max(1),
            wgpu::PresentMode::AutoVsync,
        ))
        .expect("vello surface creation failed");

        let renderer = ScholiumRenderer::from_context(ctx, &mut surface);

        self.window = Some(window);
        self.surface = Some(surface);
        self.renderer = Some(renderer);

        // Initial compile of empty document
        match compile(&mut self.world, &self.doc) {
            Ok(output) => {
                self.compile_output = Some(output);
                self.update_cursor_screen();
            }
            Err(e) => eprintln!("initial compile failed: {e:?}"),
        }

        if let Some(ref w) = self.window {
            w.request_redraw();
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed
                    && event.logical_key == Key::Named(NamedKey::Escape) =>
            {
                el.exit();
            }
            WindowEvent::ModifiersChanged(mods) => {
                self.cached_mods = mods;
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_position = Some(position);
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: winit::event::MouseButton::Left,
                ..
            } => {
                if let Some(position) = self.mouse_position {
                    self.click_at(position);
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                self.handle_keyboard(&event);
            }
            WindowEvent::Ime(ime) => self.handle_ime(ime),
            WindowEvent::Resized(size) => {
                if let (Some(renderer), Some(surface)) =
                    (self.renderer.as_mut(), self.surface.as_mut())
                {
                    renderer.resize_surface(surface, size.width, size.height);
                }
                if let Some(ref w) = self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }
}

impl App {
    /// Rebuild the scene and submit it to the renderer.
    fn redraw(&mut self) {
        let preedit_start = if self.pending_text.is_empty() {
            None
        } else {
            let visible = self.visible_document();
            self.compile_output
                .as_ref()
                .and_then(|output| {
                    output
                        .interaction
                        .caret_for_cursor(&visible, 0, &self.cursor)
                })
                .map(|pos| CursorScreenPos {
                    x: pos.x,
                    y: pos.y,
                    height: pos.height,
                })
        };
        let Some(ref mut renderer) = self.renderer else {
            return;
        };
        let Some(ref mut surface) = self.surface else {
            return;
        };

        let (w, h) = (surface.config.width, surface.config.height);
        self.scene.reset();
        fill_background(
            &mut self.scene,
            AlphaColor::<Srgb>::new([0.95, 0.95, 0.93, 1.0]),
            w as f64,
            h as f64,
        );

        if let Some(ref output) = self.compile_output
            && let Some(page_frame) = output.doc.pages().first().map(|p| &p.frame)
        {
            let view = fit_page(
                w,
                h,
                page_frame.width().to_pt(),
                page_frame.height().to_pt(),
            );

            frame_scene::add_page(
                &mut self.scene,
                page_frame,
                &mut self.font_cache,
                view.offset_x,
                view.offset_y,
                view.scale,
            );

            if let Some(cursor_pos) = self.cursor_screen {
                let sx = cursor_pos.x * view.scale + view.offset_x;
                let sy = cursor_pos.y * view.scale + view.offset_y;
                let sh = cursor_pos.height * view.scale;
                let screen = CursorScreenPos {
                    x: sx,
                    y: sy,
                    height: sh,
                };
                frame_scene::draw_cursor(&mut self.scene, screen);

                if self.ime_active
                    && let Some(ref window) = self.window
                {
                    window.set_ime_cursor_area(
                        PhysicalPosition::new(sx as i32, (sy + sh) as i32),
                        PhysicalSize::new(2, sh.max(1.0) as u32),
                    );
                }

                if let Some(start) = preedit_start {
                    let start = CursorScreenPos {
                        x: start.x * view.scale + view.offset_x,
                        y: start.y * view.scale + view.offset_y,
                        height: start.height * view.scale,
                    };
                    frame_scene::draw_preedit_underline(&mut self.scene, start, screen);
                }
            }
        }

        renderer.render(surface, &self.scene);
    }
}
