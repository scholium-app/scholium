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
    window: Option<Window>,
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
    /// preedit 是否带过光标范围——决定能否画下划线的分段
    preedit_with_cursor: usize,
    preedit_without_cursor: usize,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        let attrs = Window::default_attributes()
            .with_title("Scholium P0 — IME 验证")
            .with_inner_size(PhysicalSize::new(720, 400));

        let window = el.create_window(attrs).expect("创建窗口失败");

        // 关键：必须显式开启，否则收不到任何 Ime 事件
        window.set_ime_allowed(true);

        // 关键：不设这个，候选框会飘到屏幕角落而不是跟着光标
        window.set_ime_cursor_area(
            PhysicalPosition::new(CARET_X, CARET_Y),
            PhysicalSize::new(1, CARET_H),
        );

        println!("窗口已创建。");
        println!("  set_ime_allowed(true) 已调用");
        println!("  set_ime_cursor_area 设到 ({CARET_X}, {CARET_Y})，高 {CARET_H}px");
        println!();
        println!("请切到中文输入法，敲拼音。观察：");
        println!("  1. 是否收到 Ime::Enabled");
        println!("  2. 敲拼音时是否收到 Ime::Preedit，内容是否是拼音串");
        println!("  3. 候选框是否出现在窗口内 ({CARET_X}, {CARET_Y}) 附近，而非屏幕角落");
        println!("  4. 选字后是否收到 Ime::Commit");
        println!();
        println!("按 Esc 退出并打印统计。");
        println!("────────────────────────────────────────");

        self.window = Some(window);
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

            WindowEvent::Ime(ime) => match ime {
                Ime::Enabled => {
                    self.stats.enabled += 1;
                    println!("[{t:>6}ms] Ime::Enabled");
                }

                Ime::Preedit(text, cursor) => {
                    self.stats.preedit_events += 1;
                    if cursor.is_some() {
                        self.stats.preedit_with_cursor += 1;
                    } else {
                        self.stats.preedit_without_cursor += 1;
                    }
                    println!(
                        "[{t:>6}ms] Ime::Preedit  {text:?}  cursor={cursor:?}  \
                         (字节 {} / 字符 {})",
                        text.len(),
                        text.chars().count()
                    );
                    self.preedit = text;
                }

                Ime::Commit(text) => {
                    self.stats.commit_events += 1;
                    println!("[{t:>6}ms] Ime::Commit   {text:?}");
                    self.committed.push_str(&text);
                    self.preedit.clear();
                    println!("           已提交文本: {:?}", self.committed);
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
    fn report(&self) {
        println!("────────────────────────────────────────");
        println!("═══ 统计 ═══");
        println!("Ime::Enabled       {}", self.stats.enabled);
        println!("Ime::Preedit       {}", self.stats.preedit_events);
        println!("  带 cursor 范围   {}", self.stats.preedit_with_cursor);
        println!("  不带 cursor      {}", self.stats.preedit_without_cursor);
        println!("Ime::Commit        {}", self.stats.commit_events);
        println!("Ime::Disabled      {}", self.stats.disabled);
        println!("最终文本           {:?}", self.committed);
        println!();

        let pass = self.stats.preedit_events > 0 && self.stats.commit_events > 0;
        if pass {
            println!("✅ 事件通路可用：Preedit 和 Commit 都收到了");
            if self.stats.preedit_without_cursor > 0 {
                println!("⚠️  部分 Preedit 不带 cursor 范围，画下划线分段时需兜底");
            }
        } else if self.stats.enabled == 0 {
            println!("❌ 连 Ime::Enabled 都没收到 —— IME 未接通");
        } else {
            println!("❌ 缺 Preedit 或 Commit —— 通路不完整");
        }
        println!();
        println!("候选框位置是否跟随 ({CARET_X}, {CARET_Y})，需人工确认。");
    }
}

fn main() {
    let el = EventLoop::new().expect("创建事件循环失败");
    el.set_control_flow(ControlFlow::Wait);
    let mut app = App::default();
    el.run_app(&mut app).expect("事件循环异常退出");
}
