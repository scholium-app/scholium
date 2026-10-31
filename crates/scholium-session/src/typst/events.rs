//! Toolkit-independent input vocabulary used by the session core.
use super::*;

/// Document edge used by a follow request.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FollowEdge {
    Top,
    Bottom,
}

/// Follow state retained until a current Scene can provide the caret rectangle.
#[allow(missing_docs)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FollowState {
    pub pending: bool,
    pub edge: Option<FollowEdge>,
}

/// Keys understood by the UI-free editing core.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Home,
    End,
    Backspace,
    Delete,
    Enter,
    Tab,
    Escape,
    Slash,
    Num(u8),
    Character(char),
}

/// Modifier state independent of any window toolkit.
#[allow(missing_docs)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Modifiers {
    pub command: bool,
    pub shift: bool,
    pub alt: bool,
}

/// Pointer phase; coordinates are page points in the current Scene.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerKind {
    Press,
    Move,
    Release,
}

/// Abstract input accepted by [`EditorCore::input`].
#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub enum Input {
    Key {
        key: Key,
        mods: Modifiers,
    },
    Text(String),
    Pointer {
        kind: PointerKind,
        at: [f64; 2],
        clicks: u8,
        mods: Modifiers,
    },
    Command(CommandId, CommandArgs),
    Viewport {
        visible: ViewRange,
        raster: RasterSpec,
    },
}

/// Pointer state held between abstract press, drag and release events.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerState {
    pub at: [f64; 2],
    pub clicks: u8,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typst::input;
    use scholium_model::RequestId;
    fn fixture_path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("t17-{label}-{:?}.db", RequestId::fresh()))
    }

    #[test]
    fn abstract_input_edits_text_and_reports_semantic_geometry() {
        let file = fixture_path("input");
        let mut core = EditorCore::open(&file).expect("fixture");
        core.input(Input::Text("中文 English".into()))
            .expect("text");
        let outcome = core
            .input(Input::Key {
                key: Key::ArrowLeft,
                mods: Modifiers::default(),
            })
            .expect("key");
        assert_eq!(
            outcome.caret.map(|target| target.byte),
            Some("中文 Englis".len())
        );
        assert_eq!(input::leaves(&core.snapshot)[0].text, "中文 English");
        let _ = std::fs::remove_file(file);
    }

    #[test]
    fn failed_key_and_pointer_inputs_restore_the_complete_checkpoint() {
        let file = fixture_path("rollback");
        let mut core = EditorCore::open(&file).expect("fixture");
        core.input(Input::Text("saved".into())).expect("text");
        core.save().expect("save");
        let before = format!("{:?}", core.checkpoint().expect("checkpoint"));
        core.fail_next_input = true;
        assert!(
            core.input(Input::Key {
                key: Key::ArrowLeft,
                mods: Modifiers::default()
            })
            .is_err()
        );
        assert_eq!(
            format!("{:?}", core.checkpoint().expect("checkpoint")),
            before
        );
        core.fail_next_input = true;
        assert!(
            core.input(Input::Pointer {
                kind: PointerKind::Press,
                at: [10.0, 20.0],
                clicks: 1,
                mods: Modifiers::default()
            })
            .is_err()
        );
        assert_eq!(
            format!("{:?}", core.checkpoint().expect("checkpoint")),
            before
        );
        let reopened = EditorCore::open(&file).expect("saved head");
        assert_eq!(reopened.snapshot, core.snapshot);
        let _ = std::fs::remove_file(file);
    }
}
