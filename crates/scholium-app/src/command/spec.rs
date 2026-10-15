//! Command data model. No egui types: this layer moves into the UI-independent editing
//! core later, and every entry point (menu, toolbar, shortcut, search) reads it.
use crate::{
    commands::ViewCommand, i18n::LocaleSetting, state::ViewMode, theme::ThemeKind,
    typst_editor::Command,
};

/// Stable dotted id (`format.bold`, `view.mode.source`); never shown, never translated.
pub(crate) type CommandId = &'static str;

/// Catalog key of a title, tooltip or reason (see `crate::i18n`).
pub(crate) type CatalogKey = &'static str;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ChordKey {
    /// Upper-case ASCII letter.
    Letter(char),
    Digit(u8),
    Function(u8),
    Plus,
    Equals,
    Minus,
    Slash,
}

/// A key combination; `ctrl` means the platform command key (Ctrl, or Cmd on macOS).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct KeyChord {
    pub(crate) ctrl: bool,
    pub(crate) shift: bool,
    pub(crate) key: ChordKey,
}

impl KeyChord {
    pub(crate) const fn ctrl(key: ChordKey) -> Self {
        Self {
            ctrl: true,
            shift: false,
            key,
        }
    }

    pub(crate) const fn ctrl_shift(key: ChordKey) -> Self {
        Self {
            ctrl: true,
            shift: true,
            key,
        }
    }

    /// Menu text such as `Ctrl+Shift+B`; key names are not translated.
    pub(crate) fn display(&self) -> String {
        let mut text = String::new();
        if self.ctrl {
            text.push_str("Ctrl+");
        }
        if self.shift {
            text.push_str("Shift+");
        }
        match self.key {
            ChordKey::Letter(c) => text.push(c),
            ChordKey::Digit(d) => text.push_str(&d.to_string()),
            ChordKey::Function(n) => text.push_str(&format!("F{n}")),
            ChordKey::Plus => text.push('+'),
            ChordKey::Equals => text.push('='),
            ChordKey::Minus => text.push('-'),
            ChordKey::Slash => text.push('/'),
        }
        text
    }
}

/// Who receives a chord. `App` chords are consumed before any widget sees them; `Editor`
/// chords stay in the page's event stream so they run in keystroke order with typing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Routing {
    App,
    Editor,
    /// Delivered by the platform (clipboard events); listed for display only.
    Platform,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum MenuId {
    File,
    Edit,
    Insert,
    Format,
    Document,
    View,
    Tools,
    Help,
}

impl MenuId {
    pub(crate) const ALL: [Self; 8] = [
        Self::File,
        Self::Edit,
        Self::Insert,
        Self::Format,
        Self::Document,
        Self::View,
        Self::Tools,
        Self::Help,
    ];

    pub(crate) fn key(self) -> CatalogKey {
        match self {
            Self::File => "menu.file",
            Self::Edit => "menu.edit",
            Self::Insert => "menu.insert",
            Self::Format => "menu.format",
            Self::Document => "menu.document",
            Self::View => "menu.view",
            Self::Tools => "menu.tools",
            Self::Help => "menu.help",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Submenu {
    ParagraphStyle,
    WorkMode,
    Zoom,
    Theme,
    Language,
}

impl Submenu {
    pub(crate) fn key(self) -> CatalogKey {
        match self {
            Self::ParagraphStyle => "menu.format.paragraph_style",
            Self::WorkMode => "menu.view.work_mode",
            Self::Zoom => "menu.view.zoom",
            Self::Theme => "menu.view.theme",
            Self::Language => "menu.view.language",
        }
    }
}

/// Menu placement: items sort by `(group, order)`; a group change draws a separator.
#[derive(Clone, Copy, Debug)]
pub(crate) struct MenuPlacement {
    pub(crate) menu: MenuId,
    pub(crate) submenu: Option<Submenu>,
    pub(crate) group: u8,
    pub(crate) order: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolbarGroup {
    Paragraph,
    Text,
    Insert,
    History,
}

impl ToolbarGroup {
    /// Display order; the toolbar separates groups and moves whole groups into overflow.
    pub(crate) const ALL: [Self; 4] = [Self::Paragraph, Self::Text, Self::Insert, Self::History];
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ToolbarPlacement {
    pub(crate) group: ToolbarGroup,
    pub(crate) order: u8,
}

/// Which focus toolbar shows the command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FocusKind {
    Text,
    Math,
}

/// Icon identity; the UI edge maps it to a Lucide glyph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IconId {
    None,
    New,
    Open,
    Save,
    Export,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    Find,
    Bold,
    Italic,
    Underline,
    Strike,
    ClearFormat,
    Formula,
    Fraction,
    Root,
    Page,
    Margins,
    Code,
    Navigation,
    Diagnostics,
    Zoom,
    Search,
    DisplayFormula,
    Paragraph,
    Heading1,
    Heading2,
    Attach,
    Table,
    Image,
    Link,
    Delimiter,
    Matrix,
    ExitFormula,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClipboardOp {
    Copy,
    Cut,
    Paste,
}

/// What running a command does; reuses the handlers that already exist.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Action {
    View(ViewCommand),
    Editor(Command),
    Undo,
    Redo,
    Clipboard(ClipboardOp),
    /// Not integrated yet; availability reports `Unimplemented`.
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Availability {
    Enabled,
    Disabled(CatalogKey),
    Unimplemented(CatalogKey),
}

impl Availability {
    pub(crate) fn is_enabled(self) -> bool {
        self == Self::Enabled
    }

    /// The reason to show on hover when the command cannot run.
    pub(crate) fn reason(self) -> Option<CatalogKey> {
        match self {
            Self::Enabled => None,
            Self::Disabled(key) | Self::Unimplemented(key) => Some(key),
        }
    }
}

/// The facts availability and checked state depend on, built from the workspace state.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CommandContext {
    pub(crate) has_document: bool,
    pub(crate) mode: ViewMode,
    pub(crate) composing: bool,
    pub(crate) in_math: bool,
    pub(crate) selected: bool,
    pub(crate) can_undo: bool,
    pub(crate) can_redo: bool,
    pub(crate) strong: bool,
    pub(crate) emphasis: bool,
    pub(crate) navigation: bool,
    pub(crate) diagnostics: bool,
    pub(crate) fit_width: bool,
    pub(crate) actual_size: bool,
    pub(crate) toolbars_visible: bool,
    pub(crate) theme: ThemeKind,
    pub(crate) locale: LocaleSetting,
}

pub(crate) type AvailabilityFn = fn(&CommandContext) -> Availability;
pub(crate) type CheckedFn = fn(&CommandContext) -> bool;

#[derive(Clone, Debug)]
pub(crate) struct CommandSpec {
    pub(crate) id: CommandId,
    pub(crate) title: CatalogKey,
    #[allow(dead_code)] // the longer description; read by the command search (next task)
    pub(crate) tooltip: CatalogKey,
    pub(crate) icon: IconId,
    pub(crate) shortcuts: &'static [KeyChord],
    pub(crate) routing: Routing,
    pub(crate) menu: Option<MenuPlacement>,
    pub(crate) toolbar: Option<ToolbarPlacement>,
    pub(crate) focus: Option<FocusKind>,
    #[allow(dead_code)] // marks commands the read-only source view must refuse
    pub(crate) writes: bool,
    pub(crate) availability: AvailabilityFn,
    pub(crate) checked: Option<CheckedFn>,
    pub(crate) action: Action,
}

impl CommandSpec {
    /// Defaults for the `command!` macro: always enabled, no placement, no action.
    pub(crate) const BASE: Self = Self {
        id: "",
        title: "",
        tooltip: "",
        icon: IconId::None,
        shortcuts: &[],
        routing: Routing::App,
        menu: None,
        toolbar: None,
        focus: None,
        writes: false,
        availability: super::avail::always,
        checked: None,
        action: Action::None,
    };
}
