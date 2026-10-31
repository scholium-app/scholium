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

    fn settle(core: &mut EditorCore) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while !core.scene_is_current() {
            core.poll_layout();
            assert!(core.layout_error.is_none(), "{:?}", core.layout_error);
            assert!(std::time::Instant::now() < deadline, "worker timeout");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
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

    fn scenario_pointer(core: &EditorCore, event: &serde_json::Value) -> Input {
        let target = event["target"].as_str().expect("semantic target");
        let leaves = input::leaves(&core.snapshot);
        let (leaf, byte) = if let Some(index) = target.strip_prefix("end of leaf ") {
            let leaf = &leaves[index.parse::<usize>().expect("leaf index")];
            (leaf.id, leaf.text.len())
        } else {
            let index = target
                .strip_prefix("word ")
                .expect("word target")
                .parse::<usize>()
                .expect("word index");
            let leaf = leaves.iter().find(|leaf| !leaf.math).expect("body leaf");
            let word = leaf.text.split_whitespace().nth(index).expect("word");
            (leaf.id, leaf.text.find(word).expect("word offset") + 1)
        };
        let caret = core
            .current_scene()
            .expect("current Scene")
            .geometry
            .carets
            .iter()
            .find(|c| c.position.leaf == leaf && c.position.byte == byte && c.exact)
            .expect("exact target caret");
        let at = [caret.top[0], (caret.top[1] + caret.bottom[1]) * 0.5];
        let kind = match event["kind"].as_str() {
            Some("Press") => PointerKind::Press,
            Some("Move") => PointerKind::Move,
            _ => PointerKind::Release,
        };
        Input::Pointer {
            kind,
            at,
            clicks: event["clicks"].as_u64().unwrap_or(1) as u8,
            mods: Modifiers::default(),
        }
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
            let mut core = EditorCore::open_with_wake(&db, || {}).expect("fixture");
            core.submit_layout();
            settle(&mut core);
            for event in value["inputs"].as_array().expect("inputs") {
                settle(&mut core);
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
                        "Slash" => Key::Slash,
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
                    let mods = Modifiers {
                        command: event["mods"]["command"].as_bool().unwrap_or(false),
                        shift: event["mods"]["shift"].as_bool().unwrap_or(false),
                        alt: event["mods"]["alt"].as_bool().unwrap_or(false),
                    };
                    core.input(Input::Key { key, mods }).expect("key input");
                    if let Key::Character(character) = key
                        && !mods.command
                        && !mods.alt
                    {
                        core.input(Input::Text(character.to_string()))
                            .expect("character commit");
                    }
                } else if event["pointer"].is_object() {
                    let input = scenario_pointer(&core, &event["pointer"]);
                    core.input(input).expect("pointer input");
                } else if event["command"] == "Bold" {
                    core.input(Input::Command(
                        CommandId::Style(TextStyle::Strong),
                        CommandArgs::None,
                    ))
                    .expect("bold command");
                }
            }
            settle(&mut core);
            let scene = core.current_scene().expect("real worker adopted a Scene");
            assert_eq!(scene.stamp.revision, core.snapshot.revision);
            assert_eq!(scene.stats.source_reads, 0);
            assert!(scene.geometry.caret(core.cursor).is_some());
            let _ = std::fs::remove_file(db);
        }
    }
}
