//! Trusted native-test fixture, never linked into the product or mounted as an extra tool.
use std::{fs, io::Write, net::TcpStream, process::Command, time::Duration};

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("hang") => {
            let mut child = Command::new("/worker").arg("child").spawn().expect("child");
            child.wait().expect("wait");
        }
        Some("child") => loop {
            fs::write("/out/ticks", format!("{:?}", std::time::Instant::now())).expect("tick");
            std::thread::sleep(Duration::from_millis(25));
        },
        Some("quota-bytes") | Some("quota-inodes") => {
            let bytes = std::env::args().nth(1).as_deref() == Some("quota-bytes");
            for index in 0..1024 {
                let data = if bytes { vec![b'x'; 8192] } else { Vec::new() };
                if fs::write(format!("/out/file-{index}"), data).is_err() { break; }
            }
        }
        Some("memory-charge") => {
            println!("ready");
            std::io::stdin().read_line(&mut String::new()).expect("start");
            fs::write("/out/pages", vec![0; 32 * 1024 * 1024]).expect("tmpfs pages");
            for index in 0..12000 { fs::write(format!("/out/inode-{index}"), []).expect("tmpfs inode"); }
            println!("charged");
            std::io::stdin().read_line(&mut String::new()).expect("finish");
        }
        Some("spam") => {
            std::io::stdout().write_all(&vec![b'x'; 2 * 1024 * 1024]).expect("output");
        }
        _ => verify(),
    }
}

fn verify() {
    let status = fs::read_to_string("/proc/self/status").expect("status");
    assert!(status.lines().any(|s| s == "CapEff:\t0000000000000000"), "capabilities retained");
    assert!(status.lines().any(|s| s == "NoNewPrivs:\t1"), "no_new_privs missing");
    unsafe extern "C" { fn unshare(flags: i32) -> i32; fn mount(source: *const u8, target: *const u8, filesystem: *const u8, flags: usize, data: *const u8) -> i32; }
    // SAFETY: unshare(0) has no namespace side effect and is rejected by the inherited filter.
    assert_eq!(unsafe { unshare(0) }, -1, "non-bwrap namespace syscall must be blocked");
    assert_eq!(std::io::Error::last_os_error().raw_os_error(), Some(1));
    // SAFETY: null arguments cannot mount anything; seccomp rejects before argument validation.
    assert_eq!(unsafe { mount(std::ptr::null(), std::ptr::null(), std::ptr::null(), 0, std::ptr::null()) }, -1);
    assert_eq!(std::io::Error::last_os_error().raw_os_error(), Some(1), "mount must return EPERM");
    let request = fs::read_to_string("/request.json").expect("request read");
    let mut lines = request.lines();
    let secret = lines.next().expect("secret path");
    let address = lines.next().expect("host listener").parse().expect("address");
    assert!(fs::read(secret).is_err(), "host file visible");
    assert!(TcpStream::connect_timeout(&address, Duration::from_millis(200)).is_err(), "host network visible");
    assert_eq!(fs::read_to_string("/project/input").expect("resource"), "resource");
    for path in ["/project/input", "/request.json", "/worker", "/usr/bin/prlimit"] {
        assert!(fs::write(path, "poison").is_err(), "writable {path}");
    }
    let limits = fs::read_to_string("/proc/self/limits").expect("rlimits");
    for (prefix, value) in [
        ("Max cpu time", "30"),
        ("Max address space", "4294967296"),
        ("Max file size", "67108864"),
        ("Max open files", "128"),
    ] {
        let line = limits.lines().find(|line| line.starts_with(prefix)).expect("limit");
        let columns: Vec<_> = line.trim_start_matches(prefix).split_whitespace().collect();
        assert_eq!(&columns[..2], [value, value], "{line}");
    }
    fs::write("/out/result", "isolated").expect("output");
    println!("Typst profile isolation and rlimits enforced");
}
