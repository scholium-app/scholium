//! Platform picker boundary. The native portal implementation can replace the fallback here.
use super::*;
use crate::widgets::{self, DialogButton};
use std::path::Path;

#[derive(Clone, Copy, Debug)]
pub(super) enum Purpose {
    Open,
    Location,
}
#[derive(Clone, Copy, Debug)]
pub(super) enum Selection {
    FileOrFolder,
    Folder,
}
pub(super) enum Pick {
    Pending,
    Cancelled,
    Selected(PathBuf),
    Invalid,
}
/// Every platform implementation must return a canonical existing selection or cancellation;
/// selection must never create project metadata or modify source files.
pub(super) trait FilePicker {
    fn selection_kind(&self) -> Selection;
    fn show(&mut self, ctx: &egui::Context, locale: i18n::Locale, recent: &[PathBuf]) -> Pick;
}
// TODO(P1): SystemPicker implements FilePicker using xdg-desktop-portal; run picker contract tests
// against its selection adapter before choosing it as the default. No portal dependency today.
pub(super) struct FallbackPicker {
    path: String,
    selection: Selection,
    error: bool,
}
impl FallbackPicker {
    pub fn open() -> Self {
        Self {
            path: String::new(),
            selection: Selection::FileOrFolder,
            error: false,
        }
    }
    pub fn folder(path: &str) -> Self {
        Self {
            path: path.into(),
            selection: Selection::Folder,
            error: false,
        }
    }
}
fn selection(path: &Path, kind: Selection) -> Result<PathBuf, std::io::Error> {
    let path = path.canonicalize()?;
    if path.is_dir()
        || (matches!(kind, Selection::FileOrFolder)
            && path.is_file()
            && matches!(
                path.extension().and_then(|s| s.to_str()),
                Some("tex" | "typ")
            ))
    {
        Ok(path)
    } else {
        Err(std::io::ErrorKind::InvalidInput.into())
    }
}
impl FilePicker for FallbackPicker {
    fn selection_kind(&self) -> Selection {
        self.selection
    }
    fn show(&mut self, ctx: &egui::Context, locale: i18n::Locale, recent: &[PathBuf]) -> Pick {
        let text = |key| i18n::text(locale, key);
        let buttons = [
            DialogButton::new("cancel", text("chrome.dialog.cancel"), false),
            DialogButton::new("submit", text("project.ok"), true),
        ];
        let action = widgets::dialog(
            ctx,
            "file-picker",
            &text("project.picker.title"),
            locale,
            &buttons,
            |ui| {
                ui.label(text("project.path.open"));
                let error = self.error.then(|| text("project.picker.error"));
                let response = widgets::text_input(ui, &mut self.path, error.as_deref());
                audit_rect("path", response.rect);
                ui.label(text("project.picker.recent"));
                for path in recent {
                    let folder = if path.is_dir() {
                        path.as_path()
                    } else {
                        path.parent().unwrap_or(path)
                    };
                    let label = folder.display().to_string();
                    let t = crate::theme::tokens(ctx);
                    if widgets::row(
                        ui,
                        &widgets::Row {
                            glyph: crate::icons::Glyph::Open,
                            tint: t.text_dim,
                            primary: &label,
                            secondary: None,
                            selected: false,
                            secondary_right: true,
                        },
                        widgets::Meta::new(format!("picker.{label}"), "dialog", &label),
                    )
                    .clicked()
                    {
                        self.path = label;
                    }
                }
            },
        );
        match action.as_deref() {
            Some("cancel") => Pick::Cancelled,
            Some("submit") => match selection(Path::new(self.path.trim()), self.selection) {
                Ok(path) => Pick::Selected(path),
                Err(_) => {
                    self.error = true;
                    Pick::Pending
                }
            },
            _ => Pick::Pending,
        }
    }
}
// All providers pass through this boundary, including a future native result adapter.
fn checked_pick(
    picker: &mut dyn FilePicker,
    ctx: &egui::Context,
    locale: i18n::Locale,
    recent: &[PathBuf],
) -> Pick {
    match picker.show(ctx, locale, recent) {
        Pick::Selected(path) => match selection(&path, picker.selection_kind()) {
            Ok(path) => Pick::Selected(path),
            Err(_) => Pick::Invalid,
        },
        other => other,
    }
}
impl ProjectFiles {
    pub(super) fn picker_dialog(
        &mut self,
        ctx: &egui::Context,
        state: &mut WorkspaceState,
        project: &mut Option<ProjectWorkspace>,
    ) {
        let Some((purpose, picker)) = &mut self.picker else {
            return;
        };
        let purpose = *purpose;
        match checked_pick(picker.as_mut(), ctx, state.locale.resolve(), &self.recent) {
            Pick::Pending => {}
            Pick::Cancelled => self.picker = None,
            Pick::Invalid => {
                self.picker = None;
                self.error = Some(i18n::text(state.locale.resolve(), "project.picker.error"));
            }
            Pick::Selected(path) => {
                self.picker = None;
                match purpose {
                    Purpose::Location => self.new.location = path.display().to_string(),
                    Purpose::Open => {
                        if path.is_dir()
                            && !path.join(".scholium/project.toml").exists()
                            && entries(&path).len() != 1
                        {
                            self.path = path.display().to_string();
                            self.entry.clear();
                            self.dialog = Some(FileDialog::Entry);
                        } else {
                            self.request(Operation::Open(path, None), ctx, state, project);
                        }
                    }
                }
            }
        }
    }
}
pub(super) fn entries(root: &Path) -> Vec<PathBuf> {
    let mut result: Vec<_> = std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_type().is_ok_and(|t| t.is_file())
                && matches!(
                    e.path().extension().and_then(|s| s.to_str()),
                    Some("tex" | "typ")
                )
        })
        .map(|e| PathBuf::from(e.file_name()))
        .collect();
    result.sort();
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    // The same suite is callable by the future system picker's result adapter.
    fn contract(select: impl Fn(&Path, Selection) -> Result<PathBuf, std::io::Error>) {
        let dir = tempfile::tempdir().expect("directory");
        let file = dir.path().join("paper.tex");
        std::fs::write(&file, "untouched").expect("file");
        assert_eq!(select(&file, Selection::FileOrFolder).expect("file"), file);
        assert!(select(&file, Selection::Folder).is_err());
        assert!(select(&dir.path().join("absent"), Selection::FileOrFolder).is_err());
        assert!(select(dir.path(), Selection::Folder).is_ok());
        assert!(!dir.path().join(".scholium").exists());
        assert_eq!(std::fs::read_to_string(file).expect("source"), "untouched");
    }
    #[test]
    fn fallback_selection_satisfies_the_platform_contract() {
        contract(selection);
    }

    struct PlatformResult {
        path: PathBuf,
        kind: Selection,
    }
    impl FilePicker for PlatformResult {
        fn selection_kind(&self) -> Selection {
            self.kind
        }
        fn show(&mut self, _: &egui::Context, _: i18n::Locale, _: &[PathBuf]) -> Pick {
            Pick::Selected(self.path.clone())
        }
    }

    #[test]
    fn platform_results_and_fallback_selections_share_the_validation_contract() {
        contract(|path, kind| {
            let mut picker = PlatformResult {
                path: path.into(),
                kind,
            };
            match checked_pick(
                &mut picker,
                &egui::Context::default(),
                i18n::Locale::En,
                &[],
            ) {
                Pick::Selected(path) => Ok(path),
                _ => Err(std::io::ErrorKind::InvalidInput.into()),
            }
        });
    }
}
