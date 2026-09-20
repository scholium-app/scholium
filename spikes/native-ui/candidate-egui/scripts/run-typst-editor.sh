#!/usr/bin/env bash
# Build both executables before starting the native editor.
set -euo pipefail
project_root="$(cd "$(dirname "$0")/../../../.." && pwd)"
export CARGO_HOME="${CARGO_HOME:-$project_root/spikes/native-ui/.cargo-home}"
cargo build --offline --release --manifest-path "$project_root/spikes/typst-mapping/Cargo.toml"
cargo build --offline --manifest-path "$project_root/spikes/native-ui/candidate-egui/Cargo.toml"
export SCHOLIUM_EDITOR_RENDERER="$project_root/spikes/typst-mapping/target/release/scholium-spike-typst"
export SCHOLIUM_TYPST_BIN="${SCHOLIUM_TYPST_BIN:-$project_root/spikes/mixed-build/out/packages/toolchain/typst}"
exec "$project_root/spikes/native-ui/candidate-egui/target/debug/scholium-spike-egui"
