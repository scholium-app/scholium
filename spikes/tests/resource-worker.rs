//! Trusted attack fixture; never a project-provided executable.
use std::{process::Command, thread, time::Duration};

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("memory") => {
            let mut children = Vec::new();
            for _ in 0..3 {
                children.push(
                    Command::new(std::env::current_exe().unwrap())
                        .arg("allocate")
                        .spawn()
                        .unwrap(),
                );
            }
            for mut child in children {
                assert!(child.wait().unwrap().success());
            }
        }
        Some("allocate") => {
            let mut bytes = vec![0u8; 768 * 1024 * 1024];
            for page in bytes.chunks_mut(4096) {
                page[0] = 1;
            }
            println!("touched {}", bytes.len());
            thread::sleep(Duration::from_secs(30));
            std::hint::black_box(bytes);
        }
        Some("tasks") => {
            let mut threads = Vec::new();
            for _ in 0..100 {
                match thread::Builder::new().spawn(|| thread::sleep(Duration::from_secs(30))) {
                    Ok(thread) => threads.push(thread),
                    Err(error) => {
                        assert_eq!(error.raw_os_error(), Some(11));
                        assert!(!threads.is_empty() && threads.len() < 64);
                        println!("task quota enforced after {} threads", threads.len());
                        return;
                    }
                }
            }
            panic!("task quota not enforced");
        }
        _ => panic!("unknown trusted fixture"),
    }
}
