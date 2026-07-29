use scholium_doc::{Cursor, Document, EditOp, Origin, Selection, Transaction};
use scholium_layout::{FontConfig, ScholiumWorld, compile};
use scholium_render::frame_scene::{self, CursorScreenPos, FontCache};
use scholium_render::{ScholiumRenderer, fill_background};
use vello::Scene;
use vello::peniko::color::{AlphaColor, Srgb};
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, Ime, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

const MARGIN: f64 = 48.0;

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

    ime_active: bool,
    pending_text: String,
}

impl App {
    /// Create a new App with empty document and default font config.
    pub fn new() -> Self {
        let font_config = FontConfig::default_cjk();
        let world = ScholiumWorld::new(&font_config);
        let doc = Document::new();
        Self {
            window: None,
            surface: None,
            renderer: None,
            scene: Scene::new(),
            doc,
            world,
            compile_output: None,
            cursor: Cursor::start(),
            cursor_screen: None,
            font_cache: FontCache::new(),
            ime_active: false,
            pending_text: String::new(),
        }
    }

    /// Apply a transaction to the document and recompile.
    fn edit_and_recompile(&mut self, tx: Transaction) {
        let before = self.doc.clone();
        if self.doc.apply_transaction(&tx).is_err() {
            return;
        }

        self.update_cursor_after_ops(&tx.ops);

        match compile(&mut self.world, &self.doc) {
            Ok(output) => {
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
        self.cursor_screen = frame_scene::find_last_glyph_position(&page.frame);
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
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                match &event.logical_key {
                    Key::Named(NamedKey::Backspace) | Key::Named(NamedKey::Delete) => {
                        self.delete_backward();
                    }
                    Key::Named(NamedKey::Enter) => {
                        self.insert_text("\n");
                    }
                    Key::Named(NamedKey::Space) => {
                        self.insert_text(" ");
                    }
                    Key::Named(NamedKey::Tab) => {}
                    Key::Character(ch) if !ch.is_empty() && !self.ime_active => {
                        self.insert_text(ch);
                    }
                    _ => {}
                }
            }
            WindowEvent::Ime(ime) => match ime {
                Ime::Commit(text) => {
                    if !text.is_empty() {
                        self.insert_text(&text);
                    }
                }
                Ime::Preedit(text, _) => {
                    if text.is_empty() {
                        self.pending_text.clear();
                    } else {
                        self.pending_text = text;
                    }
                }
                Ime::Enabled => {
                    self.ime_active = true;
                }
                Ime::Disabled => {
                    self.ime_active = false;
                    self.pending_text.clear();
                }
            },
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
            frame_scene::add_page(
                &mut self.scene,
                page_frame,
                &mut self.font_cache,
                MARGIN,
                MARGIN,
            );
        }

        if let Some(cursor_pos) = self.cursor_screen {
            let pos = CursorScreenPos {
                x: cursor_pos.x + MARGIN,
                y: cursor_pos.y + MARGIN,
                height: cursor_pos.height,
            };
            frame_scene::draw_cursor(&mut self.scene, pos);
        }

        renderer.render(surface, &self.scene);
    }
}
