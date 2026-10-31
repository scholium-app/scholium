//! Shared scenarios through the production input/worker/projection APIs.
use super::*;

fn settle(core: &mut EditorCore) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while !core.scene_is_current() {
        core.poll_layout();
        assert!(core.layout_error.is_none(), "{:?}", core.layout_error);
        assert!(std::time::Instant::now() < deadline, "worker timeout");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
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
        let db = std::env::temp_dir().join(format!("t20-scenario-{:?}.db", RequestId::fresh()));
        let mut core = EditorCore::open_with_wake(&db, || {}).expect("fixture");
        core.submit_layout();
        settle(&mut core);
        let mut depths = Vec::new();
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
                    "Backspace" => Key::Backspace,
                    "Home" => Key::Home,
                    "End" => Key::End,
                    "Tab" => Key::Tab,
                    "Escape" => Key::Escape,
                    "Slash" => Key::Slash,
                    "Character" => Key::Character(
                        event["value"]
                            .as_str()
                            .expect("character value")
                            .chars()
                            .next()
                            .expect("nonempty character"),
                    ),
                    _ => panic!("unknown scenario key: {key}"),
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
            } else {
                panic!("unknown scenario input: {event}");
            }
            depths.push(core.undo.len());
        }
        if value["name"] == "formula-fraction-superscript" {
            assert_eq!(depths, [1, 2, 3, 4, 4, 5, 5, 5], "undo grouping rules");
        }
        settle(&mut core);
        let scene = core.current_scene().expect("real worker adopted a Scene");
        assert_eq!(scene.stamp.revision, core.snapshot.revision);
        assert_eq!(scene.stats.source_reads, 0);
        assert!(scene.geometry.caret(core.cursor).is_some());
        assert_eq!(
            core.semantic_projection(),
            value["expected"],
            "{}: frozen semantic projection",
            path.display()
        );
        let _ = std::fs::remove_file(db);
    }
}
