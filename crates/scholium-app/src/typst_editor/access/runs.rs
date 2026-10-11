//! TextRuns built from the authoritative snapshot and, when current, its Scene carets.
//!
//! Geometry is kept in padded Frame pt so scrolling or zoom never invalidates the cache;
//! the page placement is applied only when publishing.
use super::*;
use std::collections::HashMap;

/// One TextRun: `leaf[start..end]`, plus `\n` when it ends a paragraph.
#[derive(Debug, Clone)]
pub(super) struct Run {
    pub(super) id: egui::Id,
    pub(super) block: NodeId,
    /// A placeholder for a block without body text points at the block's first leaf.
    pub(super) leaf: Option<NodeId>,
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) text: String,
    pub(super) geometry: Option<Geometry>,
}

/// Frame pt: run bounds, per-char x offsets relative to `rect.min.x`, per-char widths.
#[derive(Debug, Clone)]
pub(super) struct Geometry {
    rect: egui::Rect,
    positions: Vec<f32>,
    widths: Vec<f32>,
}

impl Run {
    pub(super) fn publish(&self, ui: &mut egui::Ui, placement: Option<(egui::Rect, f32)>) {
        let screen = self
            .geometry
            .as_ref()
            .zip(placement)
            .map(|(g, (page, scale))| {
                let rect = egui::Rect::from_min_max(
                    page.min + g.rect.min.to_vec2() * scale,
                    page.min + g.rect.max.to_vec2() * scale,
                );
                let scaled = |v: &[f32]| v.iter().map(|x| x * scale).collect::<Vec<_>>();
                (rect, scaled(&g.positions), scaled(&g.widths))
            });
        // A child Ui registers the accessibility parent; it draws nothing.
        let mut builder = egui::UiBuilder::new()
            .id(self.id)
            .accessibility_parent(body_id());
        if let Some((rect, ..)) = &screen {
            builder = builder.max_rect(*rect);
        }
        drop(ui.new_child(builder));
        ui.ctx().accesskit_node_builder(self.id, |node| {
            node.set_role(Role::TextRun);
            node.set_text_direction(accesskit::TextDirection::LeftToRight);
            node.set_value(self.text.clone());
            node.set_character_lengths(
                self.text
                    .chars()
                    .map(|c| c.len_utf8() as u8)
                    .collect::<Vec<_>>(),
            );
            if let Some((rect, positions, widths)) = screen {
                // Screen logical px, the same transform as the painted caret.
                node.set_bounds(accesskit::Rect {
                    x0: rect.min.x.into(),
                    y0: rect.min.y.into(),
                    x1: rect.max.x.into(),
                    y1: rect.max.y.into(),
                });
                node.set_character_positions(positions);
                node.set_character_widths(widths);
            }
        });
    }

    fn body(&self) -> &str {
        self.text.strip_suffix('\n').unwrap_or(&self.text)
    }

    pub(super) fn encode(&self, leaf: NodeId, byte: usize) -> Option<TextPosition> {
        if self.leaf != Some(leaf) || byte < self.start || byte > self.end {
            return None;
        }
        Some(TextPosition {
            node: self.id.accesskit_id(),
            character_index: self.body().get(..byte - self.start)?.chars().count(),
        })
    }

    /// Leaf, UTF-8 byte and whether the position ends this visual line (Upstream).
    pub(super) fn decode(&self, position: &TextPosition) -> Option<(NodeId, usize, bool)> {
        if position.node != self.id.accesskit_id() {
            return None;
        }
        let body = self.body();
        // An index on the trailing `\n` (or past it) is the paragraph end.
        let offset = body
            .char_indices()
            .map(|(byte, _)| byte)
            .nth(position.character_index)
            .unwrap_or(body.len());
        let line_end = offset == body.len() && !body.is_empty();
        Some((self.leaf?, self.start + offset, line_end))
    }
}

/// Visual-line segments `[start, end)` of one leaf from its exact carets, top to bottom.
///
/// Carets on one line overlap vertically even when fonts differ (CJK fallback glyphs sit
/// at another top than Latin ones); wrapped lines never overlap. Same rule as `vertical`.
/// The first segment always starts at byte 0 so no text is left outside a run.
fn segments(carets: &[&Caret], len: usize) -> Vec<(usize, usize, Vec<Caret>)> {
    let span = |c: &Caret| (c.top[1].min(c.bottom[1]), c.top[1].max(c.bottom[1]));
    let mut lines: Vec<((f64, f64), Vec<Caret>)> = Vec::new();
    for caret in carets {
        let (top, bottom) = span(caret);
        match lines.iter_mut().find(|((t, b), _)| top < *b && *t < bottom) {
            Some((band, stops)) => {
                *band = (band.0.min(top), band.1.max(bottom));
                stops.push((*caret).clone());
            }
            None => lines.push(((top, bottom), vec![(*caret).clone()])),
        }
    }
    lines.sort_by(|a, b| a.0.0.total_cmp(&b.0.0));
    let mut starts: Vec<usize> = lines
        .iter()
        .map(|(_, s)| s.iter().map(|c| c.position.byte).min().unwrap_or(0))
        .collect();
    if let Some(first) = starts.first_mut() {
        *first = 0;
    }
    lines
        .into_iter()
        .enumerate()
        .map(|(i, (_, mut stops))| {
            stops.sort_by_key(|c| c.position.byte);
            (starts[i], starts.get(i + 1).copied().unwrap_or(len), stops)
        })
        .collect()
}

/// Run geometry in Frame pt; chars inside a cluster share its stop. `None` when any char
/// lacks a stop at or before it, so a run never carries partial geometry.
fn geometry(text: &str, start: usize, end: usize, stops: &[Caret]) -> Option<Geometry> {
    let x_at = |byte: usize| {
        stops
            .iter()
            .rfind(|c| c.position.byte <= byte)
            .map(|c| c.top[0])
    };
    let left = text
        .get(start..end)?
        .char_indices()
        .map(|(i, _)| x_at(start + i))
        .collect::<Option<Vec<f64>>>()?;
    let right_end = x_at(end)?;
    let widths: Vec<f64> = (0..left.len())
        .map(|i| (left.get(i + 1).copied().unwrap_or(right_end) - left[i]).max(0.0))
        .collect();
    let first = stops.first()?;
    let x0 = left.first().copied().unwrap_or(first.top[0]);
    let x1 = left.last().zip(widths.last()).map_or(x0, |(l, w)| l + w);
    Some(Geometry {
        rect: egui::Rect::from_min_max(
            egui::pos2(x0 as f32, first.top[1] as f32),
            egui::pos2(x1 as f32, first.bottom[1] as f32),
        ),
        positions: left.iter().map(|x| (x - x0) as f32).collect(),
        widths: widths.iter().map(|w| *w as f32).collect(),
    })
}

/// All runs in document order; every block contributes at least one run.
pub(super) fn build(
    snapshot: &StructuredDocument,
    scene: Option<&scholium_typst::editor::EditorScene>,
) -> Vec<Run> {
    // One pass over the Scene; per-leaf lookups stay linear in document size.
    let mut carets: HashMap<NodeId, Vec<&Caret>> = HashMap::new();
    for caret in scene
        .iter()
        .flat_map(|s| &s.geometry.carets)
        .filter(|c| c.exact)
    {
        carets.entry(caret.position.leaf).or_default().push(caret);
    }
    let leaves = input::leaves(snapshot);
    let mut runs = Vec::new();
    for (index, block) in snapshot.blocks.iter().enumerate() {
        let first = runs.len();
        for inline in &block.content {
            let InlineBody::Text { text, .. } = &inline.body else {
                continue;
            };
            // Empty leaves beside other inline content shape nothing; skip them.
            if text.is_empty() && block.content.len() > 1 {
                continue;
            }
            runs.extend(leaf_runs(
                block.node,
                inline.node,
                text,
                carets.get(&inline.node),
            ));
        }
        if runs.len() == first {
            runs.push(Run {
                id: body_id().with(("block", block.node)),
                block: block.node,
                leaf: leaves.iter().find(|l| l.block == block.node).map(|l| l.id),
                start: 0,
                end: 0,
                text: String::new(),
                geometry: None,
            });
        }
        if index + 1 < snapshot.blocks.len()
            && let Some(last) = runs.last_mut()
        {
            last.text.push('\n');
            if let Some(g) = &mut last.geometry {
                g.positions.push(
                    g.positions
                        .last()
                        .zip(g.widths.last())
                        .map_or(0.0, |(p, w)| p + w),
                );
                g.widths.push(0.0);
            }
        }
    }
    runs
}

fn leaf_runs(block: NodeId, leaf: NodeId, text: &str, carets: Option<&Vec<&Caret>>) -> Vec<Run> {
    let plain = |start: usize, end: usize| Run {
        id: body_id().with((leaf, start)),
        block,
        leaf: Some(leaf),
        start,
        end,
        text: text.get(start..end).unwrap_or_default().to_owned(),
        geometry: None,
    };
    let Some(carets) = carets.filter(|c| !c.is_empty()) else {
        return vec![plain(0, text.len())];
    };
    segments(carets, text.len())
        .into_iter()
        .map(|(start, end, stops)| Run {
            geometry: geometry(text, start, end, &stops),
            ..plain(start, end)
        })
        .collect()
}
