use super::{ThemeKind, Tokens, fonts};
use std::path::{Path, PathBuf};

/// The P1_UI token table: (name, light, dark).
const TABLE: [(&str, u32, u32); 24] = [
    ("menubar", 0xEDEDED, 0x2A2A2A),
    ("toolbar", 0xEDEDED, 0x2A2A2A),
    ("focusbar", 0xF7F7F7, 0x313131),
    ("tabbar", 0xDEDEDE, 0x222222),
    ("tab_active", 0xE8E8E8, 0x1B1B1B),
    ("canvas", 0xE8E8E8, 0x1B1B1B),
    ("page", 0xFFFFFF, 0xFFFFFF),
    ("field", 0xFFFFFF, 0x1F1F1F),
    ("border", 0x8C8C8C, 0x7A7A7A),
    ("separator", 0xD2D2D2, 0x3C3C3C),
    ("hover", 0xDCDCDC, 0x3A3A3A),
    ("pressed", 0xCBCBCB, 0x474747),
    ("active_bg", 0xD6E3F8, 0x22395F),
    ("accent", 0x1D5BD0, 0x6EA4FF),
    ("focus_ring", 0x1D5BD0, 0x6EA4FF),
    ("text", 0x1F1F1F, 0xE8E8E8),
    ("text_dim", 0x595959, 0xA6A6A6),
    ("text_disabled", 0xA6A6A6, 0x5E5E5E),
    ("warning", 0x9A6700, 0xE0A82E),
    ("warning_bg", 0xFBEFC5, 0x3A2F12),
    ("warning_text", 0x5C3D00, 0xF3CF78),
    ("math_bg", 0xE1EBFB, 0xE1EBFB),
    ("tooltip", 0x262626, 0xE8E8E8),
    ("math_border", 0x9EBBEE, 0x9EBBEE),
];

fn value(t: &Tokens, name: &str) -> [u8; 3] {
    let c = match name {
        "menubar" => t.menubar,
        "toolbar" => t.toolbar,
        "focusbar" => t.focusbar,
        "tabbar" => t.tabbar,
        "tab_active" => t.tab_active,
        "canvas" => t.canvas,
        "page" => t.page,
        "field" => t.field,
        "border" => t.border,
        "separator" => t.separator,
        "hover" => t.hover,
        "pressed" => t.pressed,
        "active_bg" => t.active_bg,
        "accent" => t.accent,
        "focus_ring" => t.focus_ring,
        "text" => t.text,
        "text_dim" => t.text_dim,
        "text_disabled" => t.text_disabled,
        "warning" => t.warning,
        "warning_bg" => t.warning_bg,
        "warning_text" => t.warning_text,
        "math_bg" => t.math_bg,
        "math_border" => t.math_border,
        "tooltip" => t.tooltip,
        other => panic!("token {other} is not in the table"),
    };
    [c.r(), c.g(), c.b()]
}

fn rgb(hex: u32) -> [u8; 3] {
    [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8]
}

#[test]
fn every_semantic_token_has_the_p1_ui_value_in_both_themes() {
    for (name, light, dark) in TABLE {
        assert_eq!(value(&Tokens::LIGHT, name), rgb(light), "light {name}");
        assert_eq!(value(&Tokens::DARK, name), rgb(dark), "dark {name}");
    }
}

#[test]
fn the_tooltip_text_is_the_opposite_themes_text() {
    assert_eq!(Tokens::LIGHT.tooltip_text, Tokens::DARK.text);
    assert_eq!(Tokens::DARK.tooltip_text, Tokens::LIGHT.text);
}

#[test]
fn the_page_is_white_in_both_themes() {
    assert_eq!(Tokens::LIGHT.page, Tokens::DARK.page);
    assert_eq!(Tokens::of(ThemeKind::Dark).kind, ThemeKind::Dark);
    assert_eq!(ThemeKind::default(), ThemeKind::Light);
}

#[test]
fn bar_heights_sum_to_the_designed_chrome() {
    let top = Tokens::MENU_BAR + Tokens::TOOLBAR + Tokens::FOCUS_BAR + Tokens::TAB_BAR;
    assert_eq!(top, 122.0, "P1_UI: the four top bars total 122 logical px");
    assert_eq!(Tokens::STATUS_BAR, 24.0);
}

fn sources(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("source directory") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            sources(&path, found);
        } else if path.extension().is_some_and(|e| e == "rs") {
            found.push(path);
        }
    }
}

#[test]
fn no_color_literal_exists_outside_the_theme_module() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let theme = root.join("theme");
    let mut files = Vec::new();
    sources(&root, &mut files);
    let needle = ["Color32::", "from_rgb"].concat();
    let offenders: Vec<_> = files
        .iter()
        .filter(|p| !p.starts_with(&theme))
        .filter(|p| std::fs::read_to_string(p).is_ok_and(|s| s.contains(&needle)))
        .collect();
    assert!(
        offenders.is_empty(),
        "color literals outside theme: {offenders:?}"
    );
}

#[test]
fn the_first_installed_family_of_each_preference_list_wins() {
    let installed = |set: &'static [&'static str]| move |n: &str| set.contains(&n);
    let pick = |set| fonts::first_installed(&fonts::UI_FONTS, installed(set));
    assert_eq!(pick(&["Microsoft YaHei", "MiSans"]), Some("MiSans"));
    assert_eq!(
        pick(&["Microsoft YaHei", "Microsoft YaHei UI"]),
        Some("Microsoft YaHei UI")
    );
    assert_eq!(
        pick(&["HarmonyOS Sans SC", "PingFang SC"]),
        Some("HarmonyOS Sans SC")
    );
    assert_eq!(pick(&["DejaVu Sans"]), None, "Noto stays the last fallback");
}
