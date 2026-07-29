//! vello rendering infrastructure for Scholium.
//!
//! Manages the wgpu device, vello renderer, and surface.

use vello::peniko::Fill;
use vello::peniko::color::{AlphaColor, Srgb};
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu;
use vello::{AaConfig, RenderParams, Renderer, RendererOptions, Scene};

/// The Scholium renderer — owns a vello `Renderer` and `RenderContext`.
pub struct ScholiumRenderer {
    ctx: RenderContext,
    renderer: Renderer,
    size: (u32, u32),
}

impl ScholiumRenderer {
    /// Create a new `ScholiumRenderer` from an existing `RenderContext` and surface.
    ///
    /// # Panics
    /// Panics if the vello renderer cannot be created.
    pub fn from_context(ctx: RenderContext, surface: &mut RenderSurface<'static>) -> Self {
        let dev_id = surface.dev_id;
        let renderer = Renderer::new(
            &ctx.devices[dev_id].device,
            RendererOptions {
                use_cpu: false,
                antialiasing_support: vello::AaSupport::area_only(),
                num_init_threads: std::num::NonZeroUsize::new(1),
                ..Default::default()
            },
        )
        .expect("vello renderer creation failed");

        let size = (surface.config.width, surface.config.height);
        Self {
            ctx,
            renderer,
            size,
        }
    }

    /// Draw a scene to the given surface.
    ///
    /// Handles wgpu 29's `CurrentSurfaceTexture` enum by skipping
    /// `Timeout`/`Occluded`/`Outdated` frames.
    ///
    /// # Panics
    /// Panics if the vello render fails.
    pub fn render(&mut self, surface: &mut RenderSurface<'static>, scene: &Scene) {
        let (w, h) = self.size;
        let dev = &self.ctx.devices[surface.dev_id];

        let texture = match surface.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            _other => return,
        };

        self.renderer
            .render_to_texture(
                &dev.device,
                &dev.queue,
                scene,
                &surface.target_view,
                &RenderParams {
                    base_color: AlphaColor::<Srgb>::new([1.0, 1.0, 1.0, 1.0]),
                    width: w,
                    height: h,
                    antialiasing_method: AaConfig::Area,
                },
            )
            .expect("vello render failed");

        let mut encoder = dev
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("scholium-blit"),
            });
        surface.blitter.copy(
            &dev.device,
            &mut encoder,
            &surface.target_view,
            &texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default()),
        );
        dev.queue.submit([encoder.finish()]);
        texture.present();
    }

    /// Resize the surface and update internal size.
    pub fn resize_surface(
        &mut self,
        surface: &mut RenderSurface<'static>,
        width: u32,
        height: u32,
    ) {
        self.ctx
            .resize_surface(surface, width.max(1), height.max(1));
        self.size = (width.max(1), height.max(1));
    }
}

/// Fill the given scene with a solid background color.
pub fn fill_background(scene: &mut Scene, color: AlphaColor<Srgb>, width: f64, height: f64) {
    use vello::kurbo::{Affine, Rect, Shape as _};
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        color,
        None,
        &Rect::new(0.0, 0.0, width, height).into_path(0.0),
    );
}
