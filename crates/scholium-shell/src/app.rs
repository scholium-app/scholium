use scholium_render::{ScholiumRenderer, fill_background};
use vello::Scene;
use vello::peniko::color::{AlphaColor, Srgb};
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

/// The main application state.
pub struct App {
    window: Option<std::sync::Arc<Window>>,
    surface: Option<RenderSurface<'static>>,
    renderer: Option<ScholiumRenderer>,
    scene: Scene,
}

impl App {
    /// Create a new `App` with no window, surface, or renderer.
    pub fn new() -> Self {
        Self {
            window: None,
            surface: None,
            renderer: None,
            scene: Scene::new(),
        }
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

        renderer.render(surface, &self.scene);
    }
}
