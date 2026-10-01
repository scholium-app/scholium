//! Editor metadata follows the actual shaped advances, including justification.

use typst_library::editor::{EditCaret, EditCluster};
use typst_library::layout::{Abs, Frame, Point, Size};
use typst_library::text::Glyph;
use unicode_segmentation::UnicodeSegmentation;

use super::{ShapedGlyph, ShapedText, SpanMapper};

impl ShapedText<'_> {
    pub(super) fn build_edit_clusters(
        &self,
        frame: &mut Frame,
        spans: &SpanMapper,
        shaped: &[ShapedGlyph],
        glyphs: &[Glyph],
        pos: Point,
    ) {
        let mut cursor = pos.x;
        let mut start = 0;
        while start < shaped.len() {
            let range = &shaped[start].range;
            let mut end = start + 1;
            while end < shaped.len() && shaped[end].range == *range {
                end += 1;
            }
            let width: Abs = glyphs[start..end]
                .iter()
                .map(|glyph| glyph.x_advance.at(shaped[start].size))
                .sum();
            if let Some((origin, bytes)) = spans.edit_at(range)
                && let Some(text) = range
                    .start
                    .checked_sub(self.base)
                    .zip(range.end.checked_sub(self.base))
                    .and_then(|(a, b)| self.text.get(a..b))
            {
                let rtl = !self.dir.is_positive();
                let carets = caret_stops(text, bytes.start, width, rtl);
                frame.attach_edit_cluster(EditCluster {
                    origin,
                    range: bytes,
                    position: Point::new(cursor, pos.y - frame.baseline()),
                    size: Size::new(width, frame.height()),
                    baseline: pos.y,
                    rtl,
                    carets,
                });
            }
            cursor += width;
            start = end;
        }
    }
}

// UTF-8 byte stops follow graphemes. Interior ligature stops use a declared
// proportional fallback; only the two shaped cluster edges are exact.
fn caret_stops(text: &str, base: usize, width: Abs, rtl: bool) -> Vec<EditCaret> {
    let offsets: Vec<_> = text
        .grapheme_indices(true)
        .map(|(byte, _)| byte)
        .chain(std::iter::once(text.len()))
        .collect();
    let intervals = offsets.len().saturating_sub(1).max(1);
    offsets
        .iter()
        .enumerate()
        .map(|(index, byte)| {
            let fraction = index as f64 / intervals as f64;
            EditCaret {
                byte: base + byte,
                offset: width * if rtl { 1.0 - fraction } else { fraction },
                exact: index == 0 || index == intervals,
            }
        })
        .collect()
}
