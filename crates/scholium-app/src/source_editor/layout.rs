use eframe::egui;
pub(crate) fn source_job(
    ui: &egui::Ui,
    source: &str,
    highlight: Option<std::ops::Range<usize>>,
) -> egui::text::LayoutJob {
    let font = egui::FontId::monospace(13.0);
    let color = ui.visuals().text_color();
    let plain = egui::TextFormat::simple(font.clone(), color);
    let marked = egui::TextFormat {
        background: crate::theme::tokens(ui.ctx()).active_bg,
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

/// Read-only adapter used by generated source projections. Cache jobs, never atlas galleys.
#[derive(Default)]
pub(crate) struct ReadOnlySource {
    key: usize,
    highlight: Option<std::ops::Range<usize>>,
    colors: Option<[egui::Color32; 2]>,
    job: egui::text::LayoutJob,
}
impl ReadOnlySource {
    pub(crate) fn galley(
        &mut self,
        ui: &egui::Ui,
        key: usize,
        source: &str,
        highlight: Option<std::ops::Range<usize>>,
    ) -> (std::sync::Arc<egui::Galley>, bool) {
        let colors = [
            ui.visuals().text_color(),
            crate::theme::tokens(ui.ctx()).active_bg,
        ];
        let moved = self.key != key || self.highlight != highlight || self.colors.is_none();
        if moved || self.colors != Some(colors) {
            self.job = source_job(ui, source, highlight.clone());
            self.key = key;
            self.highlight = highlight;
            self.colors = Some(colors);
        }
        (
            ui.fonts_mut(|fonts| fonts.layout_job(self.job.clone())),
            moved,
        )
    }
    pub(crate) fn show(
        ui: &mut egui::Ui,
        source: &str,
        galley: std::sync::Arc<egui::Galley>,
        follow: Option<std::ops::Range<usize>>,
    ) {
        egui::ScrollArea::both()
            .id_salt("generated-source")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let response = ui.add(egui::Label::new(galley.clone()));
                if let Some(range) = follow {
                    let chars = source[..range.start].chars().count();
                    let row = galley.pos_from_cursor(egui::text::CCursor::new(chars));
                    ui.scroll_to_rect(
                        row.translate(response.rect.min.to_vec2()),
                        Some(egui::Align::Center),
                    );
                }
            });
    }
}
