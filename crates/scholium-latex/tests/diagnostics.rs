//! Transcript producer regression and arbitrary-input checks.
use scholium_latex::log::parse_log;
use scholium_model::diagnostic::Severity;

#[test]
fn errors_keep_the_active_include_file_and_log_position() {
    let log = "(./main.tex\n(./chapter.tex\n! Undefined control sequence.\nl.7 \\foo\n)\nLaTeX Warning: Reference `x' undefined on input line 9.\n)";
    let items = parse_log(log);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].resource.as_deref(), Some("chapter.tex"));
    assert_eq!(items[0].range.as_ref().expect("range").start.line, 7);
    assert_eq!(items[0].severity, Severity::Error);
    assert_eq!(items[1].resource.as_deref(), Some("main.tex"));
    assert_eq!(items[1].range.as_ref().expect("range").start.line, 9);
    assert_eq!(items[0].log.as_ref().expect("log").line, 3);
}

#[test]
fn unknown_external_files_never_fall_back_to_the_parent() {
    let items = parse_log(
        "(./main.tex (/usr/share/texlive/a.sty\n! Package a Error: bad.\nl.12 x\n))\n! unknown\nl.99 x",
    );
    assert!(items.iter().all(|d| d.resource.is_none()));
    assert!(items.iter().all(|d| d.log.is_some()));
}

#[test]
fn real_engine_logs_cover_errors_warnings_and_boxes() {
    for text in [
        include_str!("fixtures/logs/lualatex.log"),
        include_str!("fixtures/logs/xelatex.log"),
    ] {
        let items = parse_log(text);
        let error = items
            .iter()
            .find(|d| d.message.contains("Undefined control sequence"))
            .expect("undefined macro");
        assert_eq!(error.resource.as_deref(), Some("chapter.tex"));
        assert_eq!(error.range.as_ref().expect("line").start.line, 2);
        assert!(
            items
                .iter()
                .any(|d| d.message.contains("Citation") && d.severity == Severity::Warning)
        );
        assert!(
            items
                .iter()
                .any(|d| d.message.contains("Reference") && d.severity == Severity::Warning)
        );
        assert!(
            items
                .iter()
                .any(|d| d.message.contains("Package demo Error") && d.severity == Severity::Error)
        );
        assert!(
            items
                .iter()
                .any(|d| d.message.contains("Package demo Warning")
                    && d.severity == Severity::Warning)
        );
        assert!(
            items
                .iter()
                .any(|d| d.message.contains("Overfull") && d.severity == Severity::Info)
        );
        assert!(
            items
                .iter()
                .any(|d| d.message.contains("Underfull") && d.severity == Severity::Info)
        );
    }
}

#[test]
fn arbitrary_unicode_transcripts_never_panic_and_keep_valid_log_offsets() {
    let alphabet = [
        '!', '(', ')', '\n', '\r', '\0', '中', '🦀', 'l', '.', '9', ' ', '\\', '"',
    ];
    let mut state = 17u64;
    for len in 0..2048 {
        let mut text = String::new();
        for _ in 0..len % 257 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            text.push(alphabet[(state >> 32) as usize % alphabet.len()]);
        }
        text.push_str("\n! arbitrary\nl.99999999999999999999999999999999999999 x\n");
        for item in parse_log(&text) {
            let log = item.log.expect("original log position");
            assert!(text.get(log.bytes).is_some());
        }
    }
}

#[test]
fn wrapped_messages_and_quoted_paths_keep_original_positions() {
    let log = "(\"./章节 (一).tex\"\nPackage demo Warning: Missing citation\n(demo)                on input line 17.\nOverfull \\hbox (2pt too wide) at lines 18--19\n)\n(./main.tex\n! broken\nl.0 x";
    let items = parse_log(log);
    assert_eq!(items[0].resource.as_deref(), Some("章节 (一).tex"));
    assert_eq!(items[0].range.as_ref().expect("line").start.line, 17);
    assert!(items[0].message.contains("on input line 17"));
    assert_eq!(items[1].range.as_ref().expect("box line").start.line, 18);
    assert!(items[2].range.is_none());
    assert_eq!(
        &log[items[0].log.as_ref().expect("log").bytes.clone()],
        "Package demo Warning: Missing citation"
    );
}

#[test]
fn package_errors_without_bang_use_the_following_tex_line() {
    let items = parse_log("(./main.tex\nPackage demo Error: broken.\nl.8 \\demo\n)");
    assert_eq!(items[0].severity, Severity::Error);
    assert_eq!(
        items[0]
            .range
            .as_ref()
            .expect("package error line")
            .start
            .line,
        8
    );
}

#[test]
fn tex_print_line_wrapping_can_split_the_location_phrase() {
    let items =
        parse_log("(./main.tex\nLaTeX Warning: Citation `long' undefined on input\nline 23.\n\n)");
    assert_eq!(
        items[0]
            .range
            .as_ref()
            .expect("wrapped location")
            .start
            .line,
        23
    );
}
