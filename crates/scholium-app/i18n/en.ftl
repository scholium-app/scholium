# Generated catalog layout: see ADR-0040. Keys: command.<id>.title|tooltip, menu.<id>, reason.<name>.

# Command titles and tooltips

command-file-new =
    .title = New
    .tooltip = Start a new document

command-file-open =
    .title = Open…
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

command-insert-display_formula =
    .title = Display Formula
    .tooltip = Insert a formula on its own line

command-insert-fraction =
    .title = Fraction
    .tooltip = Insert a fraction; Tab moves between slots

command-insert-root =
    .title = Radical
    .tooltip = Insert a radical

command-insert-attach =
    .title = Superscript/Subscript
    .tooltip = Insert a superscript or subscript

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

command-view-mode-visual =
    .title = WYSIWYG
    .tooltip = Edit on the page · Ctrl+Shift+M to switch

command-view-mode-source =
    .title = Source Preview
    .tooltip = Source and preview · Ctrl+Shift+M to switch

command-view-mode-toggle =
    .title = Switch Work Mode
    .tooltip = Switch between WYSIWYG and source

command-view-navigation =
    .title = File Navigation
    .tooltip = Show or hide file navigation

command-view-diagnostics =
    .title = Diagnostics
    .tooltip = Show diagnostics

command-view-split =
    .title = Split View
    .tooltip = Show source and preview side by side

command-view-ribbon =
    .title = Ribbon
    .tooltip = Show or hide the Ribbon

command-view-theme =
    .title = Theme
    .tooltip = Switch between light and dark

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


# Availability reasons

reason-no_document = No document is open

reason-source_readonly = The source view is read-only; switch back to WYSIWYG first

reason-composing = Finish the IME composition first

reason-no_selection = Select something first

reason-nothing_to_undo = Nothing to undo

reason-nothing_to_redo = Nothing to redo

reason-unimplemented = This feature is not available yet

reason-final_build = The final build is not available yet
