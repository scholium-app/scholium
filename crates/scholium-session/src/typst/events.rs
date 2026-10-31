//! Toolkit-independent input vocabulary used by the session core.
use super::*;

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

    #[test]
    fn every_shared_scenario_replays_through_abstract_inputs() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../spikes/typst-main-editor/scenarios");
        for file in std::fs::read_dir(root)
            .expect("scenario directory")
            .filter_map(Result::ok)
        {
            let path = file.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let value: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&path).expect("scenario"))
                    .expect("valid scenario JSON");
            assert!(value["inputs"].is_array(), "{}", path.display());
            assert!(
                value["expected"]["undo_depth"].is_u64(),
                "{}",
                path.display()
            );
            let db = fixture_path("scenario");
            let mut core = EditorCore::open(&db).expect("fixture");
            for event in value["inputs"].as_array().expect("inputs") {
                if let Some(text) = event["text"].as_str() {
                    core.input(Input::Text(text.to_owned()))
                        .expect("text input");
                } else if let Some(key) = event["key"].as_str() {
                    let key = match key {
                        "ArrowLeft" => Key::ArrowLeft,
                        "ArrowRight" => Key::ArrowRight,
                        "ArrowUp" => Key::ArrowUp,
                        "ArrowDown" => Key::ArrowDown,
                        "Delete" => Key::Delete,
                        "Enter" => Key::Enter,
                        "Character" => Key::Character(
                            event["value"]
                                .as_str()
                                .unwrap_or("?")
                                .chars()
                                .next()
                                .unwrap_or('?'),
                        ),
                        _ => Key::Escape,
                    };
                    core.input(Input::Key {
                        key,
                        mods: Modifiers::default(),
                    })
                    .ok();
                } else if event["pointer"].is_object() {
                    core.input(Input::Pointer {
                        kind: PointerKind::Release,
                        at: [0.0, 0.0],
                        clicks: event["pointer"]["clicks"].as_u64().unwrap_or(1) as u8,
                        mods: Modifiers::default(),
                    })
                    .expect("pointer input");
                }
            }
            let _ = std::fs::remove_file(db);
        }
    }
}
