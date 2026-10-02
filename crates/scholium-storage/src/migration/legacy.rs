//! Reject unsupported legacy data instead of dropping fields through permissive DTOs.
use crate::{StructuredStoreError, structured::MAX_JSON_BYTES};
use rusqlite::Connection;
use scholium_model::{Block, BlockKind, DocumentId, DocumentSnapshot, Inline, NodeId, Revision};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyDocument {
    document: DocumentId,
    revision: Revision,
    blocks: Vec<LegacyBlock>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyBlock {
    node: NodeId,
    kind: BlockKind,
    content: Vec<Inline>,
}

pub(super) fn decode(json: &str) -> Result<DocumentSnapshot, StructuredStoreError> {
    if json.len() > MAX_JSON_BYTES {
        return Err(StructuredStoreError::Encoding);
    }
    let old: LegacyDocument =
        serde_json::from_str(json).map_err(|_| StructuredStoreError::Legacy)?;
    Ok(DocumentSnapshot {
        document: old.document,
        revision: old.revision,
        blocks: old
            .blocks
            .into_iter()
            .map(|b| Block {
                node: b.node,
                kind: b.kind,
                content: b.content,
            })
            .collect(),
    })
}

pub(super) fn validate_schema(db: &Connection) -> Result<(), StructuredStoreError> {
    let mut stmt = db.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT GLOB 'sqlite_*' ORDER BY name")?;
    let tables: Vec<String> = stmt
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    if tables != ["action_log", "meta", "snapshots"] {
        return Err(StructuredStoreError::Schema);
    }
    for (table, expected) in [
        ("action_log", ["idx", "request"]),
        ("meta", ["key", "value"]),
        ("snapshots", ["revision", "json"]),
    ] {
        let mut columns = db.prepare("SELECT name FROM pragma_table_xinfo(?1) ORDER BY cid")?;
        let actual: Vec<String> = columns
            .query_map([table], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        if actual != expected {
            return Err(StructuredStoreError::Schema);
        }
    }
    let unsupported: i64 = db.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type IN ('view', 'trigger')",
        [],
        |r| r.get(0),
    )?;
    let meta: i64 = db.query_row(
        "SELECT COUNT(*) FROM meta WHERE key != 'head_revision'",
        [],
        |r| r.get(0),
    )?;
    if unsupported != 0 || meta != 0 {
        return Err(StructuredStoreError::Schema);
    }
    Ok(())
}
