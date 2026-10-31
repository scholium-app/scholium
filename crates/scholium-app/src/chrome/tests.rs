//! Whole-chrome frames in a headless egui context: controls are found through the same
//! audit map the real-window checks use, never by coordinates.
use super::{count_words, show};
use crate::diagnostics::{Diagnostic, Severity};
use crate::{
    command::{registry, spec::MenuId, spec::Submenu},
    i18n::LocaleSetting,
    state::{ViewMode, WorkspaceState},
    theme::{self, ThemeKind, Tokens},
    typst_editor::EditorView,
    widgets::audit::{self, Entry},
};
use eframe::egui;

pub(crate) fn overflow() -> Diagnostic {
    Diagnostic::new(Severity::Warning, "diag.formula_overflow")
}

pub(crate) struct Window {
    ctx: egui::Context,
    pub(crate) state: WorkspaceState,
    size: egui::Vec2,
    time: f64,
    output: egui::FullOutput,
}

impl Window {
    pub(crate) fn new(width: f32, locale: LocaleSetting) -> Self {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let state = WorkspaceState {
            locale,
            editor: Some(EditorView::default()),
            ..Default::default()
        };
        let mut window = Self {
            ctx,
            state,
            size: egui::vec2(width, 700.0),
            time: 0.0,
            output: egui::FullOutput::default(),
        };
        window.settle();
        window
    }

    fn frame(&mut self, events: Vec<egui::Event>) {
        self.time += 0.2;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
            events,
            time: Some(self.time),
            ..Default::default()
        };
        let state = &mut self.state;
        let mut output = self.ctx.run_ui(input, |ui| show(ui, state));
        output.textures_delta.clear();
        self.output = output;
    }

    pub(crate) fn settle(&mut self) {
        for _ in 0..5 {
            self.frame(vec![]);
        }
    }

    fn entries(&self) -> Vec<Entry> {
        audit::entries(&self.ctx)
    }

    fn on(&self, surface: &str) -> Vec<Entry> {
        let mut found: Vec<Entry> = self
            .entries()
            .into_iter()
            .filter(|e| e.surface == surface)
            .collect();
        found.sort_by(|a, b| {
            (a.rect.min.y, a.rect.min.x)
                .partial_cmp(&(b.rect.min.y, b.rect.min.x))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        found
    }

    pub(crate) fn ids(&self, surface: &str) -> Vec<String> {
        self.on(surface).into_iter().map(|e| e.id).collect()
    }

    pub(crate) fn rects(&self, surface: &str) -> Vec<egui::Rect> {
        self.on(surface).into_iter().map(|e| e.rect).collect()
    }

    fn find(&self, id: &str, surface: &str) -> Entry {
        self.entries()
            .into_iter()
            .find(|e| e.id == id && e.surface == surface)
            .unwrap_or_else(|| panic!("no control {id} on {surface}"))
    }

    fn hover(&mut self, id: &str, surface: &str) {
        let pos = self.find(id, surface).rect.center();
        self.frame(vec![egui::Event::PointerMoved(pos)]);
        self.settle();
    }

    pub(crate) fn click(&mut self, id: &str, surface: &str) {
        self.click_nth(id, surface, 0);
    }

    /// Click the `index`-th control (in audit order) that has this id.
    pub(crate) fn click_nth(&mut self, id: &str, surface: &str, index: usize) {
        let pos = self
            .entries()
            .into_iter()
            .filter(|e| e.id == id && e.surface == surface)
            .nth(index)
            .unwrap_or_else(|| panic!("no control {id} #{index} on {surface}"))
            .rect
            .center();
        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        self.frame(vec![egui::Event::PointerMoved(pos)]);
        self.frame(vec![button(true)]);
        self.frame(vec![button(false)]);
        self.settle();
    }

    /// Give a control keyboard focus without clicking it.
    pub(crate) fn focus(&mut self, id: &str, surface: &'static str) {
        let egui_id = crate::widgets::Meta::new(id, surface, "").id_for();
        self.ctx.memory_mut(|m| m.request_focus(egui_id));
        self.settle();
    }

    /// One key event in one frame, then the frames that follow it.
    pub(crate) fn press(&mut self, event: egui::Event) {
        self.frame(vec![event]);
    }

    pub(crate) fn focused_row(&self) -> Option<usize> {
        (0..16).find(|i| {
            self.ctx
                .memory(|m| m.has_focus(crate::diagnostics::panel::row_id(*i)))
        })
    }

    pub(crate) fn position_of(&self, text: &str) -> egui::Pos2 {
        self.painted()
            .into_iter()
            .find(|(_, g)| g.text() == text)
            .map(|(p, _)| p)
            .unwrap_or_else(|| panic!("{text:?} is not painted"))
    }

    pub(crate) fn status_top(&self) -> f32 {
        self.size.y - Tokens::STATUS_BAR
    }

    /// The panel's rectangle once drawn, `None` while it is closed.
    pub(crate) fn panel_rect(&self) -> Option<egui::Rect> {
        let open = self.texts().iter().any(|t| t == "问题" || t == "Problems");
        let state = egui::containers::panel::PanelState::load(
            &self.ctx,
            crate::diagnostics::panel::panel_id(),
        )?;
        open.then_some(state.outer_rect)
    }

    pub(crate) fn painted(&self) -> Vec<(egui::Pos2, std::sync::Arc<egui::Galley>)> {
        self.output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) => Some((t.pos, t.galley.clone())),
                _ => None,
            })
            .collect()
    }

    pub(crate) fn texts(&self) -> Vec<String> {
        self.painted()
            .into_iter()
            .map(|(_, g)| g.text().to_owned())
            .collect()
    }

    pub(crate) fn editor(&mut self) -> &mut EditorView {
        self.state.editor.get_or_insert_with(Default::default)
    }
}

pub(crate) const ZH: LocaleSetting = LocaleSetting::ZhHans;
pub(crate) const EN: LocaleSetting = LocaleSetting::En;

#[test]
fn the_menu_bar_lists_the_eight_menus_left_to_right() {
    let w = Window::new(1280.0, ZH);
    let expected: Vec<String> = MenuId::ALL
        .iter()
        .map(|m| format!("ui.{}", m.key()))
        .collect();
    assert_eq!(w.ids("menubar"), expected);
}

/// The registry's direct menu entries, independently ordered by (group, order).
fn expected_menu(menu: MenuId) -> Vec<String> {
    let mut rows: Vec<((u8, u8), String)> = registry::all()
        .iter()
        .filter_map(|s| {
            s.menu
                .filter(|m| m.menu == menu && m.submenu.is_none())
                .map(|m| ((m.group, m.order), s.id.to_owned()))
        })
        .collect();
    for sub in registry::submenus(menu) {
        let first = registry::all()
            .iter()
            .filter_map(|s| s.menu.filter(|m| m.menu == menu && m.submenu == Some(sub)))
            .map(|m| (m.group, m.order))
            .min();
        rows.extend(first.map(|at| (at, format!("ui.{}", sub.key()))));
    }
    rows.sort();
    rows.into_iter().map(|(_, id)| id).collect()
}

#[test]
fn every_open_menu_equals_the_registry_tree() {
    for locale in [ZH, EN] {
        for menu in MenuId::ALL {
            let mut w = Window::new(1280.0, locale);
            w.click(&format!("ui.{}", menu.key()), "menubar");
            assert_eq!(w.ids("menu"), expected_menu(menu), "{locale:?} {menu:?}");
        }
    }
}

#[test]
fn a_submenu_opens_on_hover_and_lists_its_registry_entries() {
    let mut w = Window::new(1280.0, ZH);
    w.click("ui.menu.format", "menubar");
    w.hover("ui.menu.format.paragraph_style", "menu");
    let rows = w.ids("menu");
    let sub: Vec<_> = registry::menu_items(MenuId::Format, Some(Submenu::ParagraphStyle))
        .iter()
        .map(|s| s.id.to_owned())
        .collect();
    assert!(sub.iter().all(|id| rows.contains(id)), "{rows:?}");
}

#[test]
fn hovering_another_title_switches_the_open_menu() {
    let mut w = Window::new(1280.0, ZH);
    w.click("ui.menu.file", "menubar");
    assert!(w.ids("menu").contains(&"file.save".to_owned()));
    w.hover("ui.menu.edit", "menubar");
    assert!(w.ids("menu").contains(&"edit.undo".to_owned()));
    assert!(!w.ids("menu").contains(&"file.save".to_owned()));
}

#[test]
fn only_checkable_commands_are_exposed_as_toggles() {
    let w = Window::new(1280.0, ZH);
    assert!(w.find("format.bold", "toolbar").checked.is_some());
    assert!(w.find("edit.undo", "toolbar").checked.is_none());
    assert!(w.find("view.mode.page", "focus").checked == Some(true));
    assert!(w.find("view.mode.source", "focus").checked == Some(false));
}

#[test]
fn unimplemented_toolbar_commands_are_disabled_with_a_reason() {
    let w = Window::new(1280.0, ZH);
    let underline = w.find("format.underline", "toolbar");
    assert!(!underline.enabled);
    let state = &w.state;
    let tip = crate::command::ui::resolve(state, "format.underline").map(|r| r.tip(state));
    assert_eq!(
        tip.and_then(|t| t.reason).as_deref(),
        Some("此功能尚未接入")
    );
    assert!(
        !w.find("tools.search", "toolbar").enabled,
        "the search opens nothing until T14"
    );
}

fn x_order(entries: &[Entry]) -> Vec<String> {
    let mut sorted = entries.to_vec();
    sorted.sort_by(|a, b| {
        a.rect
            .min
            .x
            .partial_cmp(&b.rect.min.x)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    sorted.into_iter().map(|e| e.id).collect()
}

#[test]
fn the_toolbar_follows_the_registry_groups_and_ends_with_the_search() {
    let w = Window::new(1280.0, ZH);
    let mut expected = vec!["ui.toolbar.paragraph".to_owned()];
    for (group, items) in registry::toolbar_groups() {
        if group != crate::command::spec::ToolbarGroup::Paragraph {
            expected.extend(items.iter().map(|s| s.id.to_owned()));
        }
    }
    expected.push("tools.search".to_owned());
    assert_eq!(x_order(&w.on("toolbar")), expected);
}

#[test]
fn a_narrow_window_moves_whole_trailing_groups_into_overflow_in_order() {
    let mut w = Window::new(420.0, ZH);
    let shown = x_order(&w.on("toolbar"));
    assert!(shown.contains(&"ui.toolbar.overflow".to_owned()));
    assert!(
        shown.contains(&"tools.search".to_owned()),
        "search stays visible"
    );
    assert!(shown.contains(&"format.bold".to_owned()));
    assert!(!shown.contains(&"edit.undo".to_owned()));
    w.click("ui.toolbar.overflow", "toolbar");
    let listed = w.ids("overflow");
    let hidden: Vec<String> = registry::toolbar_groups()
        .into_iter()
        .filter(|(g, _)| {
            matches!(
                g,
                crate::command::spec::ToolbarGroup::Insert
                    | crate::command::spec::ToolbarGroup::History
            )
        })
        .flat_map(|(_, items)| items.into_iter().map(|s| s.id.to_owned()))
        .collect();
    assert_eq!(listed, hidden);
    assert!(
        w.find("view.mode.page", "focus").enabled,
        "the mode switch stays visible"
    );
}

#[test]
fn every_group_fits_in_a_wide_window_and_no_overflow_button_shows() {
    let w = Window::new(1440.0, EN);
    assert!(!w.ids("toolbar").contains(&"ui.toolbar.overflow".to_owned()));
}

#[test]
fn the_focus_bar_shows_document_properties_in_text_and_formula_commands_in_math() {
    let mut w = Window::new(1280.0, ZH);
    let text = x_order(&w.on("focus"));
    for id in [
        "document.style_template",
        "document.page_size",
        "document.language",
        "document.font",
        "document.font_size",
    ] {
        assert!(text.contains(&id.to_owned()), "{id} in {text:?}");
        assert!(!w.find(id, "focus").enabled, "{id} is read-only for now");
    }
    assert!(!text.contains(&"insert.fraction".to_owned()));
    w.editor().in_math = true;
    w.settle();
    let math = x_order(&w.on("focus"));
    for id in [
        "insert.fraction",
        "insert.exit_formula",
        "view.mode.page",
        "view.mode.source",
    ] {
        assert!(math.contains(&id.to_owned()), "{id} in {math:?}");
    }
    assert!(!math.contains(&"document.page_size".to_owned()));
    assert!(w.texts().contains(&"公式".to_owned()), "the formula tag");
}

#[test]
fn the_focus_bar_leaves_formulas_alone_in_the_source_view() {
    let mut w = Window::new(1280.0, ZH);
    w.editor().in_math = true;
    w.state.mode = ViewMode::Source;
    w.settle();
    assert!(w.ids("focus").contains(&"document.page_size".to_owned()));
}

#[test]
fn mode_segments_are_at_least_96_wide_and_the_switch_is_rightmost() {
    for locale in [ZH, EN] {
        let w = Window::new(1024.0, locale);
        let page = w.find("view.mode.page", "focus");
        let source = w.find("view.mode.source", "focus");
        assert!(
            page.rect.width() >= 96.0 && source.rect.width() >= 96.0,
            "{locale:?}"
        );
        assert_eq!(page.rect.height(), Tokens::FIELD);
        assert!(
            source.rect.right() > 1024.0 - 16.0,
            "right-aligned: {:?}",
            source.rect
        );
        for entry in w.on("focus") {
            assert!(
                entry.rect.right() <= source.rect.right() + 0.5,
                "{}",
                entry.id
            );
        }
    }
}

#[test]
fn the_bars_have_the_designed_heights() {
    let w = Window::new(1280.0, ZH);
    assert_eq!(
        w.find("ui.menu.file", "menubar").rect.height(),
        Tokens::MENU_BAR
    );
    let bold = w.find("format.bold", "toolbar");
    assert_eq!(bold.rect.size(), egui::vec2(28.0, 28.0));
    assert_eq!(
        bold.rect.center().y,
        Tokens::MENU_BAR + Tokens::TOOLBAR / 2.0
    );
    let field = w.find("document.page_size", "focus");
    assert_eq!(field.rect.height(), Tokens::FIELD);
    assert_eq!(
        field.rect.center().y,
        Tokens::MENU_BAR + Tokens::TOOLBAR + Tokens::FOCUS_BAR / 2.0
    );
    let tab = w.find("ui.tab", "tab");
    assert_eq!(tab.rect.height(), Tokens::TAB);
    // The four top bars total 122 and the current tab sits on the bar's bottom edge.
    assert_eq!(tab.rect.bottom(), 122.0);
}

#[test]
fn hiding_the_toolbars_keeps_menu_tabs_and_status() {
    let mut w = Window::new(1280.0, ZH);
    crate::commands::dispatch(&mut w.state, crate::commands::ViewCommand::ToggleToolbars);
    w.settle();
    assert!(w.on("toolbar").is_empty() && w.on("focus").is_empty());
    assert!(!w.on("menubar").is_empty() && !w.on("tab").is_empty());
    assert!(w.texts().iter().any(|t| t == "正文"));
}

#[test]
fn the_tab_shows_the_title_and_its_buttons_use_the_new_and_close_flows() {
    let mut w = Window::new(1280.0, ZH);
    assert!(w.texts().contains(&"未命名".to_owned()));
    w.editor().title = Some("paper".into());
    w.settle();
    assert!(w.texts().contains(&"paper".to_owned()));
    w.click("file.close", "tab");
    assert!(
        w.state.close_requested,
        "the close button asks the editor's close flow"
    );
    w.click("file.new", "tab");
    assert!(w.state.new_requested);
}

#[test]
fn switching_the_language_relabels_every_visible_control_at_runtime() {
    let mut w = Window::new(1280.0, ZH);
    let zh = w.texts();
    assert!(zh.contains(&"文件".to_owned()) && zh.contains(&"页面编辑".to_owned()));
    w.state.locale = EN;
    w.settle();
    let en = w.texts();
    assert!(
        en.contains(&"File".to_owned()) && en.contains(&"Page Editing".to_owned()),
        "{en:?}"
    );
    assert!(
        !en.iter()
            .any(|t| t.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))),
        "{en:?}"
    );
    w.state.locale = ZH;
    w.settle();
    assert!(w.texts().contains(&"文件".to_owned()));
}

#[test]
fn the_language_menu_switches_the_locale_through_the_registry() {
    let mut w = Window::new(1280.0, ZH);
    w.click("ui.menu.view", "menubar");
    w.hover("ui.menu.view.language", "menu");
    w.click("view.language.en", "menu");
    assert_eq!(w.state.locale, EN);
}

#[test]
fn the_theme_menu_switches_between_light_and_dark() {
    let mut w = Window::new(1280.0, ZH);
    assert_eq!(theme::tokens(&w.ctx), Tokens::LIGHT);
    w.click("ui.menu.view", "menubar");
    w.hover("ui.menu.view.theme", "menu");
    w.click("view.theme.dark", "menu");
    assert_eq!(w.state.theme, ThemeKind::Dark);
    assert_eq!(theme::tokens(&w.ctx), Tokens::DARK);
    let fills: Vec<_> = w
        .output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Rect(r) => Some(r.fill),
            _ => None,
        })
        .collect();
    assert!(
        fills.contains(&Tokens::DARK.menubar),
        "the bars repaint with the dark tokens"
    );
}

/// No label wraps or is cut off, and no two controls on a bar overlap, in either locale
/// at the sizes the design names.
#[test]
fn no_text_wraps_or_overflows_and_no_controls_overlap() {
    for locale in [ZH, EN] {
        for width in [640.0, 1024.0, 1440.0] {
            let mut w = Window::new(width, locale);
            w.editor().words = 12_345;
            w.editor().problems = vec![
                Diagnostic::new(Severity::Error, "diag.edit").arg(
                    "detail",
                    "公式超出行宽 · 2.1 网格划分，第 1 段，超出 3.2 pt",
                ),
                overflow(),
            ];
            w.settle();
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, w.size);
            for (pos, galley) in w.painted() {
                assert_eq!(
                    galley.rows.len(),
                    1,
                    "{locale:?} {width}: {:?} wraps",
                    galley.text()
                );
                let rect = egui::Rect::from_min_size(pos, galley.size());
                assert!(
                    screen.expand(1.0).contains_rect(rect),
                    "{locale:?} {width}: {:?} at {rect:?}",
                    galley.text()
                );
            }
            for surface in ["menubar", "toolbar", "focus", "status"] {
                let on = w.on(surface);
                for (i, a) in on.iter().enumerate() {
                    for b in &on[i + 1..] {
                        assert!(
                            !a.rect.shrink(0.5).intersects(b.rect.shrink(0.5)),
                            "{locale:?} {width}: {} overlaps {}",
                            a.id,
                            b.id
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn words_count_cjk_characters_and_latin_words() {
    assert_eq!(count_words("中文 English 3"), 4);
}
