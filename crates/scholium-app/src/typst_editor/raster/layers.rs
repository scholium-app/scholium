//! The page as two texture layers: a whole-page base and sharp strips for the visible part.
//!
//! Strips belong to exactly one Scene; adopting a Scene replaces them all, so a strip of an
//! older Scene is never drawn over a newer base. Strips at a new scale gather in `next` and
//! replace the shown ones only once they cover the visible range.
use super::super::EditorSession;
use super::*;
use scholium_typst::editor::{
    EditorScene, Raster, RasterSpec, SHARP_STRIP_PX, StripUpdate, ViewRange, strip_window,
};
use std::collections::BTreeMap;

/// A changed visible range is sent to the worker at most this often while scrolling.
pub(super) const VIEW_THROTTLE: Duration = Duration::from_millis(40);
/// Visible-range changes smaller than this are not worth a request.
const VIEW_EPSILON_PT: f64 = 0.5;

fn same_view(a: ViewRange, b: ViewRange) -> bool {
    (a.top_pt - b.top_pt).abs() < VIEW_EPSILON_PT
        && (a.bottom_pt - b.bottom_pt).abs() < VIEW_EPSILON_PT
}

/// Texture bands of one page bitmap, keyed by their first pixel row.
struct Bands {
    px_per_pt: f64,
    width: u32,
    height: u32,
    bands: BTreeMap<u32, (u32, egui::TextureHandle)>,
}

impl Bands {
    fn empty(px_per_pt: f64, width: u32, height: u32) -> Self {
        Self {
            px_per_pt,
            width,
            height,
            bands: BTreeMap::new(),
        }
    }

    /// Upload every strip of `raster`, taking the bitmaps.
    fn upload(ctx: &egui::Context, raster: &mut Raster) -> Self {
        let mut bands = Self::empty(raster.px_per_pt, raster.width, raster.height);
        for strip in std::mem::take(&mut raster.strips) {
            bands.insert(ctx, strip.y, strip.height, &strip.rgba);
        }
        bands
    }

    fn insert(&mut self, ctx: &egui::Context, y: u32, rows: u32, rgba: &[u8]) {
        let image =
            egui::ColorImage::from_rgba_premultiplied([self.width as usize, rows as usize], rgba);
        let texture = ctx.load_texture(
            format!("typst-content-{y}"),
            image,
            egui::TextureOptions::LINEAR,
        );
        self.bands.insert(y, (rows, texture));
    }

    fn paint(&self, painter: &egui::Painter, origin: Pos2, scale: f32) {
        let ppp = painter.ctx().pixels_per_point();
        let rows: Vec<_> = self
            .bands
            .iter()
            .map(|(y, (rows, _))| (*y, *rows))
            .collect();
        let rects = strip_rects(origin, scale, ppp, self.px_per_pt, self.width, &rows);
        for (rect, (_, texture)) in rects.into_iter().zip(self.bands.values()) {
            painter.image(
                texture.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
    }

    fn has(&self, index: u32) -> bool {
        self.bands.contains_key(&(index * SHARP_STRIP_PX))
    }

    /// Drop strips outside the three-viewport keep window.
    fn evict(&mut self, view: ViewRange) {
        let keep = strip_window(view, self.px_per_pt, self.height).keep;
        self.bands
            .retain(|y, _| keep.contains(&(y / SHARP_STRIP_PX)));
    }

    /// Every strip that intersects the visible range is present.
    fn covers(&self, view: ViewRange) -> bool {
        strip_window(view, self.px_per_pt, self.height)
            .visible
            .all(|index| self.has(index))
    }
}

fn same_scale(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs())
}

/// Base and sharp textures of the Scene on screen.
pub(in crate::typst_editor) struct Layers {
    base: Bands,
    sharp: Bands,
    next: Option<Bands>,
}

impl Layers {
    /// Upload the base and the sharp strips of `scene`, taking their bitmaps.
    pub(in crate::typst_editor) fn from_scene(
        ctx: &egui::Context,
        scene: &mut EditorScene,
    ) -> Self {
        Self {
            base: Bands::upload(ctx, &mut scene.base),
            sharp: Bands::upload(ctx, &mut scene.sharp),
            next: None,
        }
    }

    /// Resolution of the sharp strips on screen.
    pub(in crate::typst_editor) fn sharp_px_per_pt(&self) -> f64 {
        self.sharp.px_per_pt
    }

    /// File a strip rendered after its Scene: same scale joins the shown set, another scale
    /// builds the replacement set.
    fn add(&mut self, ctx: &egui::Context, update: StripUpdate) {
        let set = if same_scale(update.px_per_pt, self.sharp.px_per_pt) {
            &mut self.sharp
        } else {
            if !self
                .next
                .as_ref()
                .is_some_and(|n| same_scale(n.px_per_pt, update.px_per_pt))
            {
                self.next = Some(Bands::empty(
                    update.px_per_pt,
                    update.page_width,
                    update.page_height,
                ));
            }
            self.next.as_mut().expect("replacement set was just made")
        };
        set.insert(ctx, update.strip.y, update.strip.height, &update.strip.rgba);
    }

    /// Evict strips outside the keep window and swap in a finished replacement set.
    pub(in crate::typst_editor) fn maintain(&mut self, view: ViewRange) {
        self.sharp.evict(view);
        if let Some(next) = &mut self.next {
            next.evict(view);
        }
        if self
            .next
            .as_ref()
            .is_some_and(|n| !n.bands.is_empty() && n.covers(view))
        {
            self.sharp = self.next.take().expect("checked above");
        }
    }

    pub(in crate::typst_editor) fn covers(&self, view: ViewRange) -> bool {
        self.sharp.covers(view)
    }

    /// Base stretched over the page, then the sharp strips over it.
    pub(in crate::typst_editor) fn paint(&self, painter: &egui::Painter, origin: Pos2, scale: f32) {
        self.base.paint(painter, origin, scale);
        self.sharp.paint(painter, origin, scale);
    }

    /// Diagnostics: `(base px/pt, sharp page width px, sharp page height px, sharp strips)`.
    pub(in crate::typst_editor) fn audit(&self) -> (f64, u32, u32, usize) {
        (
            self.base.px_per_pt,
            self.sharp.width,
            self.sharp.height,
            self.sharp.bands.len(),
        )
    }
}

impl EditorSession {
    /// A sharp strip rendered after its Scene; strips of any other Scene are dropped.
    pub(in crate::typst_editor) fn adopt_strip(
        &mut self,
        ctx: &egui::Context,
        update: StripUpdate,
    ) {
        self.settle.arrived();
        let current = self.scene.as_ref().is_some_and(|s| s.stamp == update.stamp);
        if let (true, Some(layers)) = (current, self.texture.as_mut()) {
            layers.add(ctx, update);
        }
    }

    /// Follow the resolution and visible range the placed page needs: ask the worker for
    /// missing strips once the scale is stable, and promptly (throttled) when scrolling.
    pub(in crate::typst_editor) fn track_raster(
        &mut self,
        ctx: &egui::Context,
        px_per_pt: f64,
        view: ViewRange,
    ) {
        self.spec = RasterSpec {
            px_per_pt: forced_px_per_pt().unwrap_or(px_per_pt),
            max_side: ctx.input(|i| i.max_texture_side) as u32,
        };
        self.view = view;
        let (Some(scene), Some(layers)) = (&self.scene, &mut self.texture) else {
            return;
        };
        layers.maintain(view);
        let now = Instant::now();
        let wanted = self.spec.resolve(scene.size_pt);
        let shown = layers.sharp_px_per_pt();
        let spec = match self.settle.step(now, wanted, Some(shown)) {
            Step::Request => Some(self.spec),
            Step::Wait(after) => {
                ctx.request_repaint_after(after);
                None
            }
            Step::Idle => None,
        };
        if let Some(spec) = spec {
            self.send_view(now, spec);
            return;
        }
        let unsent = self.sent.is_none_or(|(_, sent)| !same_view(sent, view));
        if !unsent {
            return;
        }
        let due = self.sent.map_or(Duration::ZERO, |(at, _)| {
            VIEW_THROTTLE.saturating_sub(now.saturating_duration_since(at))
        });
        if due.is_zero() {
            let spec = RasterSpec {
                px_per_pt: shown,
                ..self.spec
            };
            self.send_view(now, spec);
        } else {
            ctx.request_repaint_after(due);
        }
    }

    fn send_view(&mut self, now: Instant, spec: RasterSpec) {
        self.worker.set_view(spec, self.view);
        self.sent = Some((now, self.view));
    }
}
