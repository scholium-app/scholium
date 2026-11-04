use super::*;
#[test]
fn foundation_hides_unimplemented_entries_and_native_tools_in_source_projects() {
    for locale in [ZH, EN] {
        let mut w = Window::new(1280.0, locale);
        assert!(!w.ids("toolbar").contains(&"tools.search".into()));
        assert!(!w.ids("toolbar").contains(&"format.underline".into()));
        assert!(!w.ids("focus").iter().any(|id| id.starts_with("document.")));
        for language in [
            scholium_storage::project::ProjectLanguage::Typst,
            scholium_storage::project::ProjectLanguage::Latex,
        ] {
            w.state.source_language = Some(language);
            w.state.mode = ViewMode::Source;
            w.settle();
            assert_eq!(w.ids("toolbar"), ["edit.undo", "edit.redo"]);
            assert_eq!(w.ids("focus"), ["view.mode.page", "view.mode.source"]);
            assert!(!w.ids("menubar").contains(&"ui.menu.insert".into()));
            assert!(!w.ids("menubar").contains(&"ui.menu.format".into()));
        }
    }
}
