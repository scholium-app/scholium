//! Grayscale images (ink coverage: 0 = white, 255 = black), PGM I/O and the tolerant ink
//! comparison used by upstream `rtex verify --raster`: an ink pixel is matched when the other
//! image has ink within one pixel; at most 1 % of all ink may be unmatched.

use serde::Serialize;
use std::io::Write;
use std::path::Path;

/// Coverage at or above this value counts as ink (upstream uses the same threshold).
pub const INK_THRESHOLD: u8 = 96;

pub struct Gray {
    pub width: usize,
    pub height: usize,
    pub ink: Vec<u8>,
}

impl Gray {
    fn has_ink(&self, x: i64, y: i64) -> bool {
        x >= 0
            && y >= 0
            && (x as usize) < self.width
            && (y as usize) < self.height
            && self.ink[y as usize * self.width + x as usize] >= INK_THRESHOLD
    }

    fn ink_near(&self, x: i64, y: i64) -> bool {
        (-1..=1).any(|dy| (-1..=1).any(|dx| self.has_ink(x + dx, y + dy)))
    }
}

/// Reads binary PGM (`P5`, as written by `pdftoppm -gray`) into ink coverage.
pub fn read_pgm(path: &Path) -> Result<Gray, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut fields = Vec::new();
    let mut pos = 0;
    while fields.len() < 4 {
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if bytes.get(pos) == Some(&b'#') {
            while pos < bytes.len() && bytes[pos] != b'\n' {
                pos += 1;
            }
            continue;
        }
        let start = pos;
        while pos < bytes.len() && !bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        fields.push(String::from_utf8_lossy(&bytes[start..pos]).to_string());
    }
    let parse = |s: &str| s.parse::<usize>().map_err(|e| format!("pgm header: {e}"));
    if fields[0] != "P5" || parse(&fields[3])? != 255 {
        return Err(format!("unsupported pgm header {fields:?}"));
    }
    let (width, height) = (parse(&fields[1])?, parse(&fields[2])?);
    let data = bytes
        .get(pos + 1..pos + 1 + width * height)
        .ok_or("pgm truncated")?;
    Ok(Gray {
        width,
        height,
        ink: data.iter().map(|v| 255 - v).collect(),
    })
}

pub fn write_pgm(path: &Path, gray: &Gray) -> std::io::Result<()> {
    let mut file = std::io::BufWriter::new(std::fs::File::create(path)?);
    write!(file, "P5\n{} {}\n255\n", gray.width, gray.height)?;
    file.write_all(&gray.ink.iter().map(|v| 255 - v).collect::<Vec<u8>>())
}

#[derive(Debug, Serialize)]
pub struct Diff {
    pub size_a: (usize, usize),
    pub size_b: (usize, usize),
    pub ink_a: usize,
    pub ink_b: usize,
    pub unmatched_a: usize,
    pub unmatched_b: usize,
    pub unmatched_fraction: f64,
    /// Bounding box (x0, y0, x1, y1) of unmatched ink, if any.
    pub unmatched_bbox: Option<(usize, usize, usize, usize)>,
}

/// Compares over the union of both sizes; ink outside the other image is unmatched.
pub fn compare(a: &Gray, b: &Gray) -> Diff {
    let (w, h) = (a.width.max(b.width), a.height.max(b.height));
    let (mut ink_a, mut ink_b, mut ua, mut ub) = (0, 0, 0, 0);
    let mut bbox: Option<(usize, usize, usize, usize)> = None;
    let mut grow = |x: usize, y: usize| {
        bbox = Some(match bbox {
            None => (x, y, x, y),
            Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
        });
    };
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            if a.has_ink(x, y) {
                ink_a += 1;
                if !b.ink_near(x, y) {
                    ua += 1;
                    grow(x as usize, y as usize);
                }
            }
            if b.has_ink(x, y) {
                ink_b += 1;
                if !a.ink_near(x, y) {
                    ub += 1;
                    grow(x as usize, y as usize);
                }
            }
        }
    }
    Diff {
        size_a: (a.width, a.height),
        size_b: (b.width, b.height),
        ink_a,
        ink_b,
        unmatched_a: ua,
        unmatched_b: ub,
        unmatched_fraction: (ua + ub) as f64 / (ink_a + ink_b).max(1) as f64,
        unmatched_bbox: bbox,
    }
}
