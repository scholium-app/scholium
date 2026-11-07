use eframe::egui;
pub(crate) fn source_job(
    ui: &egui::Ui,
    source: &str,
    highlight: Option<std::ops::Range<usize>>,
) -> egui::text::LayoutJob {
    let tokens = scholium_typst::highlight_source(source);
    let highlights = super::typst_styles(&tokens, ui.visuals());
    let marks: Vec<_> = highlight
        .into_iter()
        .map(|bytes| super::Mark {
            bytes,
            color: crate::theme::tokens(ui.ctx()).active_bg,
            message: None,
        })
        .collect();
    super::highlight::job(ui, source, &highlights, &marks)
}

/// Read-only adapter used by generated source projections. Cache jobs, never atlas galleys.
#[derive(Default)]
pub(crate) struct ReadOnlySource {
    key: usize,
    highlight: Option<std::ops::Range<usize>>,
    colors: Option<[egui::Color32; 5]>,
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
            ui.visuals().hyperlink_color,
            ui.visuals().weak_text_color(),
            ui.visuals().warn_fg_color,
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
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let gutter_width = super::gutter::width(source);
                    let height = ui.available_height().max(galley.size().y);
                    let gutter = ui
                        .allocate_exact_size(egui::vec2(gutter_width, height), egui::Sense::hover())
                        .0;
                    let response = ui.add(egui::Label::new(galley.clone()));
                    let painter = ui.painter().with_clip_rect(ui.clip_rect());
                    for (index, row) in galley.rows.iter().enumerate() {
                        let top = response.rect.top() + row.pos.y;
                        if top + row.size.y >= ui.clip_rect().top()
                            && top <= ui.clip_rect().bottom()
                        {
                            painter.text(
                                egui::pos2(gutter.right() - 8.0, top),
                                egui::Align2::RIGHT_TOP,
                                (index + 1).to_string(),
                                egui::FontId::monospace(13.0),
                                crate::theme::tokens(ui.ctx()).text_dim,
                            );
                        }
                    }
                    if let Some(range) = follow {
                        let chars = source[..range.start].chars().count();
                        let row = galley.pos_from_cursor(egui::text::CCursor::new(chars));
                        ui.scroll_to_rect(
                            row.translate(response.rect.min.to_vec2()),
                            Some(egui::Align::Center),
                        );
                    }
                });
            });
    }
}
