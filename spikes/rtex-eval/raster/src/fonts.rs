//! Glyph outlines for display-list fonts, in em units (1.0 = one em, y up), cached per glyph.
//!
//! OpenType/TrueType/CFF faces are named by file in the display list. TFM fonts (classic
//! Computer Modern math, `cmmi10`, …) carry only their TFM name: they are resolved the way
//! LuaTeX's PDF backend resolves them, through `pdftex.map` to a Type 1 file and an optional
//! encoding vector, both located with `kpsewhich` (a trusted host tool).

use crate::dl::Font;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::raw::ps::type1::Type1Font;
use skrifa::{FontRef, GlyphId, MetadataProvider};
use std::collections::{BTreeSet, HashMap};
use std::process::Command;
use std::rc::Rc;
use tiny_skia::{Path, PathBuilder};

pub enum Glyph {
    Outline(Rc<Path>),
    /// A glyph without ink (a space): drawn, nothing to fill.
    Blank,
    /// The font or the glyph could not be resolved: counted as skipped.
    Missing,
}

enum Face {
    Open {
        data: Vec<u8>,
        index: u32,
    },
    Type1 {
        font: Box<Type1Font>,
        names: Option<Vec<String>>,
        by_name: HashMap<String, GlyphId>,
    },
}

struct MapEntry {
    pfb: String,
    enc: Option<String>,
}

#[derive(Default)]
pub struct FontCache {
    faces: HashMap<String, Option<Rc<Face>>>,
    outlines: HashMap<(String, u32), Option<Rc<Path>>>,
    map: Option<HashMap<String, MapEntry>>,
    pub missing: BTreeSet<String>,
}

struct EmPen {
    builder: PathBuilder,
    scale: f32,
}

impl OutlinePen for EmPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.builder.move_to(x * self.scale, y * self.scale);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.builder.line_to(x * self.scale, y * self.scale);
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        let s = self.scale;
        self.builder.quad_to(cx * s, cy * s, x * s, y * s);
    }
    fn curve_to(&mut self, a: f32, b: f32, c: f32, d: f32, x: f32, y: f32) {
        let s = self.scale;
        self.builder
            .cubic_to(a * s, b * s, c * s, d * s, x * s, y * s);
    }
    fn close(&mut self) {
        self.builder.close();
    }
}

fn kpsewhich(name: &str) -> Option<String> {
    let out = Command::new("kpsewhich").arg(name).output().ok()?;
    let path = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!path.is_empty()).then_some(path)
}

/// `cmmi10 CMMI10 <cmmi10.pfb` and `ec-lmr10 LMRoman10-Regular "…" <cork.enc <lmr10.pfb`.
fn read_map() -> HashMap<String, MapEntry> {
    let mut entries = HashMap::new();
    let Some(text) = kpsewhich("pdftex.map").and_then(|p| std::fs::read_to_string(p).ok()) else {
        return entries;
    };
    for line in text
        .lines()
        .filter(|l| !l.starts_with(['%', '#', ' ']) && !l.is_empty())
    {
        let Some(tfm) = line.split_whitespace().next() else {
            continue;
        };
        let (mut pfb, mut enc) = (None, None);
        for token in line.split_whitespace() {
            let file = token.trim_start_matches(['<', '[']);
            if !token.starts_with('<') {
                continue;
            }
            if file.ends_with(".enc") {
                enc = Some(file.to_string());
            } else if file.ends_with(".pfb") || file.ends_with(".pfa") {
                pfb = Some(file.to_string());
            }
        }
        if let Some(pfb) = pfb {
            entries
                .entry(tfm.to_string())
                .or_insert(MapEntry { pfb, enc });
        }
    }
    entries
}

/// Glyph names of a PostScript encoding vector file: `/Name [ /a /b … ] def`.
fn read_encoding(path: &str) -> Option<Vec<String>> {
    let text = std::fs::read_to_string(path).ok()?;
    let body: String = text
        .lines()
        .map(|l| l.split('%').next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join(" ");
    let open = body.find('[')?;
    let close = body[open..].find(']')? + open;
    Some(
        body[open + 1..close]
            .split_whitespace()
            .map(|n| n.trim_start_matches('/').to_string())
            .collect(),
    )
}

impl FontCache {
    fn key(font: &Font) -> String {
        match &font.filename {
            Some(file) => format!("{file}#{}", font.subfont),
            None => format!("tfm:{}", font.name),
        }
    }

    fn load(&mut self, font: &Font) -> Option<Rc<Face>> {
        let key = Self::key(font);
        if let Some(face) = self.faces.get(&key) {
            return face.clone();
        }
        let face = match &font.filename {
            Some(file) => std::fs::read(file).ok().map(|data| Face::Open {
                data,
                index: font.subfont.saturating_sub(1),
            }),
            None => self.load_type1(&font.name),
        }
        .map(Rc::new);
        if face.is_none() {
            self.missing.insert(key.clone());
        }
        self.faces.insert(key, face.clone());
        face
    }

    fn load_type1(&mut self, tfm: &str) -> Option<Face> {
        let map = self.map.get_or_insert_with(read_map);
        let entry = map.get(tfm)?;
        let data = std::fs::read(kpsewhich(&entry.pfb)?).ok()?;
        let font = Type1Font::new(&data).ok()?;
        let names = entry
            .enc
            .as_deref()
            .and_then(kpsewhich)
            .and_then(|p| read_encoding(&p));
        let by_name = font
            .glyph_names()
            .map(|(gid, name)| (name.to_string(), gid))
            .collect();
        Some(Face::Type1 {
            font: Box::new(font),
            names,
            by_name,
        })
    }

    /// The outline of one display-list glyph, in em units with y up.
    pub fn glyph(&mut self, font: &Font, char_code: u32, index: Option<u32>) -> Glyph {
        let Some(face) = self.load(font) else {
            return Glyph::Missing;
        };
        let id = index.unwrap_or(char_code);
        let key = (Self::key(font), id);
        if let Some(path) = self.outlines.get(&key) {
            return path.clone().map_or(Glyph::Blank, Glyph::Outline);
        }
        let outline = match face.as_ref() {
            Face::Open {
                data,
                index: face_index,
            } => match index {
                Some(gid) => open_outline(data, *face_index, gid),
                None => None,
            },
            Face::Type1 {
                font,
                names,
                by_name,
            } => {
                let gid = match names {
                    Some(names) => names
                        .get(char_code as usize)
                        .and_then(|n| by_name.get(n))
                        .copied(),
                    None => font.encoding().and_then(|e| e.map(char_code as u8)),
                };
                gid.and_then(|gid| type1_outline(font, gid))
            }
        };
        match outline {
            None => Glyph::Missing,
            Some(path) => {
                let path = path.map(Rc::new);
                self.outlines.insert(key, path.clone());
                path.map_or(Glyph::Blank, Glyph::Outline)
            }
        }
    }
}

/// `None`: unresolvable; `Some(None)`: resolved but blank.
fn open_outline(data: &[u8], index: u32, gid: u32) -> Option<Option<Path>> {
    let font = FontRef::from_index(data, index).ok()?;
    let upem = font
        .metrics(Size::unscaled(), LocationRef::default())
        .units_per_em as f32;
    let glyph = font.outline_glyphs().get(GlyphId::new(gid))?;
    let mut pen = EmPen {
        builder: PathBuilder::new(),
        scale: 1.0 / upem,
    };
    glyph
        .draw(
            DrawSettings::unhinted(Size::unscaled(), LocationRef::default()),
            &mut pen,
        )
        .ok()?;
    Some(pen.builder.finish())
}

fn type1_outline(font: &Type1Font, gid: GlyphId) -> Option<Option<Path>> {
    let mut pen = EmPen {
        builder: PathBuilder::new(),
        scale: 1.0 / font.upem() as f32,
    };
    font.draw(gid, None, &mut pen).ok()?;
    Some(pen.builder.finish())
}
