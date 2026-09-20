//! Bounded host reads of files created by isolated workers.
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom},
    os::unix::fs::OpenOptionsExt,
    path::Path,
    process::{Command, Output, Stdio},
};

const MAX_COMMAND_OUTPUT: u64 = 4 * 1024 * 1024;

/// Read one regular artifact without following its final symlink or blocking on a FIFO.
pub(crate) fn read(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    read_file(file, limit)
}

fn read_file(mut file: File, limit: u64) -> io::Result<Vec<u8>> {
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(io::Error::other("artifact type or byte budget exceeded"));
    }
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    // The limit applies during the read too: a resident worker can grow the inode.
    file.take(limit.saturating_add(1)).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(io::Error::other("artifact grew beyond byte budget"));
    }
    Ok(bytes)
}

/// Capture a bounded response after the sandbox exits, without unbounded pipe buffering.
pub(crate) fn command_output(command: &mut Command, root: &Path) -> io::Result<Output> {
    let create = |name| {
        OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(root.join(name))
    };
    let stdout = create("command-stdout")?;
    let stderr = match create("command-stderr") {
        Ok(file) => file,
        Err(error) => {
            let _ = std::fs::remove_file(root.join("command-stdout"));
            return Err(error);
        }
    };
    let result = (|| {
        let status = command
            .stdout(Stdio::from(stdout.try_clone()?))
            .stderr(Stdio::from(stderr.try_clone()?))
            .status()?;
        Ok(Output {
            status,
            stdout: read_file(stdout, MAX_COMMAND_OUTPUT)?,
            stderr: read_file(stderr, MAX_COMMAND_OUTPUT)?,
        })
    })();
    let _ = std::fs::remove_file(root.join("command-stdout"));
    let _ = std::fs::remove_file(root.join("command-stderr"));
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn rejects_symlink_directory_fifo_and_oversize_before_reading() {
        let root = std::env::temp_dir().join(format!("scholium-artifact-{}", std::process::id()));
        std::fs::create_dir(&root).expect("test directory");
        std::fs::write(root.join("valid"), b"abc").expect("file");
        symlink(root.join("valid"), root.join("link")).expect("symlink");
        assert!(
            Command::new("mkfifo")
                .arg(root.join("fifo"))
                .status()
                .expect("mkfifo")
                .success()
        );
        assert_eq!(read(&root.join("valid"), 3).expect("read"), b"abc");
        assert!(read(&root.join("valid"), 2).is_err());
        for path in [&root, &root.join("link"), &root.join("fifo")] {
            assert!(read(path, 1024).is_err(), "accepted {path:?}");
        }
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn caps_command_response_and_preserves_status() {
        let root = std::env::temp_dir().join(format!("scholium-command-{}", std::process::id()));
        std::fs::create_dir(&root).expect("test directory");
        let good = command_output(Command::new("sh").args(["-c", "printf ok; exit 3"]), &root)
            .expect("capture");
        assert_eq!(good.status.code(), Some(3));
        assert_eq!(good.stdout, b"ok");
        let oversized = command_output(
            Command::new("head").args(["-c", "4194305", "/dev/zero"]),
            &root,
        );
        assert!(oversized.is_err());
        assert_eq!(std::fs::read_dir(&root).expect("directory").count(), 0);
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
