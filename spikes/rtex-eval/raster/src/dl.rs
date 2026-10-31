//! The JSON form of rtex display lists (upstream `docs/display-list.md`, format revision 1).
//!
//! Only what a host needs to paint is decoded: fonts, rows and their items. Lengths stay in
//! TeX scaled points (65536 sp = 1 pt); y grows downward.

use serde_json::Value;
use std::collections::HashMap;

/// Scaled points per PostScript point (bp).
pub const SP_PER_BP: f64 = 65536.0 * 72.27 / 72.0;

#[derive(Debug, Clone)]
pub struct Font {
    /// Font file for OpenType/TrueType fonts; `None` for TFM fonts (resolved by map file).
    pub filename: Option<String>,
    /// TFM name (`cmmi10`) or luaotfload specification.
    pub name: String,
    pub size_sp: f64,
    /// 1-based face index inside a collection; 0 or 1 select the first face.
    pub subfont: u32,
    /// Per-mille slant; 0 = upright.
    pub slant: f64,
    /// Per-mille horizontal extension; 0 or 1000 = unchanged.
    pub extend: f64,
}

#[derive(Debug, Clone)]
pub enum Item {
    Glyph {
        font: i64,
        char_code: u32,
        index: Option<u32>,
        x: i64,
        y: i64,
        expansion: i64,
    },
    Rule {
        x: i64,
        y_top: i64,
        width: i64,
        height: i64,
    },
    /// Colour stack operation: 1 push, 2 pop, anything else sets the top.
    Color {
        cmd: Option<i64>,
        data: String,
    },
    Image,
    /// `l` literals and `u` unsupported records: the list is degraded.
    Degraded,
    Other,
}

#[derive(Debug, Clone)]
pub struct Line {
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
    pub d: i64,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone)]
pub struct DisplayList {
    pub fonts: HashMap<i64, Font>,
    pub lines: Vec<Line>,
    pub other: Vec<Item>,
    pub page_width: Option<i64>,
    pub page_height: Option<i64>,
    pub width: i64,
    pub degraded: bool,
}

fn int(value: Option<&Value>) -> i64 {
    value
        .and_then(|v| v.as_f64())
        .map(|v| v as i64)
        .unwrap_or(0)
}

fn float(value: Option<&Value>) -> f64 {
    value.and_then(|v| v.as_f64()).unwrap_or(0.0)
}

fn item(value: &Value) -> Item {
    let Some(a) = value.as_array() else {
        return Item::Other;
    };
    match a.first().and_then(|t| t.as_str()) {
        Some("g") => Item::Glyph {
            font: int(a.get(1)),
            char_code: int(a.get(2)) as u32,
            index: a.get(3).and_then(|v| v.as_u64()).map(|v| v as u32),
            x: int(a.get(4)),
            y: int(a.get(5)),
            expansion: int(a.get(7)),
        },
        Some("r") => Item::Rule {
            x: int(a.get(1)),
            y_top: int(a.get(2)),
            width: int(a.get(3)),
            height: int(a.get(4)),
        },
        Some("c") => Item::Color {
            cmd: a.get(2).and_then(|v| v.as_i64()),
            data: a.get(3).and_then(|v| v.as_str()).unwrap_or("").to_string(),
        },
        Some("i") => Item::Image,
        Some("l") | Some("u") => Item::Degraded,
        _ => Item::Other,
    }
}

fn font(value: &Value) -> Font {
    let text = |key: &str| value.get(key).and_then(|v| v.as_str()).map(str::to_string);
    Font {
        filename: text("filename"),
        name: text("name").unwrap_or_default(),
        size_sp: value
            .get("size")
            .and_then(|v| v.as_f64())
            .unwrap_or(655360.0),
        subfont: int(value.get("subfont")).max(0) as u32,
        slant: float(value.get("slant")),
        extend: float(value.get("extend")),
    }
}

impl DisplayList {
    pub fn from_value(value: &Value) -> DisplayList {
        let mut fonts = HashMap::new();
        // An empty fonts table arrives as `[]` from the Lua encoder.
        if let Some(map) = value.get("fonts").and_then(|f| f.as_object()) {
            for (key, spec) in map {
                if let Ok(id) = key.parse::<i64>() {
                    fonts.insert(id, font(spec));
                }
            }
        }
        let items = |v: Option<&Value>| -> Vec<Item> {
            v.and_then(|v| v.as_array())
                .map(|a| a.iter().map(item).collect())
                .unwrap_or_default()
        };
        let lines: Vec<Line> = value
            .get("lines")
            .and_then(|l| l.as_array())
            .map(|lines| {
                lines
                    .iter()
                    .map(|l| Line {
                        x: int(l.get("x")),
                        y: int(l.get("y")),
                        w: int(l.get("w")),
                        h: int(l.get("h")),
                        d: int(l.get("d")),
                        items: items(l.get("items")),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let other = items(value.get("other"));
        let flagged = match value.get("flags") {
            Some(Value::Object(map)) => !map.is_empty(),
            Some(Value::Array(list)) => !list.is_empty(),
            _ => false,
        };
        let degraded = flagged
            || lines
                .iter()
                .flat_map(|l| &l.items)
                .chain(&other)
                .any(|i| matches!(i, Item::Degraded));
        DisplayList {
            fonts,
            lines,
            other,
            page_width: value.get("page_width").and_then(|v| v.as_i64()),
            page_height: value.get("page_height").and_then(|v| v.as_i64()),
            width: int(value.get("width")),
            degraded,
        }
    }
}
