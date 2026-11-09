//! Bwrap-created tmpfs retained by a directory fd; this module never mounts or enters namespaces.
use crate::{BuildError, ResourceLimits};
use cap_std::fs::{FileTypeExt, OpenOptionsExt};
use nix::sys::{
    socket::{ControlMessageOwned, MsgFlags, recvmsg},
    statvfs::statvfs,
};
use std::{
    fs::File,
    io::{IoSliceMut, Read},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::net::UnixStream,
    },
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Debug)]
pub(crate) struct Volume {
    _root: File,
    pub path: PathBuf,
    pub exhausted: Arc<AtomicBool>,
    pub budget: (u64, u64),
}
impl Volume {
    pub fn receive(socket: &UnixStream, limits: ResourceLimits) -> Result<Self, BuildError> {
        let mut byte = [0];
        let mut control = nix::cmsg_space!([std::os::fd::RawFd; 1]);
        let mut slices = [IoSliceMut::new(&mut byte)];
        let message = recvmsg::<()>(
            socket.as_raw_fd(),
            &mut slices,
            Some(&mut control),
            MsgFlags::MSG_CMSG_CLOEXEC,
        )
        .map_err(std::io::Error::from)?;
        let mut directories = Vec::new();
        for item in message.cmsgs().map_err(std::io::Error::from)? {
            if let ControlMessageOwned::ScmRights(fds) = item {
                for fd in fds {
                    // SAFETY: SCM_RIGHTS transfers ownership of each new fd. Own all so
                    // malformed ancillary data cannot leak descriptors on error.
                    directories.push(unsafe { File::from_raw_fd(fd) });
                }
            }
        }
        if directories.len() == 1 && !message.flags.contains(MsgFlags::MSG_CTRUNC) {
            let root = directories.remove(0);
            if !root.metadata()?.is_dir() {
                return Err(BuildError::Unavailable(
                    "invalid output directory fd".into(),
                ));
            }
            let path = PathBuf::from(format!(
                "/proc/{}/fd/{}",
                std::process::id(),
                root.as_raw_fd()
            ));
            return Ok(Self {
                _root: root,
                path,
                exhausted: Arc::new(AtomicBool::new(false)),
                budget: (limits.output_bytes, limits.output_inodes),
            });
        }
        Err(BuildError::Unavailable(
            "bwrap output handshake did not supply a directory fd".into(),
        ))
    }
    pub fn full(&self) -> bool {
        let full = statvfs(&self.path)
            .map(|s| s.blocks_available() == 0)
            .unwrap_or(true)
            || cap_std::fs::Dir::open_ambient_dir(&self.path, cap_std::ambient_authority())
                .and_then(|root| inode_count(&root, &mut self.budget.1.saturating_sub(1), 0))
                .is_err();
        if full {
            self.exhausted.store(true, Ordering::Release);
        }
        full || self.exhausted.load(Ordering::Acquire)
    }
    pub fn export(&self, work: &Path) -> Result<(), BuildError> {
        if self.full() {
            return Err(BuildError::OutputQuota);
        }
        copy_tree(&self.path, work, self.budget)
    }
}
fn inode_count(root: &cap_std::fs::Dir, remaining: &mut u64, depth: usize) -> std::io::Result<()> {
    if depth > 128 {
        return Err(std::io::Error::other("output nesting limit"));
    }
    for entry in root.entries()? {
        let entry = entry?;
        *remaining = remaining
            .checked_sub(1)
            .ok_or_else(|| std::io::Error::other("output file count"))?;
        if entry.file_type()?.is_dir() {
            inode_count(&root.open_dir(entry.file_name())?, remaining, depth + 1)?;
        }
    }
    Ok(())
}
pub(crate) fn helper() -> Result<PathBuf, BuildError> {
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

pub(crate) fn copy_tree(
    source: &Path,
    target: &Path,
    budget: (u64, u64),
) -> Result<(), BuildError> {
    let source = cap_std::fs::Dir::open_ambient_dir(source, cap_std::ambient_authority())?;
    let target = cap_std::fs::Dir::open_ambient_dir(target, cap_std::ambient_authority())?;
    copy_dir(&source, &target, &mut (budget.0, budget.1), 0)
}
fn copy_dir(
    source: &cap_std::fs::Dir,
    target: &cap_std::fs::Dir,
    remaining: &mut (u64, u64),
    depth: usize,
) -> Result<(), BuildError> {
    const MAX_OUTPUT_DEPTH: usize = 128;
    if depth > MAX_OUTPUT_DEPTH {
        return Err(BuildError::Configuration(
            "build output nesting limit exceeded".into(),
        ));
    }
    for entry in source.entries()? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".font-cache" {
            continue;
        }
        remaining.1 = remaining.1.checked_sub(1).ok_or(BuildError::OutputQuota)?;
        let kind = entry.file_type()?;
        // rtex leaves private session IPC endpoints behind. Never open or export them.
        if kind.is_socket() || kind.is_fifo() {
            continue;
        }
        if kind.is_dir() {
            target.create_dir_all(&name)?;
            copy_dir(
                &source.open_dir(&name)?,
                &target.open_dir(&name)?,
                remaining,
                depth + 1,
            )?;
        } else if kind.is_file() {
            let mut input = regular(source, Path::new(&name))?;
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
    let file = regular(&root, relative)?;
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(BuildError::OutputQuota);
    }
    Ok(bytes)
}

fn regular(root: &cap_std::fs::Dir, path: &Path) -> Result<cap_std::fs::File, BuildError> {
    let mut options = cap_std::fs::OpenOptions::new();
    options
        .read(true)
        .custom_flags(nix::libc::O_NONBLOCK | nix::libc::O_NOFOLLOW);
    let file = root.open_with(path, &options)?;
    if !file.metadata()?.is_file() {
        return Err(BuildError::Configuration("non-regular output".into()));
    }
    Ok(file)
}
