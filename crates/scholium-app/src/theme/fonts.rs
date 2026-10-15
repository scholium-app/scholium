//! Interface fonts: the first installed system font of each preference list, with the
//! bundled Noto Sans CJK only as the last fallback so the app works anywhere.
//!
//! Page text is Typst's business; these fonts draw the chrome only.
use eframe::egui::{self, FontData, FontDefinitions, FontFamily};
use std::sync::OnceLock;

/// Chinese-capable interface fonts, first installed wins.
pub(crate) const UI_FONTS: [&str; 5] = [
    "HarmonyOS Sans SC",
    "MiSans",
    "PingFang SC",
    "Microsoft YaHei UI",
    "Microsoft YaHei",
];
/// Latin interface font, used before the CJK font when installed.
pub(crate) const LATIN_FONTS: [&str; 1] = ["Inter"];
/// Monospace font of the generated source.
pub(crate) const MONO_FONTS: [&str; 1] = ["JetBrains Mono"];

const BUNDLED: &str = "noto-cjk";
const BUNDLED_NAME: &str = "Noto Sans CJK SC (bundled)";

/// What the chrome is drawn with, for the window audit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Chosen {
    pub(crate) ui: String,
    pub(crate) latin: Option<String>,
    pub(crate) mono: Option<String>,
}

static CHOSEN: OnceLock<Chosen> = OnceLock::new();

/// The fonts chosen at startup; `None` before [`install`].
pub(crate) fn chosen() -> Option<&'static Chosen> {
    CHOSEN.get()
}

/// The first name of `preference` that `installed` accepts.
pub(crate) fn first_installed<'a>(
    preference: &[&'a str],
    installed: impl Fn(&str) -> bool,
) -> Option<&'a str> {
    preference.iter().copied().find(|name| installed(name))
}

pub(crate) fn install(ctx: &egui::Context) {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let (fonts, chosen) = definitions(&db);
    ctx.set_fonts(fonts);
    let _ = CHOSEN.set(chosen);
}

fn has_family(db: &fontdb::Database, name: &str) -> bool {
    db.faces()
        .any(|face| face.families.iter().any(|(family, _)| family == name))
}

/// Regular-weight bytes and collection index of an installed family.
fn load(db: &fontdb::Database, name: &str) -> Option<FontData> {
    let query = fontdb::Query {
        families: &[fontdb::Family::Name(name)],
        ..Default::default()
    };
    let id = db.query(&query)?;
    db.with_face_data(id, |bytes, index| {
        let mut data = FontData::from_owned(bytes.to_vec());
        data.index = index;
        data
    })
}

fn definitions(db: &fontdb::Database) -> (FontDefinitions, Chosen) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        BUNDLED.into(),
        FontData::from_static(include_bytes!(
            "../../assets/fonts/NotoSansCJKsc-Regular.otf"
        ))
        .into(),
    );
    let ui = load_first(db, &UI_FONTS, &mut fonts);
    let latin = load_first(db, &LATIN_FONTS, &mut fonts);
    let mono = load_first(db, &MONO_FONTS, &mut fonts);
    prefer(&mut fonts, FontFamily::Proportional, [&latin, &ui]);
    prefer(&mut fonts, FontFamily::Monospace, [&mono, &ui]);
    let chosen = Chosen {
        ui: ui.unwrap_or_else(|| BUNDLED_NAME.to_owned()),
        latin,
        mono,
    };
    (fonts, chosen)
}

fn data_key(name: &str) -> String {
    format!("system:{name}")
}

/// Register the first installed family of `preference`; its name when it loaded.
fn load_first(
    db: &fontdb::Database,
    preference: &[&str],
    fonts: &mut FontDefinitions,
) -> Option<String> {
    let name = first_installed(preference, |name| has_family(db, name))?;
    let data = load(db, name)?;
    fonts.font_data.insert(data_key(name), data.into());
    Some(name.to_owned())
}

/// Put the chosen system fonts ahead of egui's defaults and the bundled font last.
fn prefer(fonts: &mut FontDefinitions, family: FontFamily, names: [&Option<String>; 2]) {
    let list = fonts.families.entry(family).or_default();
    let front: Vec<String> = names.into_iter().flatten().map(|n| data_key(n)).collect();
    list.splice(0..0, front);
    list.push(BUNDLED.into());
}
