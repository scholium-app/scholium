# Generated catalog layout: see ADR-0040. Keys: command.<id>.title|tooltip, menu.<id>, reason.<name>.

# Command titles and tooltips

command-file-new =
    .title = New Project…
    .tooltip = Start a new document

command-file-open =
    .title = Open Folder or File…
    .tooltip = Open an existing document

command-file-recent =
    .title = Open Recent
    .tooltip = Recently opened documents

command-file-save =
    .title = Save
    .tooltip = Save the current document

command-file-save_as =
    .title = Save As…
    .tooltip = Save the document as a separate project

command-file-export =
    .title = Export…
    .tooltip = Export a copy in another format

command-file-close =
    .title = Close
    .tooltip = Close the current document

command-edit-undo =
    .title = Undo
    .tooltip = Undo the last edit

command-edit-redo =
    .title = Redo
    .tooltip = Redo the undone edit

command-edit-cut =
    .title = Cut
    .tooltip = Cut the selection

command-edit-copy =
    .title = Copy
    .tooltip = Copy the selection

command-edit-paste =
    .title = Paste
    .tooltip = Paste at the caret

command-edit-find =
    .title = Find and Replace…
    .tooltip = Find and replace text in the document

command-insert-formula =
    .title = Inline Formula
    .tooltip = Enter or leave a formula

command-insert-exit_formula =
    .title = Leave Formula
    .tooltip = Leave the current formula; the caret stays after it

command-insert-display_formula =
    .title = Display Formula
    .tooltip = Insert a formula on its own line

command-insert-fraction =
    .title = Fraction
    .tooltip = Insert an empty fraction at the caret, or wrap the selected math; Tab changes slots

command-insert-root =
    .title = Radical
    .tooltip = Insert a radical

command-insert-superscript =
    .title = Superscript
    .tooltip = Insert a superscript in a formula

command-insert-subscript =
    .title = Subscript
    .tooltip = Insert a subscript in a formula

command-insert-delimiter =
    .title = Delimiters
    .tooltip = Insert brackets and other delimiters

command-insert-matrix =
    .title = Matrix
    .tooltip = Insert a matrix

command-insert-table =
    .title = Table
    .tooltip = Insert a table

command-insert-image =
    .title = Image
    .tooltip = Insert an image

command-insert-link =
    .title = Link
    .tooltip = Insert a link

command-format-bold =
    .title = Bold
    .tooltip = Applies to the selection, or to the text typed next

command-format-italic =
    .title = Italic
    .tooltip = Applies to the selection, or to the text typed next

command-format-underline =
    .title = Underline
    .tooltip = Underline the text

command-format-strike =
    .title = Strikethrough
    .tooltip = Strike the text through

command-format-clear =
    .title = Clear Formatting
    .tooltip = Remove bold and emphasis from the selection, or type plain text next

command-format-paragraph-body =
    .title = Body Text
    .tooltip = Applies to the current paragraph

command-format-paragraph-heading1 =
    .title = Heading 1
    .tooltip = Applies to the current paragraph

command-format-paragraph-heading2 =
    .title = Heading 2
    .tooltip = Applies to the current paragraph

command-document-style_template =
    .title = Styles and Templates…
    .tooltip = Choose the document style and template

command-document-page_size =
    .title = Paper Size
    .tooltip = Set the paper size

command-document-margins =
    .title = Margins
    .tooltip = Set the page margins

command-document-orientation =
    .title = Orientation
    .tooltip = Set the paper orientation

command-document-language =
    .title = Document Language
    .tooltip = Set the language used for typesetting

command-document-font =
    .title = Body Font
    .tooltip = Set the body font

command-document-font_size =
    .title = Font Size
    .tooltip = Set the body font size

command-document-final_check =
    .title = Final Layout Check
    .tooltip = Run the final build and check the layout

command-view-mode-page =
    .title = Page Editing
    .tooltip = Edit on the page · Ctrl+Shift+M to switch

command-view-mode-source =
    .title = Source Preview
    .tooltip = Source and preview · Ctrl+Shift+M to switch

command-view-mode-toggle =
    .title = Switch Work Mode
    .tooltip = Switch between page editing and source preview

command-view-navigation =
    .title = File Navigation
    .tooltip = Show or hide file navigation

command-view-diagnostics =
    .title = Diagnostics
    .tooltip = Show diagnostics

command-view-split =
    .title = Split View
    .tooltip = Show source and preview side by side

command-view-zoom-fit_width =
    .title = Fit Width
    .tooltip = Fit the page to the window width

command-view-zoom-actual =
    .title = Actual Size
    .tooltip = Show the page at 100%

command-view-zoom-in =
    .title = Zoom In
    .tooltip = Enlarge the page

command-view-zoom-out =
    .title = Zoom Out
    .tooltip = Shrink the page

command-view-toolbars =
    .title = Show Toolbars
    .tooltip = Show or hide the main and focus toolbars

command-view-theme-light =
    .title = Light
    .tooltip = Use the light theme

command-view-theme-dark =
    .title = Dark
    .tooltip = Use the dark theme

command-view-language-auto =
    .title = Follow System
    .tooltip = The interface language follows the system

command-view-language-zh_hans =
    .title = 简体中文
    .tooltip = 使用简体中文界面

command-view-language-en =
    .title = English
    .tooltip = Use the English interface

command-tools-search =
    .title = Command Search
    .tooltip = Search for a command by name and run it

command-tools-preferences =
    .title = Preferences…
    .tooltip = Open preferences

command-help-shortcuts =
    .title = Keyboard Shortcuts
    .tooltip = List all keyboard shortcuts

command-help-commands =
    .title = Command Reference
    .tooltip = Describe the available commands

command-help-about =
    .title = About Scholium
    .tooltip = Application information


# Menu names

menu-file = File

menu-edit = Edit

menu-insert = Insert

menu-format = Format

menu-document = Document

menu-view = View

menu-tools = Tools

menu-help = Help

menu-format-paragraph_style = Paragraph Style

menu-view-work_mode = Work Mode

menu-view-zoom = Zoom

menu-view-theme = Theme

menu-view-language = Language


# Availability reasons

reason-no_document = No document is open

reason-source_readonly = The source view is read-only; switch back to page editing first

reason-composing = Finish the IME composition first

reason-no_selection = Select something first

reason-nothing_to_undo = Nothing to undo

reason-nothing_to_redo = Nothing to redo

reason-not_in_formula = The caret is not in a formula

reason-unimplemented = This feature is not available yet

reason-final_build = The final build is not available yet

reason-document_properties = Document properties are fixed for now and cannot be edited yet


# Window chrome

chrome-overflow = More Tools

chrome-focus-label-style = Style

chrome-focus-label-paper = Paper

chrome-focus-label-language = Language

chrome-focus-label-font = Font

chrome-focus-label-size = Size

chrome-focus-value-style = Thesis

chrome-focus-value-language = Chinese

chrome-focus-math = Formula

chrome-focus-math_hint = The caret is in a formula; press → at its end or ← at its start to leave

chrome-mode-group = Work Mode

chrome-tab-untitled = Untitled

chrome-tab-unsaved = Unsaved changes

chrome-tab-saved = Saved

chrome-tab-list = Document tabs

chrome-status-words = { $count ->
    [one] { $count } word
   *[other] { $count } words
}

chrome-status-page = Page { $page } of { $pages }

chrome-status-context-body = Body

chrome-status-context-math = Formula

chrome-status-context-source = Source · read-only

chrome-status-revisions = Typst Content · body r{ $body } · view { $shown }

chrome-status-backend = Typst · Synced

chrome-status-updating = Typst · Updating

chrome-status-pending = updating

chrome-status-copy = Copy

chrome-status-copy_hint = Copy the rejected input

chrome-status-zoom = Zoom

chrome-window-title = { $name } — Scholium · { $state }

chrome-dialog-title = Continuing will close the current document

chrome-dialog-body = Unsaved changes will be discarded. Saving a new document again replaces the current contents of the isolated session file.

chrome-dialog-cancel = Cancel

chrome-dialog-discard = Discard and Continue

# Source view

source-header = Generated from document · Read-only

source-warning = Empty slots: { $holes } (shown as □; rejected by strict output). Raw formulas: { $raw } (display only; not executed in the preview).

source-preview-compiling = Compiling preview

source-preview-failed = Preview failed: { $error }

source-preview-ready = A4 preview · page { $page }/{ $pages } · { $ms } ms

diag-severity-error = Error

diag-severity-warning = Warning

diag-severity-info = Note

diag-count = { $count ->
    [one] { $count } issue
   *[other] { $count } issues
}

diag-count_hint = Show the diagnostics panel

diag-edit = { $detail }

diag-save = Save failed: { $error }

diag-layout_failed = Layout failed: { $error }

diag-notice = { $detail }

diag-rejected = Rejected input was kept

diag-selection-unsupported = The selection geometry does not support this range; page edits are paused.

diag-selection-estimated = Selection boundaries inside a ligature are estimated; the mouse only places exact boundaries.

diag-formula_overflow = Formula is wider than the line

diag-caret_missing = The caret cannot be shown here for now

diag-panel-problems = Problems

diag-panel-empty = No problems

diag-panel-layout = Layout

diag-panel-backend = Backend: { $name }

diag-panel-body = Body r{ $revision }

diag-panel-shown = View { $revision }

diag-panel-content = Content built { $built } / reused { $reused } · source reads { $reads }

diag-panel-timing = Layout + raster { $ms } ms

diag-panel-row = { $severity }: { $message }

diag-input-line_geometry = No verified line geometry is available here; use Left/Right or Tab.

diag-input-math_selection = Selection endpoints must be in body text; selection inside math slots is not yet supported.

diag-input-read_only = The structure view is read-only; return to the editor before editing body text.

# Math structure edge announcements never contribute to body Text offsets.
a11y-math-edge-fraction-before = In formula, before fraction
a11y-math-edge-fraction-after = In formula, after fraction
a11y-math-edge-attach-before = In formula, before scripts
a11y-math-edge-attach-after = In formula, after scripts
a11y-math-edge-nested = { $path }, { $edge }

a11y-display-formula = Display formula: { $body }
a11y-math-empty = empty
a11y-math-formula-name = Formula
a11y-math-formula = Formula: { $body }
a11y-math-context = { $path }: { $content }
a11y-math-fraction = Fraction (numerator: { $numerator }; denominator: { $denominator })
a11y-math-attach = Scripts (base: { $base }{ $scripts })
a11y-math-attach-sub = ; subscript: { $content }
a11y-math-attach-sup = ; superscript: { $content }
a11y-math-slot-numerator = Fraction numerator
a11y-math-slot-denominator = Fraction denominator
a11y-math-slot-base = Base
a11y-math-slot-sub = Subscript
a11y-math-slot-sup = Superscript
command-insert-root_index =
    .title = Add root index
    .tooltip = Add or enter the current root index
command-insert-cycle_delimiter =
    .title = Cycle brackets
    .tooltip = Cycle the current brackets through the eight supported pairs
reason-not_in_root = Move the caret into a root first
reason-not_in_delimiter = Move the caret into brackets first
a11y-math-root-square = Root (radicand: { $radicand })
a11y-math-root-indexed = Root (index: { $index }; radicand: { $radicand })
a11y-math-delimited = Brackets ({ $body })
a11y-math-slot-root_index = Root index
a11y-math-slot-radicand = Radicand
a11y-math-slot-delimited = Brackets
a11y-math-edge-root-before = In formula, before root
a11y-math-edge-root-after = In formula, after root
a11y-math-edge-delimited-before = In formula, before brackets
a11y-math-edge-delimited-after = In formula, after brackets

command-edit-select_all =
    .title = Select all
    .tooltip = Select all body text

math-input-command = Math command
math-input-no_candidates = No matching commands
math-input-accept_hint = Enter / Tab: insert · ↑↓: choose · Esc: cancel
math-input-tab-complete = Tab: complete
math-input-tab-cycle = Tab: cycle symbol
math-input-tab-slot = Tab: next slot

# Project files and formal storage
project-source-authority = Typst source · source file authority
project-locked = This project is already open in another window or process. Close it there first.
project-conflict = Modified externally
project-conflict-help = Reload replaces your unsaved changes with the disk file; Save As keeps your current content in a new project.
project-entry-required = Choose an explicit .tex or .typ entry file.
project-entry-optional = Entry (optional, relative to the project; required for ambiguous folders)
project-open-failed = Project operation failed
project-ok = OK
project-reload = Reload
project-recent-empty = No recent projects
project-path-open = Project folder or .tex / .typ file path
project-path-new = New project folder path (must not exist yet)
project-language = Project language (fixed at creation)
project-welcome = Create or open a project from the File menu
project-migrated-title = Session migrated
project-migrated-body = An independent project was created and the original session was retained. Migration details are available in the project folder.
project-source-context = Source authority
project-import = Migrate an old session…
project-import-target = New project folder for the imported session

# LaTeX source workbench
latex-workspace = LaTeX source workbench
latex-source = LaTeX source · unsaved edits
latex-preview = Same-engine preview
latex-unavailable = Toolchain unavailable
latex-building = Compiling preview
latex-stale = Preview updating · source changed
latex-full = Full build { $seconds } s
latex-live = Live { $ms } ms
source-editor-find = Find
preview-page_editing_note = Page formula editing arrives in stage 3.
preview-failed = Preview failed. Your source is retained.
preview-retry = Retry preview
preview-retained = Showing version { $revision }
preview-problems = View { $count } problems
preview-ready = Preview · page { $page }/{ $pages } · { $ms } ms

chrome-dialog-close = Close dialog

chrome-status-position = Ln { $line }, Col { $column }

chrome-status-no_problems = No problems

chrome-status-engine_hint = Preview uses the project language engine


chrome-toolbar-paragraph_hint = Choose the paragraph style for the current block
chrome-toolbar-overflow_hint = Show toolbar actions that do not fit in the window

# Start page and project dialogs
project-start-description = Scientific writing and typesetting
project-start-begin = Start
project-start-recent = Recent
project-start-environment = Environment
project-start-closed = No project open
project-retry = Retry
project-locked-title = This project is open in another window or application
project-recent-remove = Remove from recent projects
project-new-create = Create
project-new-name = Name
project-new-location = Location
project-browse = Browse…
project-new-template = Template
project-new-article = Blank article
project-new-ctex = Chinese thesis (ctexart)
project-new-note = Blank note
project-new-latex = LaTeX — Edit .tex source with same-engine preview
project-new-typst = Typst — Edit directly on the typeset page
project-new-preview = This file will be created. Change the engine in Project Settings.
project-new-name_error = Enter a project name without path separators.
project-new-exists = This folder already exists. Choose another name.
project-new-location_error = Choose an existing, writable folder.
project-picker-title = Choose a folder or file
project-picker-recent = Recent folders
project-picker-error = Choose an existing folder or a .tex / .typ file.
project-settings-entry = Main file
project-settings-engine = Engine
project-settings-engine_help = LuaLaTeX previews paragraphs live; XeLaTeX and pdfLaTeX use full compilation.
project-settings-bibliography = Bibliography tool
project-settings-language = Project language (fixed at creation)
project-settings-auto = Automatic (source or build profile)
project-settings-none = None
project-settings-save_first = Save or discard current changes before switching the main file.
project-settings-failed = Settings were not saved. Check permissions or external changes and try again.
project-conflict-title = { $file } changed externally · { $when }
project-conflict-diff = View Diff
project-conflict-refused = Another program changed the file on disk, so this save was refused to avoid overwriting it.
project-conflict-time_unknown = change time unknown
project-conflict-read_error = The disk file could not be read. Your draft is retained.
project-environment-tex = TeX distribution
project-environment-missing = Not found
project-environment-bundled = Bundled with the app; integrity verified
project-environment-checking = Checking the environment…
project-environment-refresh = Refresh
toolchain-executable-unavailable = The application executable could not be located.
toolchain-reinstall = Reinstall Scholium.
toolchain-bundle-check-timeout = The live-engine check timed out.
toolchain-refresh = Refresh the environment check.
toolchain-detection-failed = Environment detection failed.
toolchain-bundle-missing = Bundled rtex was not found.
toolchain-bundle-invalid = The rtex bundle is incomplete or failed integrity checks.
toolchain-bundle-pin-mismatch = The rtex version does not match this application.
toolchain-override-invalid = The development rtex override is invalid.
toolchain-fix-dev-override = Check the development override paths.
toolchain-tex-missing = No TeX distribution was found.
toolchain-install-tex = Install TeX Live, MacTeX or MiKTeX.
toolchain-program-unavailable = { $program } is unavailable.
toolchain-install-tex-component = Install the corresponding TeX component.
toolchain-font-cache-cold = The font cache is not ready yet.
toolchain-warm-on-preview = It will be prepared automatically on the first preview.
toolchain-dev-override = A development live-engine override is active.
toolchain-remove-dev-override = Remove development overrides to use the installed bundle.

command-file-import =
    .title = Import from Older Version…
    .tooltip = Import an older session as an independent project

command-document-project_settings =
    .title = Project Settings…
    .tooltip = Choose the main file, engine and bibliography tool

project-new-title = New Project
project-settings-title = Project Settings
project-conflict-today = Today
project-conflict-yesterday = Yesterday
project-environment-rtex = Live preview engine rtex
project-environment-invalid = Incomplete or invalid installation
project-environment-fonts_ready = Font cache ready
project-environment-separator = { ", " }
