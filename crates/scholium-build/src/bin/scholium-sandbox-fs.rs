//! No-mount bootstrap inside bwrap. Transfer its tmpfs fd, await bounded host seeding, then exec.
use nix::{
    libc,
    sys::socket::{ControlMessage, MsgFlags, sendmsg},
};
use std::{
    io::{IoSlice, Read},
    os::{
        fd::AsRawFd,
        unix::{net::UnixStream, process::CommandExt},
    },
    process::Command,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() < 2 || !matches!(args[0].to_str(), Some("/work" | "/out")) {
        return Err("expected fixed output directory and trusted command".into());
    }
    deny_mounts()?;
    let directory = std::fs::File::open(&args[0])?;
    let mut socket = UnixStream::connect("/quota-socket")?;
    socket.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
    sendmsg::<()>(
        socket.as_raw_fd(),
        &[IoSlice::new(b"Q")],
        &[ControlMessage::ScmRights(&[directory.as_raw_fd()])],
        MsgFlags::empty(),
        None,
    )?;
    let mut ack = [0];
    socket.read_exact(&mut ack)?;
    if ack != *b"G" {
        return Err("output bootstrap rejected".into());
    }
    drop((directory, socket));
    Err(Command::new(&args[1]).args(&args[2..]).exec().into())
}

fn deny_mounts() -> std::io::Result<()> {
    // Linux audit ABI identifiers; reject compat ABIs before comparing native syscall IDs.
    let architecture = match std::env::consts::ARCH {
        "x86_64" => 0xc000_003e,
        "aarch64" => 0xc000_00b7,
        other => {
            return Err(std::io::Error::other(format!(
                "unsupported seccomp ABI: {other}"
            )));
        }
    };
    let mut filter = vec![
        instruction(
            libc::BPF_LD | libc::BPF_W | libc::BPF_ABS,
            std::mem::offset_of!(libc::seccomp_data, arch) as u32,
        ),
        libc::sock_filter {
            code: (libc::BPF_JMP | libc::BPF_JEQ | libc::BPF_K) as u16,
            jt: 1,
            jf: 0,
            k: architecture,
        },
        instruction(libc::BPF_RET | libc::BPF_K, libc::SECCOMP_RET_KILL_PROCESS),
        instruction(
            libc::BPF_LD | libc::BPF_W | libc::BPF_ABS,
            std::mem::offset_of!(libc::seccomp_data, nr) as u32,
        ),
    ];
    if cfg!(target_arch = "x86_64") {
        // x32 shares the audit architecture but uses a distinct syscall-number bit.
        const X32_SYSCALL_BIT: u32 = 0x4000_0000;
        filter.push(libc::sock_filter {
            code: (libc::BPF_JMP | libc::BPF_JGE | libc::BPF_K) as u16,
            jt: 0,
            jf: 1,
            k: X32_SYSCALL_BIT,
        });
        filter.push(instruction(
            libc::BPF_RET | libc::BPF_K,
            libc::SECCOMP_RET_KILL_PROCESS,
        ));
    }
    for syscall in [
        libc::SYS_mount,
        libc::SYS_umount2,
        libc::SYS_unshare,
        libc::SYS_setns,
        libc::SYS_fsopen,
        libc::SYS_fsmount,
        libc::SYS_move_mount,
        libc::SYS_open_tree,
        libc::SYS_fsconfig,
        libc::SYS_mount_setattr,
    ] {
        filter.push(libc::sock_filter {
            code: (libc::BPF_JMP | libc::BPF_JEQ | libc::BPF_K) as u16,
            jt: 0,
            jf: 1,
            k: syscall as u32,
        });
        filter.push(libc::sock_filter {
            code: (libc::BPF_RET | libc::BPF_K) as u16,
            jt: 0,
            jf: 0,
            k: libc::SECCOMP_RET_ERRNO | libc::EPERM as u32,
        });
    }
    filter.push(libc::sock_filter {
        code: (libc::BPF_RET | libc::BPF_K) as u16,
        jt: 0,
        jf: 0,
        k: libc::SECCOMP_RET_ALLOW,
    });
    let program = libc::sock_fprog {
        len: filter.len() as u16,
        filter: filter.as_mut_ptr(),
    };
    // SAFETY: the kernel copies this valid filter during prctl. NO_NEW_PRIVS is permanent,
    // and the deny filter is inherited across exec; neither call grants capabilities.
    unsafe {
        if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0
            || libc::prctl(libc::PR_SET_SECCOMP, libc::SECCOMP_MODE_FILTER, &program) != 0
        {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

fn instruction(code: u32, k: u32) -> libc::sock_filter {
    libc::sock_filter {
        code: code as u16,
        jt: 0,
        jf: 0,
        k,
    }
}
