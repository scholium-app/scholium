//! Font sources for normal rendering and reproducible validation.
use typst::foundations::Bytes;
use typst::text::Font;
use typst_kit::fonts::FontStore;

/// Fixed before a World or worker starts; never changes within that World.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FontProfile {
    /// Existing product policy: embedded fallback plus host-installed fonts.
    #[default]
    System,
    /// Application-packaged Noto CJK first, then Typst's embedded fonts; no host scan.
    Bundled,
}

impl FontProfile {
    /// Opt-in validation setting. Unset leaves normal product rendering unchanged.
    pub fn from_environment() -> Self {
        if std::env::var_os("SCHOLIUM_TEST_FONTS").is_some_and(|v| v == "bundled") {
            Self::Bundled
        } else {
            Self::System
        }
    }

    pub(crate) fn body(self, normal: &'static [&'static str]) -> &'static [&'static str] {
        match self {
            Self::System => normal,
            Self::Bundled => &["Noto Sans CJK SC"],
        }
    }

    pub(crate) fn load(self) -> FontStore {
        let mut fonts = FontStore::new();
        if self == Self::Bundled {
            let data = include_bytes!("../../scholium-app/assets/fonts/NotoSansCJKsc-Regular.otf");
            fonts.extend(Font::iter(Bytes::new(data.as_slice())).map(|font| {
                let info = font.info().clone();
                (font, info)
            }));
        }
        fonts.extend(typst_kit::fonts::embedded());
        if self == Self::System {
            fonts.extend(typst_kit::fonts::system());
        }
        fonts
    }
}
