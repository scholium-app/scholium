//! Read-only generated Typst source beside an A4 quick preview of the same snapshot.
//!
//! Both are projections of the authoritative session; nothing here writes the document.
//! The preview worker starts on first use and keys results by a monotonic generation,
//! because undo can return to an earlier revision in a new epoch.
use super::*;
use scholium_model::NodeId;
use scholium_typst::{
    CompileOutcome, GeneratedSource, PreviewCompiler, PreviewEvent, SourceMode, generate_source,
};

/// A4 in Typst pt, the preview page size the display-resolution request is derived from.
const A4_PT: [f64; 2] = [595.276, 841.89];
/// Request scale before the pane has been placed once.
const FALLBACK_PX_PER_PT: f64 = 2.0;
/// Compiler's accepted range of px per pt.
const PX_PER_PT_RANGE: (f64, f64) = (0.25, 8.0);

/// Bitmap px per pt that shows an A4 page `width` logical px wide 1:1 on a display of `ppp`
/// pixels per point whose textures are at most `max_side` px; lowered, never failing, when the
/// page would not fit one texture.
fn preview_px_per_pt(width: f32, ppp: f32, max_side: u32) -> f64 {
    let fit = raster::fit(A4_PT[0], width / A4_PT[0] as f32, ppp);
    let side = f64::from(max_side.min(scholium_typst::editor::MAX_STRIP_PX));
    fit.px_per_pt
        .min(side / A4_PT[1])
        .clamp(PX_PER_PT_RANGE.0, PX_PER_PT_RANGE.1)
}
/// Preview pane share of the source workspace, per P1_UI's roughly equal split.
const PREVIEW_SHARE: f32 = 0.5;

#[derive(Default)]
pub(super) struct SourceView {
    compiler: Option<PreviewCompiler>,
    generation: u64,
    submitted: Option<Arc<StructuredDocument>>,
    view: Option<(Arc<StructuredDocument>, Arc<GeneratedSource>)>,
    /// Laid-out source for (snapshot, highlighted range); avoids re-hashing it every frame.
    galley: Option<SourceGalley>,
    /// Block ids in preview order for the submitted generation.
    blocks: Vec<NodeId>,
    compiled: Option<CompileOutcome>,
    page: usize,
    texture: Option<Page>,
    /// Screen rect of the shown page image and its logical px per Typst pt (for audits).
    shown_at: Option<(egui::Rect, f32)>,
    /// Bitmap px per pt that shows the page 1:1 in the pane; page requests use it.
    px_per_pt: f64,
    settle: raster::Settle,
}

/// Laid-out source keyed by snapshot identity and highlighted byte range.
type SourceGalley = (
    Arc<StructuredDocument>,
    Option<std::ops::Range<usize>>,
    Arc<egui::Galley>,
);

/// One displayed preview page; `size_pt` is the page size in Typst pt.
struct Page {
    generation: u64,
    index: usize,
    size_pt: egui::Vec2,
    /// Bitmap size in px and the px per pt it was rasterized at.
    px: [u32; 2],
    px_per_pt: f64,
    texture: egui::TextureHandle,
}

impl std::fmt::Debug for SourceView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceView")
            .field("generation", &self.generation)
            .field("page", &self.page)
            .finish_non_exhaustive()
    }
}

impl SourceView {
    /// Generated view source of `snapshot`, regenerated only when the snapshot changes.
    pub(super) fn source(&mut self, snapshot: &Arc<StructuredDocument>) -> Arc<GeneratedSource> {
        match &self.view {
            Some((s, generated)) if Arc::ptr_eq(s, snapshot) => generated.clone(),
            _ => {
                let generated = Arc::new(generate_source(snapshot, SourceMode::View));
                self.view = Some((snapshot.clone(), generated.clone()));
                generated
            }
        }
    }

    /// Source galley with the cursor block highlighted; `true` when the highlight moved.
    fn galley(
        &mut self,
        ui: &egui::Ui,
        snapshot: &Arc<StructuredDocument>,
        highlight: Option<std::ops::Range<usize>>,
    ) -> (Arc<egui::Galley>, bool) {
        if let Some((s, h, galley)) = &self.galley
            && Arc::ptr_eq(s, snapshot)
            && *h == highlight
        {
            return (galley.clone(), false);
        }
        let generated = self.source(snapshot);
        let job = source_job(ui, &generated.source, highlight.clone());
        let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
        self.galley = Some((snapshot.clone(), highlight, galley.clone()));
        (galley, true)
    }

    /// Submit a changed snapshot and take finished results; `focus_block` picks the page.
    pub(super) fn sync(
        &mut self,
        ctx: &egui::Context,
        snapshot: &Arc<StructuredDocument>,
        focus_block: Option<NodeId>,
    ) {
        let compiler = self.compiler.get_or_insert_with(|| {
            let wake = ctx.clone();
            PreviewCompiler::spawn_with_wake(move || wake.request_repaint())
        });
        if !self
            .submitted
            .as_ref()
            .is_some_and(|s| Arc::ptr_eq(s, snapshot))
        {
            self.generation += 1;
            let generated = generate_source(snapshot, SourceMode::Preview);
            self.blocks = generated.blocks.iter().map(|b| b.node).collect();
            compiler.submit(
                snapshot.document,
                self.generation,
                generated.source,
                self.blocks.len(),
            );
            self.submitted = Some(snapshot.clone());
        }
        while let Some(event) = self.compiler.as_mut().and_then(PreviewCompiler::poll) {
            self.accept(ctx, snapshot.document, event, focus_block);
        }
    }

    fn accept(
        &mut self,
        ctx: &egui::Context,
        document: scholium_model::DocumentId,
        event: PreviewEvent,
        focus_block: Option<NodeId>,
    ) {
        match event {
            PreviewEvent::Compiled(outcome) => {
                if (outcome.document, outcome.revision) != (document, self.generation) {
                    return;
                }
                let focus = focus_block
                    .and_then(|b| self.blocks.iter().position(|n| *n == b))
                    .and_then(|ordinal| outcome.anchors.iter().find(|a| a.block == ordinal))
                    .map(|a| a.start_page.saturating_sub(1));
                self.page = focus
                    .unwrap_or(self.page)
                    .min(outcome.page_count.saturating_sub(1));
                let ok = outcome.error.is_none() && outcome.page_count > 0;
                self.compiled = Some(outcome);
                if ok {
                    self.request_page(document);
                }
            }
            PreviewEvent::Page(page) => {
                if (page.document, page.revision, page.page)
                    != (document, self.generation, self.page)
                {
                    return;
                }
                let size = [page.pixels.width as usize, page.pixels.height as usize];
                let image = egui::ColorImage::from_rgba_premultiplied(size, &page.pixels.rgba);
                self.settle.arrived();
                self.texture = Some(Page {
                    generation: self.generation,
                    index: page.page,
                    size_pt: egui::vec2(size[0] as f32, size[1] as f32) / page.pixel_per_pt,
                    px: [page.pixels.width, page.pixels.height],
                    px_per_pt: f64::from(page.pixel_per_pt),
                    texture: ctx.load_texture(
                        "typst-preview-page",
                        image,
                        egui::TextureOptions::LINEAR,
                    ),
                });
            }
        }
    }

    fn request_page(&mut self, document: scholium_model::DocumentId) {
        let px_per_pt = if self.px_per_pt > 0.0 {
            self.px_per_pt
        } else {
            FALLBACK_PX_PER_PT
        };
        if let Some(compiler) = &self.compiler {
            compiler.request_page(document, self.generation, self.page, px_per_pt as f32);
            self.settle.requested(px_per_pt);
        }
    }

    /// Follow the pane's display resolution: re-request the shown page once it is stable.
    fn track_resolution(&mut self, ctx: &egui::Context, document: scholium_model::DocumentId) {
        let Some(shown) = self
            .texture
            .as_ref()
            .filter(|p| p.generation == self.generation)
        else {
            return;
        };
        let shown = shown.px_per_pt;
        match self
            .settle
            .step(std::time::Instant::now(), self.px_per_pt, Some(shown))
        {
            raster::Step::Idle => {}
            raster::Step::Wait(after) => ctx.request_repaint_after(after),
            raster::Step::Request => self.request_page(document),
        }
    }

    /// Block under a point of the shown page, in Typst pt from the page's top-left.
    fn locate(&self, y_pt: f32) -> Option<NodeId> {
        let compiled = self.compiled.as_ref()?;
        let page = self
            .texture
            .as_ref()
            .filter(|p| p.generation == self.generation)?;
        let ordinal = scholium_typst::block_at_click(&compiled.anchors, page.index + 1, y_pt)?;
        self.blocks.get(ordinal).copied()
    }

    /// Opt-in audit facts: shown page, its screen placement and the block anchor heights.
    pub(super) fn audit(&self) -> serde_json::Value {
        let current = self
            .texture
            .as_ref()
            .is_some_and(|p| p.generation == self.generation);
        serde_json::json!({
            "generation": self.generation,
            "page": self.page,
            "pages": self.compiled.as_ref().map(|c| c.page_count),
            "error": self.compiled.as_ref().and_then(|c| c.error.clone()),
            "shown": current,
            "rect": self.shown_at.map(|(r, _)| [r.min.x, r.min.y, r.max.x, r.max.y]),
            "scale": self.shown_at.map(|(_, s)| s),
            "px_per_pt": self.texture.as_ref().map(|p| p.px_per_pt),
            "wanted_px_per_pt": self.px_per_pt,
            "bitmap_px": self.texture.as_ref().map(|p| p.px),
            "anchors": self.compiled.as_ref().map(|c| c.anchors.iter()
                .map(|a| serde_json::json!({"page": a.page, "start_y": a.start_y, "y": a.y}))
                .collect::<Vec<_>>()),
        })
    }

    fn status(&self) -> String {
        match &self.compiled {
            None => "预览编译中".into(),
            Some(c) if c.revision != self.generation => "预览编译中".into(),
            Some(c) => match &c.error {
                Some(error) => format!("预览失败：{error}"),
                None => format!(
                    "A4 预览 · 第 {}/{} 页 · {} ms",
                    self.page + 1,
                    c.page_count,
                    c.elapsed_ms
                ),
            },
        }
    }
}

impl EditorSession {
    /// Source workspace: generated Typst (read-only) and its quick preview.
    pub(super) fn show_source(&mut self, ui: &mut egui::Ui) {
        let focus_block = self.cursor_block();
        let snapshot = self.snapshot.clone();
        self.source_view.sync(ui.ctx(), &snapshot, focus_block);
        let preview_width = ui.available_width() * PREVIEW_SHARE;
        egui::Panel::right("source-preview")
            .exact_size(preview_width)
            .show(ui, |ui| self.preview_pane(ui));
        egui::CentralPanel::default().show(ui, |ui| self.source_pane(ui, focus_block));
    }

    fn cursor_block(&self) -> Option<NodeId> {
        input::leaves(&self.snapshot)
            .iter()
            .find(|l| l.id == self.cursor.leaf)
            .map(|l| l.block)
    }

    fn source_pane(&mut self, ui: &mut egui::Ui, focus_block: Option<NodeId>) {
        let snapshot = self.snapshot.clone();
        let generated = self.source_view.source(&snapshot);
        ui.label(format!(
            "生成 Typst 源码 · 只读 · 正文 r{}",
            snapshot.revision.0
        ));
        if generated.holes > 0 || !generated.raw.is_empty() {
            ui.weak(format!(
                "{} 个空槽显示为 □，严格输出会拒绝；{} 个原串公式只显示、不在预览中执行。",
                generated.holes,
                generated.raw.len()
            ));
        }
        let highlight = focus_block
            .and_then(|b| generated.blocks.iter().find(|s| s.node == b))
            .map(|s| s.range.clone());
        let (galley, moved) = self.source_view.galley(ui, &snapshot, highlight.clone());
        egui::ScrollArea::both()
            .id_salt("generated-source")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let response = ui.add(egui::Label::new(galley.clone()));
                if moved && let Some(range) = highlight {
                    let chars = generated.source[..range.start].chars().count();
                    let row = galley.pos_from_cursor(egui::text::CCursor::new(chars));
                    ui.scroll_to_rect(
                        row.translate(response.rect.min.to_vec2()),
                        Some(egui::Align::Center),
                    );
                }
            });
    }

    fn preview_pane(&mut self, ui: &mut egui::Ui) {
        let document = self.snapshot.document;
        ui.horizontal(|ui| {
            let pages = self
                .source_view
                .compiled
                .as_ref()
                .map_or(0, |c| c.page_count);
            let page = self.source_view.page;
            if ui.add_enabled(page > 0, egui::Button::new("‹")).clicked() {
                self.source_view.page -= 1;
                self.source_view.request_page(document);
            }
            if ui
                .add_enabled(page + 1 < pages, egui::Button::new("›"))
                .clicked()
            {
                self.source_view.page += 1;
                self.source_view.request_page(document);
            }
            ui.label(self.source_view.status());
        });
        let ppp = ui.ctx().pixels_per_point();
        let max_side = ui.ctx().input(|i| i.max_texture_side) as u32;
        self.source_view.px_per_pt = preview_px_per_pt(ui.available_width(), ppp, max_side);
        self.source_view.track_resolution(ui.ctx(), document);
        // While a new generation compiles, keep the last page visible but inert.
        let Some(page) = self.source_view.texture.as_ref() else {
            return;
        };
        let current = page.generation == self.source_view.generation;
        // A raster made for this pane maps texel to physical pixel; otherwise stretch to width.
        let size = if (page.px_per_pt - self.source_view.px_per_pt).abs() < 1e-6 {
            egui::vec2(page.px[0] as f32, page.px[1] as f32) / ppp
        } else {
            page.size_pt * (ui.available_width() / page.size_pt.x)
        };
        let scale = size.x / page.size_pt.x;
        let texture = page.texture.id();
        let mut clicked = None;
        let mut shown_at = None;
        egui::ScrollArea::vertical()
            .id_salt("preview-page")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
                let drawn = egui::Rect::from_min_size(raster::snap_pos(rect.min, ppp), size);
                ui.painter().image(
                    texture,
                    drawn,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
                shown_at = Some((response.rect, scale));
                if current
                    && response.clicked()
                    && let Some(pos) = response.interact_pointer_pos()
                {
                    clicked = Some((pos.y - response.rect.top()) / scale);
                }
            });
        self.source_view.shown_at = shown_at;
        if let Some(block) = clicked.and_then(|y| self.source_view.locate(y)) {
            self.cursor_to_block(block);
        }
    }

    /// Put the caret at the start of a block's first body text, else its first math slot.
    fn cursor_to_block(&mut self, block: NodeId) {
        let leaves = input::leaves(&self.snapshot);
        let in_block = || leaves.iter().filter(|l| l.block == block);
        let Some(leaf) = in_block().find(|l| !l.math).or_else(|| in_block().next()) else {
            return;
        };
        self.move_to(
            Position {
                leaf: leaf.id,
                byte: 0,
                affinity: Affinity::Downstream,
            },
            false,
        );
        self.drag = None;
    }
}

fn source_job(
    ui: &egui::Ui,
    source: &str,
    highlight: Option<std::ops::Range<usize>>,
) -> egui::text::LayoutJob {
    let font = egui::FontId::monospace(13.0);
    let color = ui.visuals().text_color();
    let plain = egui::TextFormat::simple(font.clone(), color);
    let marked = egui::TextFormat {
        background: crate::theme::colors(ui).selection,
        ..egui::TextFormat::simple(font, color)
    };
    let mut job = egui::text::LayoutJob::default();
    match highlight.filter(|r| source.get(r.clone()).is_some()) {
        Some(range) => {
            job.append(&source[..range.start], 0.0, plain.clone());
            job.append(&source[range.clone()], 0.0, marked);
            job.append(&source[range.end..], 0.0, plain);
        }
        None => job.append(source, 0.0, plain),
    }
    job
}

#[cfg(test)]
mod tests;
