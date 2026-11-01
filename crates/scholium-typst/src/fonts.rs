//! Font sources for normal rendering and reproducible validation.
use std::io::Read;
use std::sync::OnceLock;

use typst::foundations::Bytes;
use typst::text::Font;
use typst_kit::fonts::FontStore;

#[cfg(test)]
const BUNDLED_ASSET_NAMES: [&str; 5] = [
    "NotoSansCJKsc-Regular.otf",
    "LiberationSerif-Regular.ttf",
    "LiberationSerif-Italic.ttf",
    "LiberationSerif-Bold.ttf",
    "LiberationSerif-BoldItalic.ttf",
];

const BUNDLED_ASSETS: [&[u8]; 5] = [
    include_bytes!("../../scholium-app/assets/fonts/NotoSansCJKsc-Regular.otf.br"),
    include_bytes!("../../scholium-app/assets/fonts/LiberationSerif-Regular.ttf.br"),
    include_bytes!("../../scholium-app/assets/fonts/LiberationSerif-Italic.ttf.br"),
    include_bytes!("../../scholium-app/assets/fonts/LiberationSerif-Bold.ttf.br"),
    include_bytes!("../../scholium-app/assets/fonts/LiberationSerif-BoldItalic.ttf.br"),
];

static DECOMPRESSED_ASSETS: OnceLock<Vec<Vec<u8>>> = OnceLock::new();

#[cfg(test)]
mod layout_tests;

fn decompressed_assets() -> &'static [Vec<u8>] {
    DECOMPRESSED_ASSETS.get_or_init(|| {
        BUNDLED_ASSETS
            .iter()
            .map(|compressed| {
                let mut decoder = brotli_decompressor::Decompressor::new(*compressed, 16 * 1024);
                let mut bytes = Vec::new();
                decoder
                    .read_to_end(&mut bytes)
                    .expect("bundled Brotli font data must decompress");
                bytes
            })
            .collect()
    })
}

/// Original Noto Sans CJK SC bytes, shared by Typst and the interface font loader.
/// Lossless decompression happens once per process, without file or network access.
#[must_use]
pub fn bundled_cjk_font_bytes() -> &'static [u8] {
    &decompressed_assets()[0]
}

/// Fixed before a World or worker starts; never changes within that World.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FontProfile {
    /// Packaged and Typst embedded fonts, plus host-installed fonts.
    #[default]
    System,
    /// Application-packaged fonts first, then Typst's embedded fonts; no host scan.
    Bundled,
}

impl FontProfile {
    /// Opt-in validation setting. Unset selects packaged fonts plus host scanning.
    pub fn from_environment() -> Self {
        if std::env::var_os("SCHOLIUM_TEST_FONTS").is_some_and(|v| v == "bundled") {
            Self::Bundled
        } else {
            Self::System
        }
    }

    pub(crate) fn body(self, normal: &'static [&'static str]) -> &'static [&'static str] {
        normal
    }

    pub(crate) fn load(self) -> FontStore {
        let mut fonts = FontStore::new();
        for data in decompressed_assets() {
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

#[cfg(test)]
mod tests {
    use super::{BUNDLED_ASSET_NAMES, BUNDLED_ASSETS, FontProfile, decompressed_assets};
    use sha2::{Digest, Sha256};

    const EXPECTED_SHA256: [&str; 5] = [
        "2c76254f6fc379fddfce0a7e84fb5385bb135d3e399294f6eeb6680d0365b74b",
        "058ea80864aef09a23f45cbec2bb5400bc3dfbdea01c3f10538a21fcb497fb74",
        "0e3dea9f8d613e006ccfa62201f33e265d19167bd0907725c3e145368b04fc2e",
        "d754ba427cfe0bca54ae052384baa8f842da5bd6550ad4da024ac441e7a7d5ce",
        "f17db8af71e24d2066b587546021d4f0b296be389512b658dec3c09affeb11a7",
    ];

    #[test]
    fn both_profiles_use_the_same_portable_document_families() {
        let body = &[
            "Times New Roman",
            "Liberation Serif",
            "SimSun",
            "Noto Sans CJK SC",
        ];
        let heading = &[
            "Times New Roman",
            "Liberation Serif",
            "SimHei",
            "Noto Sans CJK SC",
        ];
        assert_eq!(crate::BODY_FONTS, body);
        assert_eq!(crate::HEADING_FONTS, heading);
        for profile in [FontProfile::Bundled, FontProfile::System] {
            assert_eq!(profile.body(crate::BODY_FONTS), body);
            assert_eq!(profile.body(crate::HEADING_FONTS), heading);
            let store = profile.load();
            assert!(store.book().select_family("liberation serif").count() >= 4);
            assert!(store.book().select_family("noto sans cjk sc").count() >= 1);
        }
    }

    #[test]
    fn bundled_profile_has_no_host_times_or_simsun() {
        let store = FontProfile::Bundled.load();
        assert_eq!(store.book().select_family("times new roman").count(), 0);
        assert_eq!(store.book().select_family("simsun").count(), 0);
        assert_eq!(store.book().select_family("liberation serif").count(), 4);
    }

    #[test]
    fn bundled_brotli_assets_restore_recorded_bytes() {
        for (((name, compressed), bytes), expected) in BUNDLED_ASSET_NAMES
            .iter()
            .zip(BUNDLED_ASSETS)
            .zip(decompressed_assets())
            .zip(EXPECTED_SHA256)
        {
            assert!(!compressed.is_empty(), "{name} compressed asset is empty");
            let digest = format!("{:x}", Sha256::digest(bytes));
            assert_eq!(digest, expected, "{name} changed after decompression");
        }
    }
}
