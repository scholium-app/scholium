//! Native font geometry in document-local logical pixels, shared by paint and input.
use super::*;
use scholium_spike_core::layout::SourceSpan;

pub(super) struct TextGeometry {
    pub source: Option<SourceSpan>,
    pub origin: egui::Pos2,
    pub galley: Arc<egui::Galley>,
    pub offsets: Vec<usize>,
}

impl TextGeometry {
    pub fn caret(&self, byte: usize) -> Option<egui::Rect> {
        let local = byte.checked_sub(self.source?.start_byte)?;
        let index = self.offsets.binary_search(&local).ok()?;
        Some(
            self.galley
                .pos_from_cursor(egui::text::CCursor::new(index))
                .translate(self.origin.to_vec2()),
        )
    }

    pub fn bounds(&self) -> egui::Rect {
        self.galley.rect.translate(self.origin.to_vec2())
    }
}

impl SpikeApp {
    pub(super) fn refresh_text_geometry(&mut self, ui: &egui::Ui) {
        let key = (self.core.revision(), ui.ctx().pixels_per_point());
        if self.text_geometry_key == Some(key) {
            return;
        }
        self.text_geometry = self
            .layout
            .items
            .iter()
            .filter_map(|item| {
                let Item::Text {
                    x,
                    baseline,
                    size,
                    content,
                    source,
                } = item
                else {
                    return None;
                };
                let galley = ui.painter().layout_no_wrap(
                    content.clone(),
                    egui::FontId::proportional(*size),
                    egui::Color32::PLACEHOLDER,
                );
                let font_baseline = galley
                    .rows
                    .first()
                    .and_then(|row| row.glyphs.first().map(|glyph| row.pos.y + glyph.pos.y))
                    .unwrap_or(0.0);
                Some(TextGeometry {
                    source: *source,
                    origin: egui::pos2(*x, *baseline - font_baseline),
                    galley,
                    offsets: content
                        .char_indices()
                        .map(|(byte, _)| byte)
                        .chain(std::iter::once(content.len()))
                        .collect(),
                })
            })
            .collect();
        self.text_geometry_index = self
            .text_geometry
            .iter()
            .enumerate()
            .filter_map(|(index, run)| Some((run.source?.node, index)))
            .collect();
        self.text_geometry_key = Some(key);
        self.accessible_cache = None;
        for run in &self.text_geometry {
            self.layout.width = self.layout.width.max(run.bounds().right());
            self.layout.height = self.layout.height.max(run.bounds().bottom());
        }
    }

    pub(super) fn text_caret(&self, node: NodeId, byte: usize) -> Option<egui::Rect> {
        self.text_geometry
            .get(*self.text_geometry_index.get(&node)?)?
            .caret(byte)
    }

    pub(super) fn text_hit(&self, position: egui::Vec2) -> Option<(NodeId, usize)> {
        let mut best: Option<(f32, NodeId, usize)> = None;
        for run in &self.text_geometry {
            let Some(source) = run.source else { continue };
            if position.y < run.bounds().top() || position.y > run.bounds().bottom() {
                continue;
            }
            let Ok(node) = self.core.document().node(source.node) else {
                continue;
            };
            for offset in &run.offsets {
                let byte = source.start_byte + offset;
                if !node.text.is_grapheme_boundary(byte) {
                    continue;
                }
                let Some(rect) = run.caret(byte) else {
                    continue;
                };
                if position.y < rect.top() || position.y > rect.bottom() {
                    continue;
                }
                let distance = (position.x - rect.left()).abs();
                if best.is_none_or(|(old, _, _)| distance < old) {
                    best = Some((distance, source.node, byte));
                }
            }
        }
        best.map(|(_, node, byte)| (node, byte))
    }
}
