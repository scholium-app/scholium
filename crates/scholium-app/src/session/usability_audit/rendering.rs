use super::*;

audit!(b01_real_formula_glyph_click_then_enter_preserves_formula, {
    let mut h = Harness::new("$alpha$");
    h.compile();
    h.click_token_end(0, "alpha");
    assert_eq!(h.state.page_editor.caret, 6, "verify real glyph hit");
    h.press(Key::Enter);
    assert_eq!(h.texts(), ["$alpha$", ""]);
});

audit!(b02_pending_echo_does_not_paint_hidden_math_delimiters, {
    let mut h = Harness::new("before $alpha$ tail");
    h.compile();
    let end = h.texts()[0].len();
    h.select(end, end);
    let output = h.frame(vec![Event::Text("!".into())]);
    let text = painted_text(&output);
    assert!(text.contains('!'), "new input must be visible");
    assert!(!text.contains('$'), "raw markup painted on page: {text:?}");
});

audit!(b03_pending_echo_does_not_paint_bold_markers, {
    let mut h = Harness::new("before *bold* tail");
    h.compile();
    let end = h.texts()[0].len();
    h.select(end, end);
    let output = h.frame(vec![Event::Text("!".into())]);
    let text = painted_text(&output);
    assert!(text.contains('!'));
    assert!(!text.contains('*'), "raw markup painted on page: {text:?}");
});

audit!(b04_first_input_visible_before_any_compile, {
    let mut h = Harness::new("");
    let output = h.frame(vec![Event::Text("first".into())]);
    assert!(
        painted_text(&output).contains("first"),
        "no geometry means no immediate input echo"
    );
});

audit!(b05_typing_after_enter_is_visible_before_compile, {
    let mut h = Harness::new("abc");
    h.compile();
    h.select(3, 3);
    h.press(Key::Enter);
    let output = h.frame(vec![Event::Text("NEW".into())]);
    assert_eq!(h.texts(), ["abc", "NEW"]);
    assert!(
        painted_text(&output).contains("NEW"),
        "new paragraph has no immediate echo"
    );
});

audit!(
    b06_stale_geometry_after_deletion_keeps_surviving_line_clickable,
    {
        let mut h = Harness::new("abcdef");
        h.compile();
        let glyph = h.state.preview.geometry[0]
            .cells
            .iter()
            .find(|g| g.input == (5..6))
            .expect("f glyph")
            .clone();
        h.select(0, 4);
        h.press(Key::Delete);
        assert_eq!(h.texts(), ["ef"]);
        h.click(egui::pos2(
            10.0 + glyph.rect[2] - 0.1,
            10.0 + (glyph.rect[1] + glyph.rect[3]) / 2.0,
        ));
        assert_eq!(
            h.state.page_editor.caret, 2,
            "surviving f must map to current end"
        );
    }
);

audit!(b07_stale_second_block_geometry_is_shifted_once, {
    let mut h = Harness::new("abc\ndef");
    h.compile();
    h.select(0, 0);
    h.frame(vec![Event::Text("XYZ".into())]);
    h.click_token_end(1, "e");
    assert_eq!(
        h.state.page_editor.caret, 9,
        "abc gained 3 bytes, second block e ends at 9"
    );
});

audit!(b08_stale_unicode_click_inserts_at_clicked_position, {
    let mut h = Harness::new("abc\ndef");
    h.compile();
    let point = h.token_end_point(0, "b");
    h.select(0, 0);
    h.frame(vec![Event::Text("中".into())]);
    // This checks every mapped glyph boundary against the *live* UTF-8 buffer.
    h.click(point);
    h.frame(vec![Event::Text("文".into())]);
    assert_eq!(h.texts(), ["中ab文c", "def"]);
});

audit!(b09_arrow_right_at_formula_end_exits_math, {
    let mut h = Harness::new("$alpha$ tail");
    h.compile();
    h.click_token_end(0, "alpha");
    h.press(Key::ArrowRight);
    assert_eq!(
        h.state.page_editor.caret, 7,
        "leave math before consuming following space"
    );
});

audit!(
    b10_backspace_after_formula_with_geometry_preserves_structure,
    {
        let mut h = Harness::new("$alpha$");
        h.compile();
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
    }
);

audit!(b11_compiled_unicode_backspace_removes_whole_grapheme, {
    let text = "👍🏽";
    let mut h = Harness::new(text);
    h.compile();
    h.select(text.len(), text.len());
    h.press(Key::Backspace);
    assert_eq!(h.texts(), [""]);
});

audit!(b12_formula_compiles_and_maps_exact_token, {
    let mut h = Harness::new("中文 $alpha + x/2$ tail");
    h.compile();
    h.click_token_end(0, "alpha");
    assert_eq!(h.state.page_editor.caret, "中文 $alpha".len());
    assert!(h.state.preview.warning.is_none());
});

audit!(b13_stale_click_and_typing_in_one_frame_never_panics, {
    let mut h = Harness::new("abc\ndef");
    h.compile();
    let point = h.token_end_point(1, "e");
    h.select(0, 0);
    h.frame(vec![Event::Text("XYZ".into())]);
    let pointer = |pressed| Event::PointerButton {
        pos: point,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    h.frame(vec![Event::PointerMoved(point), pointer(true)]);
    h.frame(vec![pointer(false), Event::Text("!".into())]);
    assert_eq!(h.texts(), ["XYZabc", "de!f"]);
});
