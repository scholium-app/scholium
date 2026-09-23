//! 本地会话持久化的最小 redb 后端（ADR 0028 拟议 schema 的首个实现）。
//! 单写事务保存快照与动作日志；未提交事务在崩溃后整体不可见，恢复即丢弃。

use std::path::{Path, PathBuf};

use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use scholium_model::{DocumentSnapshot, RequestId, Revision};

/// 会话元数据表：schema 版本与文档身份。
const META: TableDefinition<&str, u64> = TableDefinition::new("meta");
/// 快照表：revision → 序列化投影。
const SNAPSHOTS: TableDefinition<u64, &str> = TableDefinition::new("snapshots");
/// 动作日志表：单调序号 → 已接受请求身份（append-only）。
const ACTION_LOG: TableDefinition<u64, &[u8]> = TableDefinition::new("action_log");

/// 持久化会话的还原视图：最新快照与请求身份日志。
pub struct PersistedSession {
    /// 最新内容投影。
    pub snapshot: DocumentSnapshot,
    /// 已接受请求的身份（按接受顺序），用于重复请求检测的恢复。
    pub requests: Vec<RequestId>,
}

/// 打开（或创建）会话数据库。文件本身即唯一事实，无外部状态。
pub struct SessionStore {
    db: Database,
}

impl SessionStore {
    /// 打开或创建数据库文件；`in_memory` 仅供测试。
    ///
    /// # Errors
    /// 文件不可创建/损坏或 redb 版本不符时返回错误。
    pub fn open(path: &Path) -> Result<Self, String> {
        let db = Database::create(path).map_err(|e| e.to_string())?;
        let write = db.begin_write().map_err(|e| e.to_string())?;
        {
            let mut meta = write.open_table(META).map_err(|e| e.to_string())?;
            if meta.get("schema_version").ok().flatten().is_none() {
                meta.insert("schema_version", 1)
                    .map_err(|e| e.to_string())?;
            }
            let _ = write.open_table(SNAPSHOTS).map_err(|e| e.to_string())?;
            let _ = write.open_table(ACTION_LOG).map_err(|e| e.to_string())?;
        }
        write.commit().map_err(|e| e.to_string())?;
        Ok(Self { db })
    }

    /// 占位库（打开失败时的兜底）：临时文件，进程结束即可丢弃。
    ///
    /// # Errors
    /// 临时文件创建失败时返回错误。
    ///
    /// # Panics
    /// 事务初始化失败时 panic（临时文件上的初始化失败不可恢复）。
    pub fn open_scratch() -> Result<Self, String> {
        let path =
            std::env::temp_dir().join(format!("scholium-scratch-{}.redb", std::process::id()));
        let db = Database::create(&path).map_err(|e| e.to_string())?;
        let write = db.begin_write().expect("in-memory write");
        {
            let mut meta = write.open_table(META).expect("meta table");
            let _ = meta.insert("schema_version", 1);
            let _ = write.open_table(SNAPSHOTS);
            let _ = write.open_table(ACTION_LOG);
        }
        write.commit().map_err(|e| e.to_string())?;
        Ok(Self { db })
    }

    /// 原子保存：最新快照 + 完整请求日志写入单事务，中途崩溃不留半个状态。
    ///
    /// # Errors
    /// 序列化或写入失败时返回错误；数据库保持上一个已提交状态。
    pub fn save(&self, snapshot: &DocumentSnapshot, requests: &[RequestId]) -> Result<(), String> {
        let json = serde_json::to_string(snapshot).map_err(|e| e.to_string())?;
        let write = self.db.begin_write().map_err(|e| e.to_string())?;
        {
            let mut snapshots = write.open_table(SNAPSHOTS).map_err(|e| e.to_string())?;
            snapshots
                .insert(snapshot.revision.0, json.as_str())
                .map_err(|e| e.to_string())?;
            let mut meta = write.open_table(META).map_err(|e| e.to_string())?;
            meta.insert("head_revision", snapshot.revision.0)
                .map_err(|e| e.to_string())?;
            let mut log = write.open_table(ACTION_LOG).map_err(|e| e.to_string())?;
            let mut stale_keys = Vec::new();
            for entry in log.iter().map_err(|e| e.to_string())? {
                let (key, _) = entry.map_err(|e| e.to_string())?;
                if key.value() >= requests.len() as u64 {
                    stale_keys.push(key.value());
                }
            }
            for key in stale_keys {
                log.remove(key).map_err(|e| e.to_string())?;
            }
            for (index, request) in requests.iter().enumerate() {
                log.insert(index as u64, request.as_bytes().as_slice())
                    .map_err(|e| e.to_string())?;
            }
        }
        write.commit().map_err(|e| e.to_string())
    }

    /// 读回最新快照与请求日志；空库返回 `None`。
    ///
    /// # Errors
    /// 反序列化失败（schema 不兼容）时返回错误。
    pub fn load(&self) -> Result<Option<PersistedSession>, String> {
        let read = self.db.begin_read().map_err(|e| e.to_string())?;
        let snapshots = read.open_table(SNAPSHOTS).map_err(|e| e.to_string())?;
        let meta = read.open_table(META).map_err(|e| e.to_string())?;
        let head = meta
            .get("head_revision")
            .map_err(|e| e.to_string())?
            .map(|guard| guard.value());
        let key = match head {
            Some(revision) => revision,
            // 兼容无 head 元数据的旧库：取最大 revision。
            None => {
                let mut iterator = snapshots.iter().map_err(|e| e.to_string())?;
                match iterator
                    .next_back()
                    .transpose()
                    .map_err(|e| e.to_string())?
                {
                    Some(entry) => entry.0.value(),
                    None => return Ok(None),
                }
            }
        };
        let Some(json) = snapshots.get(key).map_err(|e| e.to_string())? else {
            return Ok(None);
        };
        let snapshot: DocumentSnapshot =
            serde_json::from_str(json.value()).map_err(|e| e.to_string())?;
        let log = read.open_table(ACTION_LOG).map_err(|e| e.to_string())?;
        let mut requests = Vec::new();
        for entry in log.iter().map_err(|e| e.to_string())? {
            let (_, value) = entry.map_err(|e| e.to_string())?;
            requests.push(from_slice(value.value()));
        }
        Ok(Some(PersistedSession { snapshot, requests }))
    }

    /// 快照按 revision 直读（恢复旧版本/诊断用）。
    ///
    /// # Errors
    /// 读取或反序列化失败时返回错误。
    pub fn snapshot_at(&self, revision: Revision) -> Result<Option<DocumentSnapshot>, String> {
        let read = self.db.begin_read().map_err(|e| e.to_string())?;
        let snapshots = read.open_table(SNAPSHOTS).map_err(|e| e.to_string())?;
        match snapshots.get(revision.0).map_err(|e| e.to_string())? {
            Some(json) => {
                let snapshot: DocumentSnapshot =
                    serde_json::from_str(json.value()).map_err(|e| e.to_string())?;
                Ok(Some(snapshot))
            }
            None => Ok(None),
        }
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
    Ok(dir.join("session.scholium"))
}

// RequestId 是不透明新类型；以稳定字节形式入库。
fn from_slice(bytes: &[u8]) -> RequestId {
    let array: [u8; 16] = bytes.try_into().unwrap_or([0; 16]);
    RequestId::from_bytes(array)
}

#[cfg(test)]
mod tests;
