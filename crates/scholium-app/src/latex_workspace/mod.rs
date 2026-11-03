//! Development entry for the LaTeX source authority and same-engine preview workbench.
#[cfg(test)]
mod tests;
mod worker;
use crate::{
    i18n::{self, Locale},
    source_editor::SourceEditor,
};
use eframe::egui;
use scholium_latex::{
    Engine, SourceSession,
    view::{RasterOrigin, RasterPage},
};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    time::{Duration, Instant},
};
use worker::{Backend, Edit, Update};

pub(crate) struct Workspace {
    text: String,
    revision: u64,
    shown_revision: Option<u64>,
    generation: u64,
    editor: SourceEditor,
    backend: Option<Backend>,
    pages: BTreeMap<u64, Page>,
    error: Option<String>,
    unavailable: bool,
    engine: Engine,
    elapsed: Duration,
    edited_at: Option<Instant>,
    presented: Option<Duration>,
    paragraph: bool,
    source_rect: Option<egui::Rect>,
    preview_rect: Option<egui::Rect>,
    audit_last: Option<serde_json::Value>,
}
struct Page {
    texture: egui::TextureHandle,
    size: egui::Vec2,
    origin: RasterOrigin,
}
impl Workspace {
    pub(crate) fn open(project: PathBuf, ctx: &egui::Context) -> Self {
        let source = SourceSession::open(&project, "main.tex");
        let (text, engine, error, backend) = match source {
            Ok(source) => (
                source.text(source.entry()).unwrap_or_default().into(),
                source.engine(),
                None,
                Some(Backend::start(source, ctx.clone())),
            ),
            Err(error) => (
                String::new(),
                Engine::LuaLatex,
                Some(error.to_string()),
                None,
            ),
        };
        Self {
            text,
            revision: 0,
            shown_revision: None,
            generation: 0,
            editor: SourceEditor::default(),
            unavailable: backend.is_none(),
            backend,
            pages: BTreeMap::new(),
            error,
            engine,
            elapsed: Duration::ZERO,
            edited_at: None,
            presented: None,
            paragraph: false,
            source_rect: None,
            preview_rect: None,
            audit_last: None,
        }
    }
    pub(crate) fn show(&mut self, ui: &mut egui::Ui, locale: Locale) {
        self.poll(ui.ctx());
        egui::Panel::top("latex-header").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(i18n::text(locale, "latex.workspace"));
                ui.label(format!("{:?}", self.engine));
                ui.label(self.status(locale));
            });
            if let Some(error) = &self.error {
                ui.colored_label(ui.visuals().warn_fg_color, error);
            }
        });
        egui::CentralPanel::default().show(ui, |ui| {
            ui.columns(2, |columns| {
                self.source_rect = Some(columns[0].available_rect_before_wrap());
                columns[0].label(i18n::text(locale, "latex.source"));
                let highlights = syntax(&self.text, columns[0].visuals());
                if self
                    .editor
                    .show(&mut columns[0], &mut self.text, &highlights, &[], locale)
                {
                    self.revision += 1;
                    self.edited_at = Some(Instant::now());
                    self.presented = None;
                    if let Some(backend) = &self.backend {
                        let _ = backend.edits.send(Edit {
                            revision: self.revision,
                            text: self.text.clone(),
                        });
                    }
                }
                self.preview_rect = Some(columns[1].available_rect_before_wrap());
                columns[1].label(i18n::text(locale, "latex.preview"));
                egui::ScrollArea::both()
                    .id_salt("latex-preview")
                    .show(&mut columns[1], |ui| {
                        for page in self.pages.values() {
                            let width = ui.available_width().min(page.size.x);
                            let size = page.size * (width / page.size.x);
                            ui.add(egui::Image::new((page.texture.id(), size)));
                            ui.add_space(12.0);
                        }
                    });
            });
        });
        if self.shown_revision == Some(self.revision) && self.presented.is_none() {
            self.presented = self.edited_at.map(|t| t.elapsed());
        }
        self.audit();
    }
    fn poll(&mut self, ctx: &egui::Context) {
        loop {
            let update = self
                .backend
                .as_mut()
                .and_then(|b| b.updates.try_recv().ok());
            let Some(update) = update else {
                break;
            };
            match update {
                Update::Ready { engine } => {
                    self.engine = engine;
                    self.unavailable = false;
                    self.error = None;
                }
                Update::Unavailable(error) => {
                    self.error = Some(error);
                    self.unavailable = true;
                }
                Update::Error(error) => {
                    if (error.contains("layout") || error.contains("page"))
                        && let Some(b) = &self.backend
                    {
                        let _ = b.resync.try_send(());
                    }
                    self.error = Some(error);
                }
                Update::Page {
                    revision,
                    generation,
                    pages,
                    elapsed,
                    paragraph,
                    count,
                } => {
                    if generation < self.generation || revision != self.revision {
                        continue;
                    }
                    if generation != self.generation {
                        self.pages.clear();
                        self.generation = generation;
                    }
                    if let Some(count) = count {
                        self.pages.retain(|n, _| *n <= count);
                    }
                    self.upload(ctx, pages);
                    self.shown_revision = Some(revision);
                    self.elapsed = elapsed;
                    self.paragraph = paragraph;
                    self.error = None;
                }
            }
        }
    }
    fn upload(&mut self, ctx: &egui::Context, pages: Vec<RasterPage>) {
        for page in pages {
            let size = egui::vec2(page.width as f32, page.height as f32);
            let image = egui::ColorImage::from_rgba_premultiplied(
                [page.width as usize, page.height as usize],
                &page.rgba,
            );
            let texture = ctx.load_texture(
                format!("latex-page-{}", page.page),
                image,
                egui::TextureOptions::LINEAR,
            );
            self.pages.insert(
                page.page,
                Page {
                    texture,
                    size,
                    origin: page.origin,
                },
            );
        }
    }
    fn status(&self, locale: Locale) -> String {
        let mode = if self.engine == Engine::LuaLatex {
            "latex.live"
        } else {
            "latex.full"
        };
        let label = i18n::text_with(
            locale,
            mode,
            &[("ms", (self.elapsed.as_millis() as i64).into())],
        );
        let state = if self.unavailable {
            Some("latex.unavailable")
        } else if self.pages.is_empty() {
            Some("latex.building")
        } else if self.shown_revision != Some(self.revision) {
            Some("latex.stale")
        } else {
            None
        };
        state.map_or_else(
            || label.clone(),
            |key| format!("{label} · {}", i18n::text(locale, key)),
        )
    }

    fn audit(&mut self) {
        if std::env::var_os("SCHOLIUM_EDITOR_AUDIT").is_none() {
            return;
        }
        let rect = |r: Option<egui::Rect>| r.map(|r| [r.min.x, r.min.y, r.max.x, r.max.y]);
        let fact = serde_json::json!({"kind":"latex","revision":self.revision,"shown_revision":self.shown_revision,"generation":self.generation,"text":self.text,"engine":format!("{:?}",self.engine),"toolchain_unavailable":self.unavailable,"error":self.error,"pages":self.pages.len(),"source_rect":rect(self.source_rect),"preview_rect":rect(self.preview_rect),"paragraph":self.paragraph,"pixels_per_point":scholium_latex::view::PIXELS_PER_POINT,"origin":self.pages.values().map(|p|format!("{:?}",p.origin)).collect::<Vec<_>>(),"submitted_ms":self.presented.map(|d|d.as_secs_f64()*1000.0)});
        if self.audit_last.as_ref() != Some(&fact) {
            eprintln!("AUDIT {fact}");
            self.audit_last = Some(fact);
        }
    }
}
impl eframe::App for Workspace {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui, crate::i18n::LocaleSetting::Auto.resolve());
    }
}

fn syntax(text: &str, visuals: &egui::Visuals) -> Vec<crate::source_editor::Highlight> {
    scholium_latex::highlight(text)
        .into_iter()
        .map(|token| crate::source_editor::Highlight {
            bytes: token.bytes,
            color: match token.kind {
                scholium_latex::TokenKind::Text => visuals.text_color(),
                scholium_latex::TokenKind::Control => visuals.hyperlink_color,
                scholium_latex::TokenKind::Comment => visuals.weak_text_color(),
                scholium_latex::TokenKind::Math => visuals.selection.bg_fill,
                scholium_latex::TokenKind::Brace => visuals.warn_fg_color,
            },
        })
        .collect()
}
