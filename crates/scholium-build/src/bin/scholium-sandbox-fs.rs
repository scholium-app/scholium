//! Trusted tmpfs allocator. Invoked only in a fresh user/mount namespace, before documents run.
use nix::{
    mount::{MsFlags, mount},
    sys::socket::{ControlMessage, MsgFlags, sendmsg},
};
use std::{
    io::IoSlice,
    os::{fd::AsRawFd, unix::net::UnixDatagram},
    path::Path,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 5 {
        return Err("expected mountpoint, socket, byte and inode budgets".into());
    }
    let bytes: u64 = args[3].parse()?;
    let inodes: u64 = args[4].parse()?;
    if bytes == 0 || inodes == 0 {
        return Err("zero budget".into());
    }
    mount(
        Some("tmpfs"),
        Path::new(&args[1]),
        Some("tmpfs"),
        MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC,
        Some(format!("size={bytes},nr_inodes={inodes},mode=700").as_str()),
    )?;
    let directory = std::fs::File::open(&args[1])?;
    let namespace = std::fs::File::open("/proc/self/ns/user")?;
    let mount_namespace = std::fs::File::open("/proc/self/ns/mnt")?;
    let socket = UnixDatagram::unbound()?;
    socket.connect(&args[2])?;
    sendmsg::<()>(
        socket.as_raw_fd(),
        &[IoSlice::new(b"Q")],
        &[ControlMessage::ScmRights(&[
            directory.as_raw_fd(),
            namespace.as_raw_fd(),
            mount_namespace.as_raw_fd(),
        ])],
        MsgFlags::empty(),
        None,
    )?;
    Ok(())
}
