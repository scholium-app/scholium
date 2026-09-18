//! Revision-gated Typst editor pages with compiler-derived input geometry.
#[cfg(test)]
mod incremental_tests;
mod projection;
mod session;
mod worker;
use super::*;
use std::sync::{Mutex, mpsc};

#[derive(Debug, PartialEq)]
pub(super) struct Cell {
    node: NodeId,
    cursor: Option<Cursor>,
    end: usize,
    rect: egui::Rect,
    text: String,
}

struct Page {
    texture: egui::TextureHandle,
    size: egui::Vec2,
    y: f32,
    tiles: Vec<worker::Tile>,
}

impl Page {
    fn update(&mut self, raster: [usize; 2], tiles: Vec<worker::Tile>) -> usize {
        if self.texture.size() != raster {
            self.texture.set(
                egui::ColorImage::filled(raster, egui::Color32::TRANSPARENT),
                egui::TextureOptions::LINEAR,
            );
            self.tiles.clear();
        }
        let mut uploads = 0;
        for (i, tile) in tiles.iter().enumerate() {
            if !self
                .tiles
                .get(i)
                .is_some_and(|old| old.at == tile.at && Arc::ptr_eq(&old.image, &tile.image))
            {
                self.texture
                    .set_partial(tile.at, tile.image.clone(), egui::TextureOptions::LINEAR);
                uploads += 1;
            }
        }
        self.tiles = tiles;
        uploads
    }
}

type Job = (u64, scholium_spike_core::Document);
type Finished = (u64, Result<Vec<worker::ResultPage>, String>);

pub(super) fn preview_source(doc: &scholium_spike_core::Document) -> String {
    projection::Projection::new(doc).source
}

pub(super) struct TypstEditor {
    pub enabled: bool,
    pub status: String,
    revision: Option<u64>,
    submitted: Option<u64>,
    queue: Option<Arc<Mutex<Option<Job>>>>,
    receive: Option<mpsc::Receiver<Finished>>,
    worker_thread: Option<std::thread::Thread>,
    pages: Vec<Page>,
    cells: Vec<Cell>,
    pub size: egui::Vec2,
}

impl Default for TypstEditor {
    fn default() -> Self {
        Self {
            enabled: true,
            status: "正在准备 Typst 排版".into(),
            revision: None,
            submitted: None,
            queue: None,
            receive: None,
            worker_thread: None,
            pages: vec![],
            cells: vec![],
            size: egui::vec2(440.0, 620.0),
        }
    }
}

impl TypstEditor {
    pub fn current(&self, revision: u64) -> bool {
        self.enabled && self.revision == Some(revision)
    }

    pub fn poll(&mut self, ctx: &egui::Context, core: &Editor) -> bool {
        if !self.enabled {
            return false;
        }
        self.start_worker(ctx);
        self.submit(core);
        let mut adopted = false;
        while let Some(result) = self.receive.as_ref().and_then(|r| r.try_recv().ok()) {
            if result.0 != core.revision() {
                continue;
            }
            match result.1 {
                Ok(pages) => {
                    self.adopt(ctx, result.0, pages);
                    adopted = true;
                }
                Err(error) => {
                    self.status = format!("Typst 排版失败：{error}");
                    eprintln!("[typst-editor] {}", self.status);
                }
            }
        }
        if !self.current(core.revision()) {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
        adopted
    }

    fn start_worker(&mut self, ctx: &egui::Context) {
        if self.queue.is_none() {
            let queue: Arc<Mutex<Option<Job>>> = Arc::new(Mutex::new(None));
            let pending = queue.clone();
            let (send, receive) = mpsc::channel();
            let repaint = ctx.clone();
            let thread = std::thread::spawn(move || {
                let mut compiler = worker::Compiler::default();
                while Arc::strong_count(&pending) > 1 {
                    let request = pending.lock().ok().and_then(|mut slot| slot.take());
                    if let Some((revision, doc)) = request {
                        let started = std::time::Instant::now();
                        let result = compiler.compile(&doc);
                        println!(
                            "[typst-editor-worker] revision={revision} elapsed_ms={:.3}",
                            started.elapsed().as_secs_f64() * 1000.0
                        );
                        if send.send((revision, result)).is_err() {
                            break;
                        }
                        repaint.request_repaint();
                    } else {
                        std::thread::park_timeout(std::time::Duration::from_secs(1));
                    }
                }
            });
            self.queue = Some(queue);
            self.receive = Some(receive);
            self.worker_thread = Some(thread.thread().clone());
        }
    }

    fn submit(&mut self, core: &Editor) {
        if self.submitted != Some(core.revision())
            && let Some(queue) = &self.queue
            && let Ok(mut slot) = queue.try_lock()
        {
            *slot = Some((core.revision(), core.document().clone()));
            self.submitted = Some(core.revision());
            if let Some(thread) = &self.worker_thread {
                thread.unpark();
            }
            self.status = format!(
                "Typst 排版中 · 画面 {:?} / 正文 {} · 暂停旧画面定位",
                self.revision,
                core.revision()
            );
        }
    }

    fn adopt(&mut self, ctx: &egui::Context, revision: u64, pages: Vec<worker::ResultPage>) {
        self.pages.truncate(pages.len());
        self.cells.clear();
        let mut y = 0.0_f32;
        let mut width = 0.0_f32;
        let mut uploads = 0;
        for (index, page) in pages.into_iter().enumerate() {
            width = width.max(page.size.x);
            self.cells.extend(page.cells.into_iter().map(|mut cell| {
                cell.rect = cell.rect.translate(egui::vec2(0.0, y));
                cell
            }));
            if index == self.pages.len() {
                self.pages.push(Page {
                    texture: ctx.load_texture(
                        format!("editor-page-{index}"),
                        egui::ColorImage::filled(page.raster, egui::Color32::TRANSPARENT),
                        egui::TextureOptions::LINEAR,
                    ),
                    size: page.size,
                    y,
                    tiles: vec![],
                });
            }
            let previous = &mut self.pages[index];
            uploads += previous.update(page.raster, page.tiles);
            previous.size = page.size;
            previous.y = y;
            y += page.size.y + 16.0;
        }
        self.size = egui::vec2(width, y);
        self.revision = Some(revision);
        self.status = format!("Typst · revision {} · {} 页", revision, self.pages.len());
        println!(
            "[typst-editor] {} · {} boxes",
            self.status,
            self.cells.len()
        );
        println!("[typst-editor-tiles] revision={revision} uploaded={uploads}");
    }

    pub fn paint(&self, painter: &egui::Painter, origin: egui::Pos2, color: egui::Color32) {
        for page in &self.pages {
            let rect = egui::Rect::from_min_size(origin + egui::vec2(0.0, page.y), page.size);
            if painter.clip_rect().intersects(rect) {
                painter.image(
                    page.texture.id(),
                    rect,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    color,
                );
            }
        }
    }

    pub fn caret(&self, cursor: Cursor) -> Option<egui::Rect> {
        let start = self.cells.iter().find(|c| c.cursor == Some(cursor));
        if let Some(cell) = start {
            return Some(egui::Rect::from_min_size(
                cell.rect.min,
                egui::vec2(1.5, cell.rect.height()),
            ));
        }
        let Cursor::Text { node, byte } = cursor else {
            return None;
        };
        let cell = self
            .cells
            .iter()
            .rev()
            .find(|c| c.node == node && c.cursor.is_some() && c.end == byte)?;
        Some(egui::Rect::from_min_size(
            cell.rect.right_top(),
            egui::vec2(1.5, cell.rect.height()),
        ))
    }

    pub fn hit(&self, point: egui::Pos2, doc: &scholium_spike_core::Document) -> Option<Cursor> {
        let mut candidates: Vec<_> = self
            .cells
            .iter()
            .filter(|c| c.cursor.is_some() && c.rect.expand(2.0).contains(point))
            .collect();
        candidates.sort_by(|a, b| {
            let score = |c: &Cell| {
                c.rect.distance_sq_to_pos(point) * 100.0 + (point.y - c.rect.center().y).powi(2)
            };
            score(a).total_cmp(&score(b))
        });
        candidates.into_iter().find_map(|cell| {
            let mut cursor = cell.cursor?;
            if let Cursor::Text { node, byte } = cursor {
                let byte = if point.x > cell.rect.center().x {
                    cell.end
                } else {
                    byte
                };
                if !doc.node(node).ok()?.text.is_grapheme_boundary(byte) {
                    return None;
                }
                cursor = Cursor::Text { node, byte };
            }
            Some(cursor)
        })
    }

    pub fn bounds(&self, doc: &scholium_spike_core::Document, root: NodeId) -> Option<egui::Rect> {
        self.cells
            .iter()
            .filter(|cell| {
                let mut node = Some(cell.node);
                while let Some(id) = node {
                    if id == root {
                        return true;
                    }
                    node = doc.node(id).ok().and_then(|n| n.parent);
                }
                false
            })
            .map(|c| c.rect)
            .reduce(|a, b| a.union(b))
    }

    pub fn selection(
        &self,
        painter: &egui::Painter,
        origin: egui::Pos2,
        node: NodeId,
        start: usize,
        end: usize,
    ) {
        for cell in &self.cells {
            if let Some(Cursor::Text { byte, .. }) = cell.cursor
                && cell.node == node
                && byte < end
                && cell.end > start
            {
                painter.rect_filled(
                    cell.rect.translate(origin.to_vec2()),
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(50, 130, 230, 65),
                );
            }
        }
    }

    pub fn layout_items(&self) -> Vec<Item> {
        self.cells
            .iter()
            .filter_map(|c| {
                let Cursor::Text { node, byte } = c.cursor? else {
                    return None;
                };
                Some(Item::Text {
                    x: c.rect.left(),
                    baseline: c.rect.bottom(),
                    size: c.rect.height(),
                    content: c.text.clone(),
                    source: Some(scholium_spike_core::layout::SourceSpan {
                        node,
                        start_byte: byte,
                    }),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn flattened_image(page: &worker::ResultPage) -> egui::ColorImage {
        let mut image = egui::ColorImage::filled(page.raster, egui::Color32::TRANSPARENT);
        for tile in &page.tiles {
            for (y, row) in tile.image.pixels.chunks(tile.image.size[0]).enumerate() {
                let start = (tile.at[1] + y) * page.raster[0] + tile.at[0];
                image.pixels[start..start + row.len()].copy_from_slice(row);
            }
        }
        image
    }

    #[test]
    fn editing_keeps_the_existing_page_texture() {
        let ctx = egui::Context::default();
        let mut view = TypstEditor::default();
        let page = || worker::ResultPage {
            raster: [16, 16],
            tiles: vec![worker::Tile {
                at: [0, 0],
                image: Arc::new(egui::ColorImage::filled(
                    [16, 16],
                    egui::Color32::TRANSPARENT,
                )),
            }],
            size: egui::vec2(8.0, 8.0),
            cells: vec![],
        };
        view.adopt(&ctx, 1, vec![page()]);
        let id = view.pages[0].texture.id();
        view.adopt(&ctx, 2, vec![page()]);
        assert_eq!(
            view.pages[0].texture.id(),
            id,
            "editing must not replace a page texture"
        );
    }

    #[test]
    #[ignore = "requires built Typst helper and Linux sandbox"]
    fn page_and_empty_slots_are_transparent_but_still_have_carets() {
        let mut core = Editor::new();
        fixture::build_standard(&mut core);
        let pages = worker::compile(core.document()).expect("compile");
        let mut empty_count = 0;
        for page in pages {
            let image = flattened_image(&page);
            assert_eq!(
                image.pixels[0].a(),
                0,
                "page must use the window background"
            );
            assert!(
                image.pixels.iter().any(|p| p.a() > 0),
                "text must remain visible"
            );
            for cell in page
                .cells
                .iter()
                .filter(|c| c.cursor.is_some() && c.text.is_empty())
            {
                empty_count += 1;
                assert!(cell.rect.width() > 0.0 && cell.rect.height() > 0.0);
                // Worker raster is 2 px per Typst point; inspect the actual slot area.
                for y in (cell.rect.top() * 2.0).ceil() as usize
                    ..(cell.rect.bottom() * 2.0).floor() as usize
                {
                    for x in (cell.rect.left() * 2.0).ceil() as usize
                        ..(cell.rect.right() * 2.0).floor() as usize
                    {
                        assert_eq!(
                            image.pixels[y * image.size[0] + x].a(),
                            0,
                            "empty slot must not paint a square"
                        );
                    }
                }
            }
        }
        assert!(empty_count > 0, "fixture needs an editable empty slot");
    }

    #[test]
    fn stale_compilation_never_installs_pages_or_geometry() {
        let core = Editor::new();
        let (send, receive) = mpsc::channel();
        let mut view = TypstEditor {
            submitted: Some(core.revision()),
            queue: Some(Arc::new(Mutex::new(None))),
            receive: Some(receive),
            ..Default::default()
        };
        assert!(
            send.send((core.revision().wrapping_sub(1), Ok(vec![])))
                .is_ok()
        );
        assert!(!view.poll(&egui::Context::default(), &core));
        assert!(!view.current(core.revision()));
        assert!(view.pages.is_empty() && view.cells.is_empty());
    }

    #[test]
    #[ignore = "requires built Typst helper and Linux sandbox"]
    fn compiler_preserves_quoted_math_spaces_and_escaped_text() {
        let mut core = Editor::new();
        fixture::build_standard(&mut core);
        let mut stack = vec![core.document().root()];
        let mut leaves = Vec::new();
        while let Some(id) = stack.pop() {
            let node = core.document().node(id).expect("node");
            if node.kind.is_text() {
                leaves.push(id);
            }
            stack.extend(node.slots.iter().flatten().copied());
        }
        for id in &leaves {
            if core.document().text_of(*id).unwrap_or_default() == "a" {
                core.apply(
                    LOCAL,
                    Intent::Typing,
                    SemanticEdit::InsertText {
                        node: *id,
                        at: 0,
                        text: "Q a ".into(),
                    },
                )
                .expect("edit");
            }
        }
        let first = core
            .document()
            .first_text_descendant(core.document().root())
            .expect("first");
        core.apply(
            LOCAL,
            Intent::Typing,
            SemanticEdit::InsertText {
                node: first,
                at: 0,
                text: " A \" \\ $ ".into(),
            },
        )
        .expect("edit");
        let pages = worker::compile(core.document()).expect("compiler");
        for id in leaves {
            let mut cells: Vec<_> = pages
                .iter()
                .flat_map(|p| &p.cells)
                .filter(|c| c.node == id && c.cursor.is_some())
                .collect();
            cells.sort_by_key(|c| match c.cursor {
                Some(Cursor::Text { byte, .. }) => byte,
                _ => 0,
            });
            let actual: String = cells.iter().map(|c| c.text.as_str()).collect();
            assert_eq!(
                actual,
                core.document().text_of(id).unwrap_or_default(),
                "missing text for {id:?}"
            );
        }
    }

    #[test]
    #[ignore = "requires built Typst helper and Linux sandbox"]
    fn compiled_editor_maps_all_fixture_text_and_rejects_stale_geometry() {
        let ctx = egui::Context::default();
        let mut core = Editor::new();
        fixture::build_standard(&mut core);
        let pages = worker::compile(core.document()).expect("sandbox compilation");
        let mut seen = std::collections::HashSet::new();
        for page in &pages {
            for cell in &page.cells {
                if let Some(Cursor::Text { node, byte }) = cell.cursor {
                    assert!(
                        core.document()
                            .node(node)
                            .expect("node")
                            .text
                            .is_grapheme_boundary(byte)
                    );
                    println!(
                        "node {} byte {}..{} {:?}",
                        node.index(),
                        byte,
                        cell.end,
                        cell.text
                    );
                    seen.insert(node);
                }
            }
        }
        let mut stack = vec![core.document().root()];
        while let Some(node) = stack.pop() {
            let n = core.document().node(node).expect("node");
            if n.kind.is_text() {
                assert!(seen.contains(&node), "missing text node {}", node.index());
            }
            stack.extend(n.slots.iter().flatten().copied());
        }
        let (send, receive) = mpsc::channel();
        let mut view = TypstEditor {
            enabled: true,
            submitted: Some(core.revision()),
            queue: Some(Arc::new(Mutex::new(None))),
            receive: Some(receive),
            ..Default::default()
        };
        assert!(send.send((core.revision(), Ok(pages))).is_ok());
        assert!(view.poll(&ctx, &core));
        assert!(view.current(core.revision()));
        assert!(view.hit(egui::pos2(0.0, 0.0), core.document()).is_none());
        assert!(!view.current(core.revision() + 1));
        for cell in &view.cells {
            if let Some(cursor) = cell.cursor {
                assert!(view.caret(cursor).is_some());
                if cell.text.is_empty() {
                    assert_eq!(
                        view.hit(cell.rect.center(), core.document()),
                        Some(cursor),
                        "invisible slot must remain clickable"
                    );
                }
            }
        }
    }
}
