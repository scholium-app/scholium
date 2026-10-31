# Shared editor scenarios

Each file is a deterministic sequence of abstract inputs. `target` values are semantic
locations resolved from the current Scene, so a headless replay and the real-window parity
check address the same document object. The expected object records the stable digest,
caret/anchor and local undo depth used by both runners.

`expected` has four required fields: `snapshot`, `cursor`, `anchor`, `undo_depth`.
Both runners read `EditorCore::semantic_projection()` (the window through AUDIT's
`semantic` field), so the stable summary has one implementation. `snapshot` retains
all content and styles, removing only random `node`/`document` identities and the
revision counter. Endpoints use the node's slash-separated tree path, leaf-local
UTF-8 `byte` and `affinity`; an absent anchor is JSON null. No fields or thresholds
are relaxed for the window.

One `text` input is one committed string. The window uses clipboard paste for CJK
and strings containing whitespace, and xdotool for short ASCII and printable keys.
Pointer `clicks` denotes the click count of the gesture, including on `Release`;
headless does not need to synthesize preceding native presses to select a word or
paragraph. Window coordinates come from the current AUDIT's exact caret positions.
Unknown inputs/keys, missing windows, timeouts and expected differences fail the check.
