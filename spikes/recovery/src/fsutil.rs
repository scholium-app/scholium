//! 文件系统小工具：内容哈希、目录列举、抢救目录、排他锁。
//!
//! 这些能力在三个判据里都要用，抽在这里而不是各写一份。刻意保持薄：
//! 每个函数只做一件事，没有一个函数接触"判据"的概念。

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{Result, SpikeError, io_context};

/// 进程内自增序号，用来给临时目录命名。
static SCRATCH_SEQ: AtomicU64 = AtomicU64::new(0);

/// 计算字节内容的 SHA-256，返回小写十六进制。
///
/// 用 SHA-256 而不是 CRC：外部源码修改检测要对抗的是"故意构造的哈希碰撞"，
/// 虽然本 spike 不假设敌手，但冲突检测的强度不该由验证代码随意降低。
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// 读文件并计算内容哈希。
pub fn hash_file(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).map_err(io_context(path))?;
    Ok(sha256_hex(&bytes))
}

/// 写文件，必要时创建父目录。
pub fn write_file(path: &Path, contents: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(io_context(parent))?;
    }
    std::fs::write(path, contents).map_err(io_context(path))
}

/// 读文件。
pub fn read_file(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(io_context(path))
}

/// 列目录下的文件名（不递归），按名称排序；目录不存在时返回空。
pub fn list_names(dir: &Path) -> Result<Vec<String>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let entries = std::fs::read_dir(dir).map_err(io_context(dir))?;
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(io_context(dir))?;
        if let Some(name) = entry.file_name().to_str() {
            names.push(name.to_string());
        }
    }
    names.sort();
    Ok(names)
}

/// 抢救目录：为一次运行创建的临时根目录。
///
/// `SCHOLIUM_SPIKE_KEEP=1` 时保留现场并打印路径，否则在 `Drop` 时递归删除。
/// CI 里默认删除，排查时才保留——否则证据只存在于日志里，看不到真实文件树。
#[derive(Debug)]
pub struct Scratch {
    root: PathBuf,
    keep: bool,
}

impl Scratch {
    /// 在 `parent` 下创建唯一目录，名字带进程号与自增序号。
    pub fn create(parent: &Path, label: &str) -> Result<Self> {
        let seq = SCRATCH_SEQ.fetch_add(1, Ordering::Relaxed);
        let root = parent.join(format!("{label}-{}-{seq}", std::process::id()));
        std::fs::create_dir_all(&root).map_err(io_context(&root))?;
        Ok(Self {
            root,
            keep: std::env::var_os("SCHOLIUM_SPIKE_KEEP").is_some(),
        })
    }

    /// 临时根目录。
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 在根目录下解析子路径（不检查存在）。
    pub fn join(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if self.keep {
            println!("  [----] 保留现场: {}", self.root.display());
            return;
        }
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// 排他文件锁：第二次获取立即失败，而不是阻塞。
///
/// 判据"构建 A 不读 B 的中间文件"必须包含"并发不会互相覆盖"，
/// 因此锁的语义是**拒绝并发写同一目录**，而不是排队。锁文件留在目录里，
/// 释放时删除；进程被杀死时锁文件残留，不需要额外堆栈或 pid 记录，
/// 因为本 spike 只在一个进程里并发，崩溃进程不持有目录锁。
#[derive(Debug)]
pub struct DirLock {
    path: PathBuf,
}

impl DirLock {
    /// 在 `dir` 下创建 `name` 锁文件。
    ///
    /// # Errors
    ///
    /// 目录不可写，或锁已被同一进程/另一进程持有。
    pub fn acquire(dir: &Path, name: &str) -> Result<Self> {
        let path = dir.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                let _ = writeln!(file, "pid={}", std::process::id());
                Ok(Self { path })
            }
            Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
                Err(SpikeError::Core(format!(
                    "构建目录已被占用: {}",
                    path.display()
                )))
            }
            Err(source) => Err(io_context(&path)(source)),
        }
    }

    /// 锁文件路径，错误消息里要写清楚是谁占用了。
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for DirLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// 写文件并 `fsync`，返回写入字节数。
///
/// WAL 追加路径必须显式 `sync_all`：只 `write` 不 `sync` 的话，
/// 进程被 SIGKILL 后数据是否落在磁盘上取决于页缓存，崩溃语义就不成立了。
pub fn append_synced(file: &mut File, bytes: &[u8]) -> Result<usize> {
    file.write_all(bytes).map_err(io_context("<wal>"))?;
    file.sync_all().map_err(io_context("<wal>"))?;
    Ok(bytes.len())
}
