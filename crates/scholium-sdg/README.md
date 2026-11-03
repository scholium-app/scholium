# SDG engine comparison boundary

This crate currently exposes **no production adapter or Action payload API**. T41's
performance stop condition was triggered; see [ADR 0009](../../docs/adr/ADR-0009-crdt-engine.md).
The reserved layer is 2; a future adapter may depend on model/document, not session,
storage, app or a rendering engine. Loro remains a dependency so the WASM gate also
compiles the actual engine. Yrs is a dev-dependency used only by the comparison.

Reproduce from the repository root, using a new output directory:

```sh
cargo test --locked -p scholium-sdg --example engine_bench
cargo build --release --locked -p scholium-sdg --example engine_bench
python3 crates/scholium-sdg/examples/run-engine-bench.py \
  target/release/examples/engine_bench target/engine-bench-new
cargo build --locked -p scholium-sdg --target wasm32-unknown-unknown
```

The runner starts one engine process at a time, alternating order over five repeats.
It saves every raw result and rejects any sample with a monotonic/boottime delta over
one second; discard the whole run after suspension. Other executors can still run on
the host, so absolute times are descriptive, not an isolated latency budget.

The fixture has 32 paragraph roots, each with a Unicode body text, inline math,
math row and math text (160 stable benchmark identities). Each replica executes
50,000 actions on disjoint node sets: one ten-action cycle inserts/deletes `é`,
creates/deletes a symbol, creates a radical wrapper, moves the existing math text
into and out of it, deletes the wrapper, and moves the body inline to the next
paragraph and back. Bootstrap is outside the throughput timer. No full-document
projection or update encoding occurs per action; this is an engine benchmark, not
an editor or durable Action throughput claim.

Loro uses movable tree + node metadata + LoroText; Yrs uses stable-id maps with
parent/order/alive fields + YText. Text removal is one Unicode scalar in Loro and
one UTF-16 code unit in Yrs (`é`); Yrs explicitly selects UTF-16 indexing. Both
must preserve Chinese and the full emoji sequence. The order counter is safe for
this disjoint trace only: it does not prove Yrs arbitrary concurrent move semantics.
Root ordering never changes; ordered children of each addressed node are compared.

Every process asserts both replicas converge on the complete expected identified
state (kind, parent, ordered children, literal text), including the final inline
reordering, and that a fresh replica imports the snapshot to the same state.
Preflight tests compare both backends at intermediate insert/delete/wrap/move
states, not just a final text checksum. These are benchmark-contract tests, not
production structured-document invariant or actor-undo acceptance.

Update bytes are full history from an empty state vector for each source (shared
bootstrap counted twice). Snapshot is the merged state; Yrs uses full update v1
and its default GC, Loro uses Snapshot and retains its operation history. These
are their default retention policies, not equal checkpoint/undo guarantees.
Merge timing separates engine decode/import from local cache rebuilding and
excludes state projection. Snapshot import includes rebuilding the local cache.
RSS is Linux `/proc/self/status` for the whole process: after-edit RSS includes
both replicas and prepared traces; peak includes codecs and a third restored
replica. It is not an allocator-only engine-memory measurement.

The Loro API is locked at 1.16.0 and internal at 1.16.0 (1.17.1 internal fails to
compile with this API); common/kv-store resolve to 1.17.0. Use `--locked`, as
Cargo.lock is the exact reproducible dependency graph. No production code should
consume this benchmark representation as a permanent SDG schema.
