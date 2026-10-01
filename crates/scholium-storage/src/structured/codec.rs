//! Bound JSON before parsing, retaining serde_json's default recursion protection.
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct Encoded<'a> {
    format: &'static str,
    version: u32,
    document: &'a StructuredDocument,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Decoded {
    format: String,
    version: u32,
    document: StructuredDocument,
}

pub(crate) fn encode(snapshot: &StructuredDocument) -> Result<String, StructuredStoreError> {
    let json = serde_json::to_string(&Encoded {
        format: FORMAT,
        version: 1,
        document: snapshot,
    })
    .map_err(|_| StructuredStoreError::Encoding)?;
    if json.len() > MAX_JSON_BYTES {
        return Err(StructuredStoreError::Encoding);
    }
    Ok(json)
}

pub(crate) fn decode(json: &str) -> Result<StructuredDocument, StructuredStoreError> {
    if json.len() > MAX_JSON_BYTES {
        return Err(StructuredStoreError::Encoding);
    }
    let envelope: Decoded =
        serde_json::from_str(json).map_err(|_| StructuredStoreError::Encoding)?;
    if envelope.format != FORMAT || envelope.version != 1 {
        return Err(StructuredStoreError::Schema);
    }
    envelope.document.validate()?;
    Ok(envelope.document)
}

pub(crate) fn read_json(
    db: &Connection,
    table: &str,
    revision: i64,
) -> Result<String, StructuredStoreError> {
    // Table names are private caller constants, never user input.
    let bytes: i64 = db.query_row(
        &format!("SELECT length(CAST(json AS BLOB)) FROM {table} WHERE revision=?1"),
        [revision],
        |r| r.get(0),
    )?;
    if bytes < 0 || bytes as u64 > MAX_JSON_BYTES as u64 {
        return Err(StructuredStoreError::Encoding);
    }
    Ok(db.query_row(
        &format!("SELECT json FROM {table} WHERE revision=?1"),
        [revision],
        |r| r.get(0),
    )?)
}
