//! P0 验证 #3b：Typst Frame 的字形喂给 vello 绘制。
//!
//! 问题：Typst 排版产出的字形能不能直接用 vello 画出来？
//! 通过标准：字形出现在正确位置，数学结构（分数线、根号、矩阵）形状正确。
//!
//! 这验证的是 PLAN §2 管线的最后一段：Frame → vello scene → 屏幕。
//! 前面 span.rs 已验证 Frame → NodeId 的反查，两段合起来就是完整闭环。
//!
//! 注意坐标系：Typst 的 y_offset/y_advance 是 Y-up，vello 是 Y-down，要翻转。

mod world;

use typst::layout::{Frame, FrameItem, Point as TypstPoint};
use typst::visualize::{Geometry, Paint};
use typst_layout::PagedDocument;
// `Shape` trait 必须在作用域内才能用 `into_path`
use vello::kurbo::{Affine, BezPath, Line, Point, Rect, Shape as _, Stroke};
use vello::peniko::color::{AlphaColor, Srgb};
use vello::peniko::{Blob, Fill, FontData};
use vello::util::RenderContext;
use vello::{AaConfig, Glyph, RenderParams, Renderer, RendererOptions, Scene, wgpu};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};
use world::SpikeWorld;

/// 验证用文档。混排中英文 + 各类数学结构，覆盖 Text / Shape / Group 三种 FrameItem。
const DOC: &str = r#"= 全微分

设 $z = f(x, y) = x^y$，则有 mixed English text 混排。

$ frac(partial z, partial x) dif x + frac(partial z, partial y) dif y $

$ sqrt(x^2 + y^2) <= abs(x) + abs(y) $

$ mat(1, 2; 3, 4) times vec(alpha, beta) $

$ sum_(k=1)^n frac(1, k^2) = pi^2 / 6 $
"#;

/// Typst 用 pt，屏幕用 px。放大 1.6 倍便于肉眼检查字形位置。
const SCALE: f64 = 1.6;

#[derive(Default)]
struct Stats {
    glyphs: usize,
    shapes: usize,
    text_runs: usize,
    groups: usize,
    /// 嵌套最深层数，验证 Group 递归和变换叠加
    max_depth: usize,
}

struct App {
    ctx: RenderContext,
    /// 用 Option 是因为 winit 要求窗口在 resumed 里创建
    state: Option<State>,
    scene: Scene,
    doc: PagedDocument,
    world: SpikeWorld,
}

struct State {
    window: std::sync::Arc<Window>,
    surface: vello::util::RenderSurface<'static>,
    renderer: Renderer,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let attrs = Window::default_attributes()
            .with_title("Scholium P0 #3b — Typst Frame → vello")
            .with_inner_size(PhysicalSize::new(900, 1000));
        let window = std::sync::Arc::new(el.create_window(attrs).expect("创建窗口失败"));

        let size = window.inner_size();
        let surface = pollster::block_on(self.ctx.create_surface(
            window.clone(),
            size.width.max(1),
            size.height.max(1),
            wgpu::PresentMode::AutoVsync,
        ))
        .expect("创建 vello surface 失败");

        let renderer = Renderer::new(
            &self.ctx.devices[surface.dev_id].device,
            RendererOptions {
                use_cpu: false,
                antialiasing_support: vello::AaSupport::area_only(),
                num_init_threads: std::num::NonZeroUsize::new(1),
                ..Default::default()
            },
        )
        .expect("创建 vello renderer 失败");

        println!("窗口与 vello 就绪。按 Esc 退出。");
        self.state = Some(State {
            window,
            surface,
            renderer,
        });
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
                if let Some(st) = &mut self.state {
                    self.ctx
                        .resize_surface(&mut st.surface, size.width.max(1), size.height.max(1));
                    st.window.request_redraw();
                }
            }

            WindowEvent::RedrawRequested => self.redraw(),

            _ => {}
        }
    }
}

impl App {
    fn redraw(&mut self) {
        let Some(st) = &mut self.state else { return };

        self.scene.reset();
        let mut stats = Stats::default();

        // 纸张底色
        let (w, h) = (st.surface.config.width, st.surface.config.height);
        self.scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            AlphaColor::<Srgb>::new([1.0, 1.0, 1.0, 1.0]),
            None,
            &Rect::new(0.0, 0.0, w as f64, h as f64),
        );

        // 只画第一页，够验证了
        if let Some(page) = self.doc.pages().first() {
            let base = Affine::scale(SCALE);
            draw_frame(&mut self.scene, &page.frame, base, &mut stats, 0);
        }

        let dev = &self.ctx.devices[st.surface.dev_id];
        // wgpu 29 把 get_current_texture 从 Result 改成了枚举，
        // Timeout / Occluded / Outdated 都该跳过当前帧而不是 panic
        let texture = match st.surface.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            other => {
                println!("跳过一帧: {other:?}");
                return;
            }
        };

        st.renderer
            .render_to_texture(
                &dev.device,
                &dev.queue,
                &self.scene,
                &st.surface.target_view,
                &RenderParams {
                    base_color: AlphaColor::<Srgb>::new([1.0, 1.0, 1.0, 1.0]),
                    width: w,
                    height: h,
                    antialiasing_method: AaConfig::Area,
                },
            )
            .expect("vello 渲染失败");

        // vello 画到离屏纹理，再 blit 到 surface
        let mut encoder = dev
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("blit"),
            });
        st.surface.blitter.copy(
            &dev.device,
            &mut encoder,
            &st.surface.target_view,
            &texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default()),
        );
        dev.queue.submit([encoder.finish()]);
        texture.present();

        println!(
            "绘制完成: {} 字形 / {} shape / {} text run / {} group / 最深 {} 层",
            stats.glyphs, stats.shapes, stats.text_runs, stats.groups, stats.max_depth
        );
    }
}

/// 递归把 Frame 画进 vello scene。
///
/// `transform` 是从 frame 局部坐标到屏幕像素的累积变换（含 SCALE 和各层 group 偏移）。
fn draw_frame(scene: &mut Scene, frame: &Frame, transform: Affine, stats: &mut Stats, depth: usize) {
    stats.max_depth = stats.max_depth.max(depth);

    for (pos, item) in frame.items() {
        // frame 局部坐标 → 当前层坐标
        let at = transform * Affine::translate((pos.x.to_pt(), pos.y.to_pt()));

        match item {
            FrameItem::Group(group) => {
                stats.groups += 1;
                draw_frame(scene, &group.frame, at, stats, depth + 1);
            }

            FrameItem::Text(text) => {
                stats.text_runs += 1;

                let font_size = text.size.to_pt();
                let font = FontData::new(
                    Blob::new(std::sync::Arc::new(text.font.data().to_vec())),
                    text.font.index(),
                );

                // Typst 给的是每字形的 offset + advance，需要自己累加成绝对位置。
                // y 要取反：Typst 的 y_offset/y_advance 是 Y-up，vello 是 Y-down。
                let mut x = 0.0_f64;
                let glyphs: Vec<Glyph> = text
                    .glyphs
                    .iter()
                    .map(|g| {
                        let gx = x + g.x_offset.get() * font_size;
                        let gy = -g.y_offset.get() * font_size;
                        x += g.x_advance.get() * font_size;
                        stats.glyphs += 1;
                        Glyph {
                            id: u32::from(g.id),
                            x: gx as f32,
                            y: gy as f32,
                        }
                    })
                    .collect();

                scene
                    .draw_glyphs(&font)
                    .font_size(font_size as f32)
                    .transform(at)
                    .brush(paint_color(&text.fill))
                    .draw(Fill::NonZero, glyphs.into_iter());
            }

            FrameItem::Shape(shape, _span) => {
                stats.shapes += 1;

                let path = geometry_to_path(&shape.geometry);

                if let Some(fill) = &shape.fill {
                    scene.fill(Fill::NonZero, at, paint_color(fill), None, &path);
                }
                if let Some(stroke) = &shape.stroke {
                    scene.stroke(
                        &Stroke::new(stroke.thickness.to_pt()),
                        at,
                        paint_color(&stroke.paint),
                        None,
                        &path,
                    );
                }
            }

            // 图片和链接不在本次验证范围
            FrameItem::Image(..) | FrameItem::Link(..) | FrameItem::Tag(_) => {}
        }
    }
}

/// Typst 几何 → kurbo 路径。`CurveItem` 和 kurbo 的路径模型一一对应。
fn geometry_to_path(geom: &Geometry) -> BezPath {
    match geom {
        Geometry::Line(to) => Line::new(Point::ZERO, pt(*to)).into_path(0.0),

        Geometry::Rect(size) => {
            Rect::new(0.0, 0.0, size.x.to_pt(), size.y.to_pt()).into_path(0.0)
        }

        Geometry::Curve(curve) => {
            let mut path = BezPath::new();
            for item in &curve.0 {
                use typst::visualize::CurveItem;
                match item {
                    CurveItem::Move(p) => path.move_to(pt(*p)),
                    CurveItem::Line(p) => path.line_to(pt(*p)),
                    CurveItem::Cubic(c1, c2, p) => path.curve_to(pt(*c1), pt(*c2), pt(*p)),
                    CurveItem::Close => path.close_path(),
                }
            }
            path
        }
    }
}

fn pt(p: TypstPoint) -> Point {
    Point::new(p.x.to_pt(), p.y.to_pt())
}

/// 取 Paint 的颜色。spike 只处理纯色——渐变和图案不在验证范围。
fn paint_color(paint: &Paint) -> AlphaColor<Srgb> {
    match paint {
        Paint::Solid(color) => {
            // to_vec4 挂在 Color 上，不在 to_rgb() 的返回值上
            let [r, g, b, a] = color.to_vec4();
            AlphaColor::new([r, g, b, a])
        }
        // 渐变/图案回落成黑色，本次不验证
        _ => AlphaColor::new([0.0, 0.0, 0.0, 1.0]),
    }
}

fn main() {
    let world = SpikeWorld::new(DOC);
    let result = typst::compile::<PagedDocument>(&world);

    for w in result.warnings.iter() {
        println!("warn: {}", w.message);
    }

    let doc = match result.output {
        Ok(d) => d,
        Err(errors) => {
            for e in errors.iter() {
                eprintln!("error: {}", e.message);
            }
            std::process::exit(1);
        }
    };

    println!("编译完成，{} 页", doc.pages().len());

    let el = EventLoop::new().expect("创建事件循环失败");
    el.set_control_flow(ControlFlow::Wait);

    let mut app = App {
        ctx: RenderContext::new(),
        state: None,
        scene: Scene::new(),
        doc,
        world,
    };
    el.run_app(&mut app).expect("事件循环异常退出");
}
