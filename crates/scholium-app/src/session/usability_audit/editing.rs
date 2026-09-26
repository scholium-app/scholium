use super::*;

audit!(a01_enter_at_inline_formula_end_preserves_math, {
    let mut h = Harness::new("$alpha$");
    h.select(6, 6); // The right edge of the visible alpha, before hidden `$`.
    h.press(Key::Enter);
    assert_eq!(
        h.snapshot().blocks[0].content,
        [Inline::Math("alpha".into())]
    );
    assert_eq!(h.texts(), ["$alpha$", ""]);
});

audit!(a02_enter_at_display_formula_end_preserves_math, {
    let mut h = Harness::new("$ alpha/2 $");
    h.select(9, 9);
    h.press(Key::Enter);
    assert_eq!(h.texts(), ["$ alpha/2 $", ""]);
});

audit!(
    a03_enter_inside_formula_does_not_turn_delimiters_into_text,
    {
        let mut h = Harness::new("$alpha + beta$");
        h.select(6, 6);
        h.press(Key::Enter);
        let snapshot = h.snapshot();
        assert!(
            snapshot
                .blocks
                .iter()
                .flat_map(|b| &b.content)
                .any(|i| matches!(i, Inline::Math(_))),
            "formula lost: {:?}",
            snapshot.blocks
        );
        assert!(
            !snapshot
                .blocks
                .iter()
                .flat_map(|b| &b.content)
                .any(|i| matches!(i, Inline::Text(s) if s.contains('$'))),
            "{:?}",
            snapshot.blocks
        );
    }
);

audit!(a04_enter_inside_bold_preserves_formatting, {
    let mut h = Harness::new("*abcd*");
    h.select(3, 3);
    h.press(Key::Enter);
    assert_eq!(h.texts(), ["*ab*", "*cd*"]);
});

audit!(a05_backspace_after_formula_keeps_formula_structure, {
    let mut h = Harness::new("$alpha$");
    h.select(7, 7);
    h.press(Key::Backspace);
    assert!(
        !h.snapshot().blocks[0]
            .content
            .iter()
            .any(|i| matches!(i, Inline::Text(s) if s.contains('$'))),
        "{:?}",
        h.texts()
    );
});

audit!(a06_delete_before_formula_keeps_formula_structure, {
    let mut h = Harness::new("$alpha$");
    h.select(0, 0);
    h.press(Key::Delete);
    assert!(
        !h.snapshot().blocks[0]
            .content
            .iter()
            .any(|i| matches!(i, Inline::Text(s) if s.contains('$'))),
        "{:?}",
        h.texts()
    );
});

audit!(a07_paste_plain_punctuation_matches_typing, {
    let mut typed = Harness::new("");
    typed.frame(vec![Event::Text("a_b *c* \\path".into())]);
    let mut pasted = Harness::new("");
    pasted.frame(vec![Event::Paste("a_b *c* \\path".into())]);
    assert_eq!(
        pasted.snapshot().blocks[0].content,
        typed.snapshot().blocks[0].content
    );
});

audit!(
    a08_multiline_unicode_paste_preserves_empty_last_paragraph,
    {
        let mut h = Harness::new("");
        // eframe normalizes CRLF before delivering Event::Paste. Both keyboard
        // and Ribbon paths are checked in audit-editor-native.py.
        h.frame(vec![Event::Paste("第一行\n第二行\n".into())]);
        assert_eq!(h.texts(), ["第一行", "第二行", ""]);
    }
);

audit!(a09_cancelled_ime_allows_following_typing, {
    let mut h = Harness::new("");
    h.frame(vec![Event::Ime(egui::ImeEvent::Preedit {
        text: "zhong".into(),
        active_range_chars: None,
    })]);
    h.frame(vec![Event::Ime(egui::ImeEvent::Preedit {
        text: String::new(),
        active_range_chars: None,
    })]);
    h.frame(vec![Event::Text("a".into())]);
    assert_eq!(h.texts(), ["a"]);
    assert!(h.state.composition.is_none());
});

audit!(a10_backspace_preserves_emoji_graphemes_without_geometry, {
    for text in ["e\u{301}", "👨‍👩‍👧‍👦", "🇨🇳", "👍🏽", "中"] {
        let mut h = Harness::new(text);
        h.select(text.len(), text.len());
        h.press(Key::Backspace);
        assert_eq!(h.texts(), [""], "{text}");
    }
});

audit!(a11_plain_paragraph_split_and_merge_round_trip, {
    let mut h = Harness::new("甲乙");
    h.select(3, 3);
    h.press(Key::Enter);
    assert_eq!(h.texts(), ["甲", "乙"]);
    h.press(Key::Backspace);
    assert_eq!(h.texts(), ["甲乙"]);
});

audit!(a12_cross_paragraph_selection_is_atomic, {
    let mut h = Harness::new("ab\ncd\nef");
    let revision = h.snapshot().revision.0;
    h.select(1, 4);
    h.frame(vec![Event::Text("中".into())]);
    assert_eq!(h.texts(), ["a中d", "ef"]);
    assert_eq!(h.snapshot().revision.0, revision + 1);
});

audit!(a13_keyboard_formula_entry_round_trips, {
    let mut h = Harness::new("");
    for ch in "$alpha/2$ tail".chars() {
        h.frame(vec![Event::Text(ch.to_string())]);
    }
    assert_eq!(h.texts(), ["$alpha/2$ tail"]);
    assert_eq!(
        h.snapshot().blocks[0].content[0],
        Inline::Math("alpha/2".into())
    );
});

audit!(a14_ime_commit_is_one_edit_and_preedit_is_not_saved, {
    let mut h = Harness::new("");
    let revision = h.snapshot().revision.0;
    h.frame(vec![Event::Ime(egui::ImeEvent::Preedit {
        text: "zhong".into(),
        active_range_chars: None,
    })]);
    assert_eq!(h.snapshot().revision.0, revision);
    h.frame(vec![Event::Ime(egui::ImeEvent::Commit("中文".into()))]);
    assert_eq!(h.texts(), ["中文"]);
    assert_eq!(h.snapshot().revision.0, revision + 1);
});

audit!(a15_home_end_without_geometry_stay_in_current_paragraph, {
    let mut h = Harness::new("first\nsecond\nthird");
    h.select(8, 8);
    h.press(Key::Home);
    assert_eq!(h.state.page_editor.caret, 6);
    h.press(Key::End);
    assert_eq!(h.state.page_editor.caret, 12);
});

audit!(a16_copy_cut_paste_preserves_formula_markup, {
    let mut h = Harness::new("中文 $alpha$ tail");
    let original = h.texts()[0].clone();
    h.select(0, original.len());
    let output = h.frame(vec![Event::Cut]);
    let copied = output
        .platform_output
        .commands
        .iter()
        .find_map(|command| match command {
            egui::OutputCommand::CopyText(text) => Some(text.clone()),
            _ => None,
        })
        .expect("clipboard text");
    assert_eq!(copied, original);
    assert_eq!(h.texts(), [""]);
    h.frame(vec![Event::Paste(copied)]);
    assert_eq!(h.texts(), [original]);
});

audit!(a17_undo_restores_typing, {
    let mut h = Harness::new("before");
    h.select(6, 6);
    h.frame(vec![Event::Text(" after".into())]);
    h.frame(vec![key_event(Key::Z, Modifiers::COMMAND)]);
    assert_eq!(
        h.texts(),
        ["before"],
        "Known missing capability: no undo path"
    );
});

audit!(a18_select_all_replacement_preserves_unicode, {
    let mut h = Harness::new("a\n$alpha$\n中文");
    h.frame(vec![key_event(Key::A, Modifiers::COMMAND)]);
    h.frame(vec![Event::Text("替换 🦀".into())]);
    assert_eq!(h.texts(), ["替换 🦀"]);
});
