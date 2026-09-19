use super::snapshot::Snapshot;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

const MAX_BYTES: u64 = 16 * 1024 * 1024;

pub(super) struct Store {
    path: PathBuf,
    expected: Option<Vec<u8>>,
    _lock: File,
}

fn invalid(message: impl ToString) -> io::Error {
    io::Error::other(message.to_string())
}

fn read(path: &Path) -> io::Result<Option<Vec<u8>>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(invalid("会话超过 16 MiB"));
    }
    Ok(Some(bytes))
}

impl Store {
    pub fn open(path: PathBuf) -> io::Result<Self> {
        let lock_path = path.with_extension("session-lock");
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(lock_path)?;
        lock.try_lock().map_err(invalid)?;
        Ok(Self {
            path,
            expected: None,
            _lock: lock,
        })
    }

    pub fn load(&mut self) -> io::Result<Option<Snapshot>> {
        let bytes = read(&self.path)?;
        let snapshot = bytes
            .as_ref()
            .map(|b| serde_json::from_slice::<Snapshot>(b))
            .transpose()
            .map_err(invalid)?;
        if let Some(snapshot) = &snapshot {
            snapshot.restore().map_err(invalid)?;
        }
        self.expected = bytes;
        Ok(snapshot)
    }

    pub fn save(&mut self, snapshot: &Snapshot) -> io::Result<()> {
        let bytes = serde_json::to_vec(snapshot).map_err(invalid)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(invalid("会话超过 16 MiB"));
        }
        if read(&self.path)? != self.expected {
            return Err(invalid("文件已被外部修改，停止自动保存"));
        }
        let temporary = self
            .path
            .with_extension(format!("session-{}.tmp", std::process::id()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let result = (|| {
            file.write_all(&bytes)?;
            file.sync_all()?;
            if read(&self.path)? != self.expected {
                return Err(invalid("文件已被外部修改"));
            }
            fs::rename(&temporary, &self.path)?;
            File::open(
                self.path
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or(Path::new(".")),
            )?
            .sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(temporary);
        }
        result?;
        self.expected = Some(bytes);
        Ok(())
    }
}
