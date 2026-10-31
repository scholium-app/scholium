# Shared editor scenarios

Each file is a deterministic sequence of abstract inputs. `target` values are semantic
locations resolved from the current Scene, so a headless replay and the real-window parity
check address the same document object. The expected object records the stable digest,
caret/anchor and local undo depth used by both runners.
