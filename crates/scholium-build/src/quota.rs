//! A kernel-bounded tmpfs, retained by an fd after the allocator namespace exits.
use crate::{BuildError, ResourceLimits};
use nix::sys::{
    socket::{ControlMessageOwned, MsgFlags, recvmsg},
    statvfs::statvfs,
};
use std::{
    fs::File,
    io::{IoSliceMut, Read},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::net::UnixDatagram,
    },
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[derive(Debug)]
pub(crate) struct Volume {
    _root: File,
    pub namespace: File,
    pub mount_namespace: File,
    pub stage: tempfile::TempDir,
    pub path: PathBuf,
    pub exhausted: Arc<AtomicBool>,
    pub budget: (u64, u64),
}
impl Volume {
    pub fn create(limits: ResourceLimits) -> Result<Self, BuildError> {
        let stage = tempfile::tempdir()?;
        let target = stage.path().join("mount");
        std::fs::create_dir(&target)?;
        let address = stage.path().join("socket");
        let socket = UnixDatagram::bind(&address)?;
        socket.set_read_timeout(Some(Duration::from_secs(5)))?;
        let mut child = std::process::Command::new("/usr/bin/unshare")
            .args([
                "--user",
                "--map-root-user",
                "--mount",
                "--propagation",
                "private",
            ])
            .arg(allocator()?)
            .arg(&target)
            .arg(&address)
            .arg(limits.output_bytes.to_string())
            .arg(limits.output_inodes.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let result = receive(&socket);
        // The allocator has no input other than trusted paths and budgets, and exits after sendmsg.
        let _ = child.kill();
        let _ = child.wait();
        let (root, namespace, mount_namespace) =
            result.map_err(|e| BuildError::Unavailable(format!("output quota tmpfs: {e}")))?;
        let path = PathBuf::from(format!(
            "/proc/{}/fd/{}",
            std::process::id(),
            root.as_raw_fd()
        ));
        std::fs::create_dir(path.join(".font-cache"))?;
        Ok(Self {
            _root: root,
            namespace,
            mount_namespace,
            stage,
            path,
            exhausted: Arc::new(AtomicBool::new(false)),
            budget: (limits.output_bytes, limits.output_inodes),
        })
    }
    pub fn full(&self) -> bool {
        let full = statvfs(&self.path)
            .map(|s| s.blocks_available() == 0 || s.files_available() == 0)
            .unwrap_or(true);
        if full {
            self.exhausted.store(true, Ordering::Release);
        }
        full
    }
    pub fn export(&self, work: &Path) -> Result<(), BuildError> {
        copy_tree(&self.path, work, self.budget)
    }
}
fn allocator() -> Result<PathBuf, BuildError> {
    let executable = std::env::current_exe()?;
    let mut parent = executable
        .parent()
        .ok_or_else(|| BuildError::Unavailable("executable directory".into()))?;
    if parent.file_name().is_some_and(|s| s == "deps") {
        parent = parent
            .parent()
            .ok_or_else(|| BuildError::Unavailable("test binary directory".into()))?;
    }
    let path = parent.join("scholium-sandbox-fs");
    if !path.is_file() {
        return Err(BuildError::Unavailable(
            "install scholium-sandbox-fs beside the application".into(),
        ));
    }
    Ok(path)
}
fn receive(socket: &UnixDatagram) -> std::io::Result<(File, File, File)> {
    let mut byte = [0];
    let mut control = nix::cmsg_space!([std::os::fd::RawFd; 3]);
    let mut slices = [IoSliceMut::new(&mut byte)];
    let message = recvmsg::<()>(
        socket.as_raw_fd(),
        &mut slices,
        Some(&mut control),
        MsgFlags::MSG_CMSG_CLOEXEC,
    )?;
    for item in message.cmsgs()? {
        if let ControlMessageOwned::ScmRights(fds) = item
            && let [fd, namespace, mount_namespace] = fds.as_slice()
        {
            // SAFETY: SCM_RIGHTS transferred ownership of these three new fds.
            return Ok(unsafe {
                (
                    File::from_raw_fd(*fd),
                    File::from_raw_fd(*namespace),
                    File::from_raw_fd(*mount_namespace),
                )
            });
        }
    }
    Err(std::io::Error::other(
        "allocator did not supply a directory fd",
    ))
}

pub(crate) fn copy_tree(
    source: &Path,
    target: &Path,
    budget: (u64, u64),
) -> Result<(), BuildError> {
    let source = cap_std::fs::Dir::open_ambient_dir(source, cap_std::ambient_authority())?;
    let target = cap_std::fs::Dir::open_ambient_dir(target, cap_std::ambient_authority())?;
    copy_dir(&source, &target, &mut (budget.0, budget.1))
}
fn copy_dir(
    source: &cap_std::fs::Dir,
    target: &cap_std::fs::Dir,
    remaining: &mut (u64, u64),
) -> Result<(), BuildError> {
    for entry in source.entries()? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".font-cache" {
            continue;
        }
        remaining.1 = remaining.1.checked_sub(1).ok_or(BuildError::OutputQuota)?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            target.create_dir_all(&name)?;
            copy_dir(
                &source.open_dir(&name)?,
                &target.open_dir(&name)?,
                remaining,
            )?;
        } else if kind.is_file() {
            let mut input = source.open(&name)?;
            let size = input.metadata()?.len();
            remaining.0 = remaining
                .0
                .checked_sub(size)
                .ok_or(BuildError::OutputQuota)?;
            let mut output = target.create(&name)?;
            let copied = std::io::copy(&mut input.by_ref().take(size + 1), &mut output)?;
            if copied != size {
                return Err(BuildError::OutputQuota);
            }
        } else {
            return Err(BuildError::Configuration("non-regular build output".into()));
        }
    }
    Ok(())
}

fn volumes()
-> &'static std::sync::Mutex<std::collections::BTreeMap<PathBuf, std::sync::Weak<Volume>>> {
    static VOLUMES: std::sync::OnceLock<
        std::sync::Mutex<std::collections::BTreeMap<PathBuf, std::sync::Weak<Volume>>>,
    > = std::sync::OnceLock::new();
    VOLUMES.get_or_init(Default::default)
}
pub(crate) fn register(work: &Path, volume: &Arc<Volume>) {
    let mut map = volumes()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    map.retain(|_, value| value.strong_count() > 0);
    map.insert(work.to_owned(), Arc::downgrade(volume));
}
pub(crate) fn read(work: &Path, relative: &Path, limit: u64) -> Result<Vec<u8>, BuildError> {
    let map = volumes()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let volume = map
        .get(work)
        .and_then(std::sync::Weak::upgrade)
        .ok_or_else(|| BuildError::Configuration("no active output volume".into()))?;
    drop(map);
    let root = cap_std::fs::Dir::open_ambient_dir(&volume.path, cap_std::ambient_authority())?;
    let file = root.open(relative)?;
    if !file.metadata()?.is_file() {
        return Err(BuildError::Configuration("non-regular output".into()));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(BuildError::OutputQuota);
    }
    Ok(bytes)
}
