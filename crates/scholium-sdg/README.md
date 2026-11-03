# Structured SDG adapter and engine comparison

`Replica` is a layer 2 Loro 1.17.1 authority. It plans existing `StructuralEdit`s or
lowers validated effects, emits versioned Actions, merges remote Actions, reports
schema repairs, provides actor-local undo/redo and snapshots with replay. It depends
on model/document; session/app/storage integration is external. See
[ADR 0053](../../docs/adr/ADR-0053-loro-sdg-action-mapping.md) for the permanent format,
identity mapping, repair rules, capacity limits and restart/undo boundary.

The four-version experiment and the coordinator's acceptance criteria are in
[ADR 0009](../../docs/adr/ADR-0009-crdt-engine.md). Both criteria passed for 1.17.1.
50k actions per side merged once remains an expensive offline edge case, especially
for isolated node create/wrap/unwrap workloads. The comparison does not measure the
adapter's full-document planning/projection, UI or durable storage latency.

## Reproduction

Run from the repository root with fresh output directories:

```sh
cargo test --locked -p scholium-sdg --all-targets
cargo build --release --locked --manifest-path crates/scholium-sdg/bench-legacy/Cargo.toml \
  --target-dir target/t41-legacy
cargo build --release --locked --manifest-path crates/scholium-sdg/bench-current/Cargo.toml \
  --target-dir target/t41-current
python3 crates/scholium-sdg/examples/run-collaboration-bench.py \
  target/t41-legacy/release/collaboration_bench \
  target/t41-current/release/collaboration_bench target/collaboration-new
python3 crates/scholium-sdg/examples/run-collaboration-bench.py \
  target/t41-legacy/release/collaboration_bench \
  target/t41-current/release/collaboration_bench target/breakdown-new --breakdown
python3 crates/scholium-sdg/examples/run-engine-bench.py \
  target/t41-legacy/release/engine_bench target/throughput-legacy-new --repeats 3
python3 crates/scholium-sdg/examples/run-engine-bench.py \
  target/t41-current/release/engine_bench target/throughput-current-new --repeats 3
cargo build --locked -p scholium-sdg --target wasm32-unknown-unknown
```

The extra threshold stress run uses latest Loro with 50k actions per side and
batch 100 (the main matrix uses 10k/side for batch 100):

```sh
python3 crates/scholium-sdg/examples/run-collaboration-bench.py \
  target/t41-legacy/release/collaboration_bench \
  target/t41-current/release/collaboration_bench target/criteria-new --criteria
```

The runner retains each receive, alternates engine order and starts one engine
process at a time. It rejects suspension over one second; discard such a whole
run. Criteria use the worst per-run p99 and worst two-way offline sum over three
repeats, and write an explicit verdict. Shared host load can affect absolute times.
Separate manifests/lockfiles are necessary because Cargo unifies semver-compatible
versions; notably Loro 1.16.0 cannot compile with loro-internal 1.17.1. Legacy pins
both to 1.16.0; current pins API/internal 1.17.1. Sources are shared, never relabelled.
Run standalone fmt, clippy, tests, docs and cargo-deny for both graphs as well.

## Workloads and measurement boundary

The baseline is 32 paragraph roots with body text, inline math, math row and math
text (160 stable identities), Unicode `hello 中文 👩‍🔬` and `é`. A mixed ten-action
cycle inserts/deletes text, creates/deletes a symbol, creates a root wrapper, moves
math text in/out, deletes the wrapper, moves body text to the next paragraph/back.
Actors edit disjoint nodes. A serial identified-state oracle checks parent, kind,
ordered children and literal text; each process also checks convergence/snapshot.
Contract tests inspect intermediate states, real deltas and duplicate imports.

Incremental batches 1/10/100 contain local actions on **both** replicas before either
receives; payloads are generated against the recipient's pre-batch state vector.
Batch 1 uses 1k actions/side, batch 10/100 use 10k/side. A legacy batch-1 10k pilot
was stopped after about ten minutes without a completed sample. The 50k/side latest
batch-100 stress run guards against accepting only a short session. Offline mixed
and isolated operation probes use 1k/10k/50k actions/side and exchange missing deltas
once, excluding shared bootstrap. Each row records each one-way receive, engine
import and index time separately; offline tables report the two-way sum. p50/p99
use nearest-rank over pooled receives. Three independent repeats retain every value.

Isolated categories are text insert/delete, node create/delete, wrap, unwrap and
cross-parent move. Delete targets/content are prepared before timing. Wrap creates
a new wrapper and reparents its content in one committed action; unwrap reparents
out and deletes the wrapper in one action. Text deletion preloads `é`; structural
probes use different content nodes, while cross-parent move alternates two parents.
Pure 50k/side structural probes can exceed the product's 100k visible-node bound;
these are diagnostic engine workloads, not normal document capacity claims.

Loro uses movable tree + metadata + LoroText; Yrs uses stable-id maps with
parent/order/alive + YText and explicit UTF-16 indexing. The Yrs order counter is
safe for this disjoint workload only; it does not prove arbitrary concurrent moves.
No production Yrs adapter is implemented. Loro retains history, Yrs uses default GC;
these are not identical undo/checkpoint guarantees. Loro identity-cache maintenance
reads metadata only for newly seen TreeIDs, while still scanning retained IDs.
The original historical benchmark rebuilt all metadata; its saved evidence remains
unchanged, and it is reproducible at commit 9c8b3cf.

The full throughput probe runs 100k actions total without per-action export or
projection, then sends full history from empty vectors (bootstrap counted twice).
Snapshot is the merged state (Yrs full update v1). RSS is Linux process RSS after
editing and whole-process peak, including traces, codecs and a restored replica;
it is not engine-only heap allocation. The release benchmark representation is
separate from the production SDG mapping and wire format.
