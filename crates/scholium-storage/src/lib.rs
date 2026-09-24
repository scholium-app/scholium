//! 本地会话持久化的最小 SQLite 后端（ADR 0028 裁决后的实现）。
//! 单事务保存快照与动作日志；WAL 模式下未提交事务崩溃后不可见，恢复即丢弃。
//! libsqlite3 以系统库经 rusqlite 链接，登记见 docs/NATIVE_DEPENDENCIES.md。

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use scholium_model::{DocumentSnapshot, RequestId, Revision};

/// 持久化会话的还原视图：最新快照与请求身份日志。
pub struct PersistedSession {
    /// 最新内容投影。
    pub snapshot: DocumentSnapshot,
    /// 已接受请求的身份（按接受顺序），用于重复请求检测的恢复。
    pub requests: Vec<RequestId>,
}

/// 打开（或创建）会话数据库。文件本身即唯一事实，无外部状态。
pub struct SessionStore {
    db: Connection,
}

impl SessionStore {
    /// 打开或创建数据库文件。
    ///
    /// # Errors
    /// 文件不可打开/损坏或建表失败时返回错误。
    pub fn open(path: &Path) -> Result<Self, String> {
        let db = Connection::open(path).map_err(|e| e.to_string())?;
        Self::init(db)
    }

    fn init(db: Connection) -> Result<Self, String> {
        // WAL + FULL：显式保存是低频用户动作，换取崩溃/断电安全。
        db.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=FULL;
             CREATE TABLE IF NOT EXISTS snapshots (
                 revision INTEGER PRIMARY KEY, json TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS action_log (
                 idx INTEGER PRIMARY KEY, request BLOB NOT NULL);
             CREATE TABLE IF NOT EXISTS meta (
                 key TEXT PRIMARY KEY, value INTEGER NOT NULL);
             INSERT OR IGNORE INTO meta VALUES ('head_revision', -1);",
        )
        .map_err(|e| e.to_string())?;
        Ok(Self { db })
    }

    /// 原子保存：最新快照 + 完整请求日志 + head 指针写入单事务。
    ///
    /// # Errors
    /// revision 超出 SQLite 整数范围、请求日志长度不匹配、序列化或写入失败时返回错误；
    /// 数据库保持上一个已提交状态。
    pub fn save(&self, snapshot: &DocumentSnapshot, requests: &[RequestId]) -> Result<(), String> {
        let revision = i64::try_from(snapshot.revision.0)
            .map_err(|_| "revision exceeds SQLite integer range".to_owned())?;
        if requests.len() as u64 != snapshot.revision.0 {
            return Err("request log length does not match snapshot revision".into());
        }
        let json = serde_json::to_string(snapshot).map_err(|e| e.to_string())?;
        let tx = self.db.unchecked_transaction().map_err(|e| e.to_string())?;
        {
            let mut statements = Prepared::new(&tx)?;
            statements
                .snap
                .execute((revision, json.as_str()))
                .map_err(|e| e.to_string())?;
            statements.clear.execute(()).map_err(|e| e.to_string())?;
            for (index, request) in requests.iter().enumerate() {
                statements
                    .log
                    .execute((index as i64, request.as_bytes().as_slice()))
                    .map_err(|e| e.to_string())?;
            }
            statements
                .head
                .execute((revision,))
                .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())
    }

    /// 读回最新快照与请求日志；空库返回 `None`。
    ///
    /// # Errors
    /// head 快照缺失、revision 或请求日志不一致、请求身份损坏以及反序列化失败时返回错误。
    pub fn load(&self) -> Result<Option<PersistedSession>, String> {
        let head: Option<i64> = self
            .db
            .query_row(
                "SELECT value FROM meta WHERE key='head_revision'",
                [],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        let Some(head) = head else {
            return Err("missing head revision".into());
        };
        if head == -1 {
            return Ok(None);
        }
        let revision = u64::try_from(head).map_err(|_| "invalid head revision".to_owned())?;
        let snapshot = self
            .snapshot_at(Revision(revision))?
            .ok_or_else(|| "head snapshot is missing".to_owned())?;
        if snapshot.revision != Revision(revision) {
            return Err("head snapshot revision does not match metadata".into());
        }
        let mut statement = self
            .db
            .prepare("SELECT request FROM action_log ORDER BY idx")
            .map_err(|e| e.to_string())?;
        let rows = statement
            .query_map([], |row| {
                let bytes: Vec<u8> = row.get(0)?;
                Ok(bytes)
            })
            .map_err(|e| e.to_string())?;
        let mut requests = Vec::new();
        for row in rows {
            let bytes = row.map_err(|e| e.to_string())?;
            let array: [u8; 16] = bytes
                .try_into()
                .map_err(|_| "invalid request ID length in action log".to_owned())?;
            requests.push(RequestId::from_bytes(array));
        }
        if requests.len() as u64 != revision {
            return Err("request log length does not match head revision".into());
        }
        Ok(Some(PersistedSession { snapshot, requests }))
    }

    /// 快照按 revision 直读（恢复旧版本/诊断用）。
    ///
    /// # Errors
    /// revision 超出 SQLite 整数范围、读取或反序列化失败时返回错误。
    pub fn snapshot_at(&self, revision: Revision) -> Result<Option<DocumentSnapshot>, String> {
        let revision = i64::try_from(revision.0)
            .map_err(|_| "revision exceeds SQLite integer range".to_owned())?;
        let row = self.db.query_row(
            "SELECT json FROM snapshots WHERE revision = ?1",
            [revision],
            |row| row.get::<_, String>(0),
        );
        match row {
            Ok(json) => serde_json::from_str(&json)
                .map(Some)
                .map_err(|e| e.to_string()),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }
}

struct Prepared<'a> {
    snap: rusqlite::Statement<'a>,
    log: rusqlite::Statement<'a>,
    head: rusqlite::Statement<'a>,
    clear: rusqlite::Statement<'a>,
}

impl<'a> Prepared<'a> {
    fn new(tx: &'a rusqlite::Transaction<'_>) -> Result<Self, String> {
        Ok(Self {
            snap: tx
                .prepare("INSERT OR REPLACE INTO snapshots VALUES (?1, ?2)")
                .map_err(|e| e.to_string())?,
            log: tx
                .prepare("INSERT INTO action_log VALUES (?1, ?2)")
                .map_err(|e| e.to_string())?,
            head: tx
                .prepare("UPDATE meta SET value=?1 WHERE key='head_revision'")
                .map_err(|e| e.to_string())?,
            clear: tx
                .prepare("DELETE FROM action_log")
                .map_err(|e| e.to_string())?,
        })
    }
}

/// 默认会话文件位置（XDG 数据目录）。
///
/// # Errors
/// 无 HOME 时返回错误。
pub fn default_session_path() -> Result<PathBuf, String> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .ok_or_else(|| "no home directory".to_owned())?;
    let dir = base.join("scholium");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("session.scholium.sqlite"))
}

#[cfg(test)]
pub(crate) mod test_support {
    use scholium_model::{
        Block, BlockKind, DocumentId, DocumentSnapshot, Inline, NodeId, Revision,
    };
    use std::path::PathBuf;

    pub(crate) fn sample(revision: u64, text: &str) -> DocumentSnapshot {
        DocumentSnapshot {
            document: DocumentId::fresh(),
            revision: Revision(revision),
            blocks: vec![Block {
                node: NodeId::fresh(),
                kind: BlockKind::Paragraph,
                content: vec![Inline::Text(text.into()), Inline::Math("x/2".into())],
            }],
        }
    }

    pub(crate) fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "scholium-storage-{name}-{}.sqlite",
            std::process::id()
        ))
    }
}

#[cfg(test)]
mod tests;
