#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewMode {
    #[default]
    Visual,
    Source,
}

/// Typst 预览投影与编译记账；页纹理不是权威内容（ADR 0027）。
#[derive(Default)]
pub(crate) struct PreviewState {
    /// 正在预览的文档身份；同 revision 的另一文档结果不可采纳。
    pub(crate) document: Option<scholium_model::DocumentId>,
    /// 需要显示的会话 revision。
    pub(crate) wanted: u64,
    /// 当前页面纹理对应的 revision。
    pub(crate) shown: Option<u64>,
    /// 最近一次提交给编译线程的 revision。
    pub(crate) submitted: Option<u64>,
    /// 已编译的文档 revision，可请求其中任意页。
    pub(crate) compiled: Option<u64>,
    /// `wanted` 最近变化时刻；编译需等待去抖窗口。
    pub(crate) changed_at: Option<std::time::Instant>,
    /// 已提交编译尚未返回当前需要的结果。
    pub(crate) pending: bool,
    /// 最近一次编译错误。
    pub(crate) error: Option<String>,
    pub(crate) warning: Option<String>,
    /// 最近一次编译耗时（毫秒）。
    pub(crate) elapsed_ms: u64,
    /// 最近一次页栅格化耗时（毫秒）；状态栏与编译耗时合并为端到端口径。
    pub(crate) raster_ms: u64,
    /// 已编译的总页数；页面栅格化按需进行。
    pub(crate) page_count: usize,
    /// 当前选择的零基页索引。
    pub(crate) page: usize,
    /// 当前页纹理及其页索引。
    pub(crate) page_texture: Option<eframe::egui::TextureHandle>,
    pub(crate) page_index: Option<usize>,
    /// 已请求栅格化的 (revision, 零基页索引)。
    pub(crate) page_requested: Option<(u64, usize)>,
    /// 当前页面对应的块锚点（page/pt）。
    pub(crate) anchors: Vec<scholium_typst::BlockAnchor>,
    /// Exact glyph boxes from the compiled revision, in page pt.
    pub(crate) geometry: Vec<scholium_typst::PageGeometry>,
    /// 预览点击定位请求：滚动源码窗格到该块。
    pub(crate) locate_request: Option<usize>,
    /// 源码点击定位请求：零基页与当前排版的页面 pt 纵坐标。
    pub(crate) scroll_request: Option<(usize, f32)>,
}

impl std::fmt::Debug for PreviewState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreviewState")
            .field("wanted", &self.wanted)
            .field("shown", &self.shown)
            .field("submitted", &self.submitted)
            .field("compiled", &self.compiled)
            .field("pending", &self.pending)
            .field("error", &self.error)
            .field("elapsed_ms", &self.elapsed_ms)
            .field("raster_ms", &self.raster_ms)
            .field("page_count", &self.page_count)
            .field("page", &self.page)
            .field("anchors", &self.anchors.len())
            .finish_non_exhaustive()
    }
}

impl PreviewState {
    /// 登记当前权威投影；文档身份和 revision 一起门控后台结果。
    pub(crate) fn note_snapshot(&mut self, snapshot: &scholium_model::DocumentSnapshot) {
        if self.document != Some(snapshot.document) {
            *self = Self::default();
            self.document = Some(snapshot.document);
        }
        if self.changed_at.is_none() || snapshot.revision.0 != self.wanted {
            self.wanted = snapshot.revision.0;
            self.changed_at = Some(std::time::Instant::now());
            self.error = None;
            self.warning = None;
            self.scroll_request = None;
        }
    }

    /// 会话重置时回到空预览。
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    /// 状态栏摘要。
    pub(crate) fn summary(&self) -> String {
        if self.error.is_some() {
            "Typst 排版失败".into()
        } else if self.warning.is_some() {
            "Typst 公式待完成".into()
        } else if self.pending {
            "Typst 编译中".into()
        } else if self.shown == Some(self.wanted) {
            format!("Typst 预览 r{}", self.wanted)
        } else if self.compiled == Some(self.wanted) {
            "Typst 页面渲染中".into()
        } else {
            "Typst 预览等待".into()
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dialect {
    #[default]
    Latex,
    Typst,
}

impl Dialect {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Latex => "LaTeX",
            Self::Typst => "Typst",
        }
    }
}

/// View preferences, owned session projection and outgoing request; never authoritative content.
#[derive(Debug)]
pub(crate) struct WorkspaceState {
    /// Shared immutable projection; cloning the Arc is the frame-to-frame cost,
    /// deep copies happen once per accepted edit (R6 of the rework plan).
    pub(crate) document: Option<std::sync::Arc<scholium_model::DocumentSnapshot>>,
    /// Accepted input spelling for one block/revision; prevents canonical markup
    /// escaping from moving the caret while a delimiter is still being typed.
    pub(crate) accepted_input: Option<(scholium_model::NodeId, u64, String)>,
    pub(crate) pending_edit: Option<scholium_model::DocumentRequest>,
    pub(crate) new_requested: bool,
    pub(crate) save_requested: bool,
    /// 已落盘的 revision；None 表示从未保存。
    pub(crate) saved_revision: Option<u64>,
    pub(crate) edit_error: Option<String>,
    /// 持久化或恢复错误；不得用编辑错误或后续输入覆盖。
    pub(crate) storage_error: Option<String>,
    /// Draft of the last rejected text edit, shown until the block changes again.
    pub(crate) rejected_draft: Option<(scholium_model::NodeId, String)>,
    pub(crate) composition: Option<String>,
    /// Block whose editor held focus last frame; toolbar targets follow it.
    pub(crate) focus_block: Option<scholium_model::NodeId>,
    /// Anchor block and block count when a splitting edit was sent; once the
    /// session grows past that count, focus moves to the block after the anchor.
    pub(crate) focus_after_split: Option<(scholium_model::NodeId, usize)>,
    /// Removed block, absorbing block and caret when a merge was sent; once the
    /// removed block is gone, focus lands on the absorbing block at the seam.
    pub(crate) focus_after_merge: Option<(scholium_model::NodeId, scholium_model::NodeId, usize)>,
    /// Block, caret and the revision the splice must reach before installing;
    /// installing earlier would clamp against the pre-edit buffer.
    pub(crate) focus_after_replace: Option<(scholium_model::NodeId, usize, u64)>,
    pub(crate) mode: ViewMode,
    pub(crate) ribbon: crate::ribbon::RibbonState,
    /// Visual workspace shows the Typst page with a focused block editor.
    pub(crate) visual_typeset: bool,
    pub(crate) preview: PreviewState,
    pub(crate) page_editor: crate::page_editor::EditorState,
    pub(crate) dialect: Dialect,
    pub(crate) navigation: bool,
    pub(crate) diagnostics: bool,
    pub(crate) preview_on_narrow: bool,
    pub(crate) split: f32,
    pub(crate) zoom: f32,
    pub(crate) fit_width: bool,
    /// Actual page scale from the last rendered viewport, used when leaving fit-width mode.
    pub(crate) rendered_zoom: f32,
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self {
            document: None,
            pending_edit: None,
            accepted_input: None,
            new_requested: false,
            save_requested: false,
            saved_revision: None,
            edit_error: None,
            storage_error: None,
            rejected_draft: None,
            composition: None,
            focus_block: None,
            focus_after_split: None,
            focus_after_merge: None,
            focus_after_replace: None,
            mode: ViewMode::Visual,
            ribbon: Default::default(),
            visual_typeset: true,
            preview: PreviewState::default(),
            page_editor: Default::default(),
            dialect: Dialect::Latex,
            navigation: false,
            diagnostics: false,
            preview_on_narrow: false,
            split: 0.5,
            zoom: 1.0,
            fit_width: true,
            rendered_zoom: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Dialect, PreviewState, ViewMode, WorkspaceState};

    #[test]
    fn default_workspace_starts_in_visual_fit_width_mode() {
        let state = WorkspaceState::default();
        assert_eq!(state.mode, ViewMode::Visual);
        assert_eq!(state.dialect, Dialect::Latex);
        assert!(state.fit_width);
        assert_eq!(state.zoom, 1.0);
        assert_eq!(state.split, 0.5);
        assert!(!state.navigation);
        assert!(!state.diagnostics);
        assert_eq!(state.rendered_zoom, 1.0);
        assert_eq!(state.focus_block, None);
        assert_eq!(state.focus_after_split, None);
        assert_eq!(state.focus_after_merge, None);
        assert_eq!(state.focus_after_replace, None);
        assert_eq!(state.rejected_draft, None);
        assert_eq!(state.preview.wanted, 0);
        assert!(state.preview.page_texture.is_none());
    }

    #[test]
    fn preview_revision_notes_reset_debounce_once_per_change() {
        let mut preview = PreviewState::default();
        let mut snapshot = scholium_model::DocumentSnapshot {
            document: scholium_model::DocumentId::fresh(),
            revision: scholium_model::Revision(3),
            blocks: Vec::new(),
        };
        preview.note_snapshot(&snapshot);
        assert_eq!(preview.wanted, 3);
        let first_change = preview.changed_at;
        preview.note_snapshot(&snapshot);
        assert_eq!(
            preview.changed_at, first_change,
            "same revision is not a change"
        );
        snapshot.revision = scholium_model::Revision(4);
        preview.note_snapshot(&snapshot);
        assert_ne!(preview.changed_at, first_change);
        preview.reset();
        assert_eq!(preview.wanted, 0);
        assert_eq!(preview.summary(), "Typst 预览等待");
    }
}
