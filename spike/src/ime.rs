//! P0 验证 #3a：winit 的 IME 事件通路。
//!
//! 问题：中文输入法的预编辑串能不能拿到？候选框能不能定位？
//! 通过标准：Linux + Windows 上能收到 Preedit 和 Commit，
//!           `set_ime_cursor_area` 能把候选框固定到指定位置。
//!
//! 不通过的退路：重新考虑 Qt 6（IME 成熟度是 Qt 最强项）。
//!
//! 这个 spike 只把事件打到 stdout，不做渲染。渲染栈（vello + parley）
//! 是独立风险，混在一起失败了分不清是谁的问题。
//!
//! 需要人工操作：运行后切到中文输入法，敲拼音，观察输出。

use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, Ime, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

/// 模拟光标位置。真实实现里这个值来自 layout 反查（PLAN §5.2）。
/// 候选框应该跟着它走。
const CARET_X: i32 = 120;
const CARET_Y: i32 = 200;
const CARET_H: i32 = 24;

#[derive(Default)]
struct App {
    /// `Rc` 是 softbuffer 的要求：surface 要持有 window
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    size: PhysicalSize<u32>,
    /// 已提交的文本，模拟文档内容
    committed: String,
    /// 当前预编辑串，模拟临时覆盖节点（PLAN §5.4）
    preedit: String,
    stats: Stats,
    started: Option<Instant>,
}

#[derive(Default)]
struct Stats {
    enabled: usize,
    preedit_events: usize,
    commit_events: usize,
    disabled: usize,
    /// 非空 preedit 带光标范围——决定能否画下划线分段
    preedit_with_cursor: usize,
    /// 非空 preedit 却不带光标范围——这才是真需要兜底的情况
    nonempty_without_cursor: usize,
    /// 空 preedit，即"清除预编辑"信号。不带 cursor 是正常的，不算缺陷。
    empty_preedit: usize,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        let attrs = Window::default_attributes()
            .with_title("Scholium P0 — IME 验证")
            .with_inner_size(PhysicalSize::new(720, 400));

        let window = Rc::new(el.create_window(attrs).expect("创建窗口失败"));

        // Wayland 下窗口在附加第一个缓冲区之前不会被合成器显示。
        // 所以必须真的画东西，光创建窗口是看不见的（X11 下会显示空窗口）。
        let context = softbuffer::Context::new(window.clone()).expect("创建 softbuffer 上下文失败");
        let surface =
            softbuffer::Surface::new(&context, window.clone()).expect("创建 softbuffer surface 失败");

        self.size = window.inner_size();

        // 关键：必须显式开启，否则收不到任何 Ime 事件
        window.set_ime_allowed(true);

        println!("窗口已创建。");
        println!("  set_ime_allowed(true) 已调用");
        println!("  set_ime_cursor_area 延后到 Ime::Enabled 之后调用（见下）");
        println!();
        println!("请切到中文输入法，敲拼音。观察：");
        println!("  1. 是否收到 Ime::Enabled");
        println!("  2. 敲拼音时是否收到 Ime::Preedit，内容是否是拼音串");
        println!("  3. 候选框是否出现在窗口内 ({CARET_X}, {CARET_Y}) 附近，而非屏幕角落");
        println!("  4. 选字后是否收到 Ime::Commit");
        println!();
        println!("按 Esc 退出并打印统计。");
        println!("────────────────────────────────────────");

        window.request_redraw();
        self.window = Some(window);
        self.surface = Some(surface);
        self.started = Some(Instant::now());
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let t = self
            .started
            .map(|s| s.elapsed().as_millis())
            .unwrap_or_default();

        match event {
            WindowEvent::CloseRequested => {
                self.report();
                el.exit();
            }

            WindowEvent::Resized(size) => {
                self.size = size;
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }

            WindowEvent::RedrawRequested => self.redraw(),

            WindowEvent::Ime(ime) => match ime {
                Ime::Enabled => {
                    self.stats.enabled += 1;
                    println!("[{t:>6}ms] Ime::Enabled");
                    // 必须在这里（而非 resumed）调：Wayland 下 winit 的
                    // text_inputs 集合在窗口拿到焦点前是空的，
                    // set_ime_cursor_area 会静默变成空操作。
                    self.push_cursor_area();
                }

                Ime::Preedit(text, cursor) => {
                    self.stats.preedit_events += 1;
                    // 空串是清除信号，没有光标范围是正常的。
                    // 只有非空却缺 cursor 才需要兜底。
                    match (text.is_empty(), cursor.is_some()) {
                        (true, _) => self.stats.empty_preedit += 1,
                        (false, true) => self.stats.preedit_with_cursor += 1,
                        (false, false) => self.stats.nonempty_without_cursor += 1,
                    }
                    println!(
                        "[{t:>6}ms] Ime::Preedit  {text:?}  cursor={cursor:?}  \
                         (字节 {} / 字符 {})",
                        text.len(),
                        text.chars().count()
                    );
                    self.preedit = text;
                    // 真实编辑器里光标会随输入移动，每次都要重推。
                    // 这里位置固定，重推是为了验证 Enabled 之后调用确实生效。
                    self.push_cursor_area();
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }

                Ime::Commit(text) => {
                    self.stats.commit_events += 1;
                    println!("[{t:>6}ms] Ime::Commit   {text:?}");
                    self.committed.push_str(&text);
                    self.preedit.clear();
                    println!("           已提交文本: {:?}", self.committed);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }

                Ime::Disabled => {
                    self.stats.disabled += 1;
                    println!("[{t:>6}ms] Ime::Disabled");
                }
            },

            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }

                if event.logical_key == Key::Named(NamedKey::Escape) {
                    self.report();
                    el.exit();
                    return;
                }

                // 非 IME 的直接按键，验证两条通路不互相吞事件
                if let Some(txt) = &event.text {
                    println!("[{t:>6}ms] Key           {txt:?}");
                    self.committed.push_str(txt);
                }
            }

            _ => {}
        }
    }
}

impl App {
    /// 把候选框位置推给输入法。
    ///
    /// 必须在 `Ime::Enabled` 之后调用。winit 的 Wayland 后端里
    /// `set_ime_cursor_area` 遍历 `text_inputs` 集合，而该集合在窗口
    /// 拿到焦点前是空的——早调用会静默失败，候选框落到窗口左下角。
    fn push_cursor_area(&self) {
        let Some(w) = &self.window else { return };
        w.set_ime_cursor_area(
            PhysicalPosition::new(CARET_X, CARET_Y),
            PhysicalSize::new(1, CARET_H),
        );
    }

    /// 画一屏内容。
    ///
    /// 不做文字渲染——那是 #3b 的事。这里只画色块标出模拟光标的位置，
    /// 让候选框有没有跟上肉眼可判。
    fn redraw(&mut self) {
        let Some(surface) = &mut self.surface else {
            return;
        };
        let (Some(w), Some(h)) = (
            NonZeroU32::new(self.size.width),
            NonZeroU32::new(self.size.height),
        ) else {
            return; // 最小化时尺寸为 0
        };

        if surface.resize(w, h).is_err() {
            return;
        }
        let Ok(mut buf) = surface.buffer_mut() else {
            return;
        };

        let (width, height) = (w.get() as i32, h.get() as i32);

        // 底色：浅灰，能看出窗口确实出现了
        buf.fill(0x00f2_f2f0);

        // 模拟光标：红色竖条。候选框应该贴着它出现。
        fill_rect(&mut buf, width, height, CARET_X, CARET_Y, 2, CARET_H, 0x00d0_2020);

        // 预编辑串的占位条：有 preedit 时在光标右侧画蓝条，
        // 长度随字符数变化，用来确认 Preedit 事件确实在实时到达。
        let n = self.preedit.chars().count() as i32;
        if n > 0 {
            fill_rect(
                &mut buf,
                width,
                height,
                CARET_X + 4,
                CARET_Y + CARET_H - 3,
                n * 14,
                2,
                0x0020_60d0,
            );
        }

        // 已提交文本的占位条：绿条，长度随已提交字符数增长
        let m = self.committed.chars().count() as i32;
        if m > 0 {
            fill_rect(&mut buf, width, height, 40, 60, (m * 14).min(width - 80), 6, 0x0020_9040);
        }

        let _ = buf.present();
    }

    fn report(&self) {
        println!("────────────────────────────────────────");
        println!("═══ 统计 ═══");
        println!("Ime::Enabled       {}", self.stats.enabled);
        println!("Ime::Preedit       {}", self.stats.preedit_events);
        println!("  非空带 cursor    {}", self.stats.preedit_with_cursor);
        println!("  非空缺 cursor    {}", self.stats.nonempty_without_cursor);
        println!("  空串（清除信号） {}", self.stats.empty_preedit);
        println!("Ime::Commit        {}", self.stats.commit_events);
        println!("Ime::Disabled      {}", self.stats.disabled);
        println!("最终文本           {:?}", self.committed);
        println!();

        if self.stats.enabled == 0 {
            println!("❌ 连 Ime::Enabled 都没收到 —— IME 未接通");
        } else if self.stats.preedit_events == 0 || self.stats.commit_events == 0 {
            println!("❌ 缺 Preedit 或 Commit —— 通路不完整");
        } else {
            println!("✅ 事件通路可用：Preedit 和 Commit 都收到了");
            if self.stats.nonempty_without_cursor > 0 {
                println!(
                    "⚠️  {} 次非空 Preedit 缺 cursor 范围，下划线分段需兜底",
                    self.stats.nonempty_without_cursor
                );
            } else {
                println!("✅ 所有非空 Preedit 都带 cursor 范围，下划线分段可直接实现");
            }
        }
        println!();
        println!("候选框是否贴着窗口内 ({CARET_X}, {CARET_Y}) 的红竖条，需人工确认。");
    }
}

/// 往 softbuffer 里填一个矩形。裁剪到缓冲区边界内。
fn fill_rect(
    buf: &mut [u32],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    color: u32,
) {
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + w).min(width);
    let y1 = (y + h).min(height);

    for row in y0..y1 {
        let base = (row * width) as usize;
        for col in x0..x1 {
            buf[base + col as usize] = color;
        }
    }
}

fn main() {
    let el = EventLoop::new().expect("创建事件循环失败");
    el.set_control_flow(ControlFlow::Wait);
    let mut app = App::default();
    el.run_app(&mut app).expect("事件循环异常退出");
}
