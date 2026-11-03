//! Reproducible ADR 0049 comparison; opt in with --ignored --nocapture.
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tempfile::tempdir_in;

const KV: TableDefinition<&[u8], &[u8]> = TableDefinition::new("records");
const WRITES: usize = 1000;
const OPENS: usize = 100;

#[derive(Clone, Copy, Debug)]
enum Workload {
    Action,
    Draft(usize),
    Blob(bool),
}
impl Workload {
    fn parse(value: &str) -> Self {
        match value {
            "action" => Self::Action,
            "unique" => Self::Blob(false),
            "dedup" => Self::Blob(true),
            _ => Self::Draft(value.parse().expect("size")),
        }
    }
}
fn samples(label: &str, values: &mut [Duration]) {
    values.sort();
    println!(
        "{label},n={},p50_us={:.3},p99_us={:.3}",
        values.len(),
        values[values.len() / 2].as_secs_f64() * 1e6,
        values[(values.len() * 99).div_ceil(100).saturating_sub(1)].as_secs_f64() * 1e6
    );
}
fn payload(work: Workload, index: usize) -> (Vec<u8>, Vec<u8>) {
    let size = match work {
        Workload::Draft(n) => n,
        _ => 1024,
    };
    let mut bytes = vec![b'x'; size];
    if !matches!(work, Workload::Blob(true)) {
        bytes[..8].copy_from_slice(&(index as u64).to_le_bytes());
    }
    let key = match work {
        Workload::Action => (index as u64).to_be_bytes().to_vec(),
        Workload::Draft(_) => vec![1; 16],
        Workload::Blob(_) => Sha256::digest(&bytes).to_vec(),
    };
    (key, bytes)
}
fn crash_barrier() {
    println!("FIXTURE_READY");
    std::io::stdout().flush().expect("flush");
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).expect("barrier");
    panic!("parent must SIGKILL fixture writer");
}
fn sqlite_fixture(path: &Path, work: Workload) {
    let mut db = Connection::open(path).expect("sqlite");
    db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;")
        .expect("durability");
    db.execute_batch(include_str!("../src/project/schema.sql"))
        .expect("schema");
    db.execute(
        "INSERT INTO resources VALUES (?1,'main.typ')",
        [vec![1_u8; 16]],
    )
    .expect("resource");
    let mut timings = Vec::new();
    for index in 0..WRITES {
        let (key, bytes) = payload(work, index);
        let start = Instant::now();
        let hash = Sha256::digest(&bytes).to_vec();
        let tx = db.transaction().expect("tx");
        match work {
            Workload::Action => {
                tx.execute(
                    "INSERT INTO actions(version,payload,hash) VALUES(1,?1,?2)",
                    (&bytes, &hash),
                )
                .expect("action");
            }
            Workload::Draft(_) => {
                tx.execute("INSERT INTO drafts VALUES (?1,1,NULL,?2,?3) ON CONFLICT(resource) DO UPDATE SET bytes=excluded.bytes,hash=excluded.hash",(&key,&bytes,&hash)).expect("draft");
            }
            Workload::Blob(_) => {
                tx.execute("INSERT OR IGNORE INTO blobs VALUES (?1,?2)", (&key, &bytes))
                    .expect("blob");
                let stored: Vec<u8> = tx
                    .query_row("SELECT bytes FROM blobs WHERE hash=?1", [&key], |r| {
                        r.get(0)
                    })
                    .expect("read");
                assert_eq!(stored, bytes);
            }
        }
        tx.commit().expect("commit");
        timings.push(start.elapsed());
    }
    samples(&format!("sqlite,{work:?}"), &mut timings);
    crash_barrier();
}
fn redb_fixture(path: &Path, work: Workload) {
    let db = Database::create(path).expect("redb");
    let tx = db.begin_write().expect("tx");
    tx.open_table(KV).expect("table");
    tx.commit().expect("commit");
    let mut timings = Vec::new();
    for index in 0..WRITES {
        let (key, bytes) = payload(work, index);
        let start = Instant::now();
        let hash = Sha256::digest(&bytes);
        let mut envelope = vec![1];
        envelope.extend_from_slice(&hash);
        envelope.extend_from_slice(&bytes);
        let mut tx = db.begin_write().expect("tx");
        tx.set_durability(redb::Durability::Immediate)
            .expect("durability");
        {
            let mut table = tx.open_table(KV).expect("table");
            if matches!(work, Workload::Blob(_)) {
                let existing = table
                    .get(key.as_slice())
                    .expect("get")
                    .map(|v| v.value().to_vec());
                if let Some(existing) = existing {
                    assert_eq!(existing, envelope);
                } else {
                    table
                        .insert(key.as_slice(), envelope.as_slice())
                        .expect("write");
                }
            } else {
                table
                    .insert(key.as_slice(), envelope.as_slice())
                    .expect("write");
            }
        }
        tx.commit().expect("commit");
        timings.push(start.elapsed());
    }
    samples(&format!("redb,{work:?}"), &mut timings);
    crash_barrier();
}

#[test]
#[ignore = "subprocess fixture writer, run only by compare_formal_durable_workloads"]
fn populate_crash_fixture() {
    let engine = std::env::var("T35_BENCH_ENGINE").expect("engine");
    let path = PathBuf::from(std::env::var_os("T35_BENCH_PATH").expect("path"));
    let work = Workload::parse(&std::env::var("T35_BENCH_WORK").expect("work"));
    if engine == "sqlite" {
        sqlite_fixture(&path, work);
    } else {
        redb_fixture(&path, work);
    }
}
fn crashed_fixture(engine: &str, path: &Path, work: &str) {
    let mut child = Command::new(std::env::current_exe().expect("exe"))
        .args([
            "--ignored",
            "--exact",
            "populate_crash_fixture",
            "--nocapture",
        ])
        .env("T35_BENCH_ENGINE", engine)
        .env("T35_BENCH_PATH", path)
        .env("T35_BENCH_WORK", work)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("child");
    let stdout = child.stdout.take().expect("stdout");
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let line = line.expect("line");
            if line == "FIXTURE_READY" {
                send.send(()).expect("ready");
                return;
            }
            if line.contains("p50_us=") {
                println!("{line}");
            }
        }
    });
    let ready = receive.recv_timeout(Duration::from_secs(60));
    child.kill().expect("kill fixture");
    child.wait().expect("reap");
    ready.expect("fixture ready");
}
fn sqlite_scan(path: &Path) {
    let db = Connection::open(path).expect("reopen");
    let check: String = db
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .expect("integrity");
    assert_eq!(check, "ok");
    for (table, column) in [
        ("actions", "payload"),
        ("drafts", "bytes"),
        ("blobs", "bytes"),
    ] {
        let mut stmt = db
            .prepare(&format!("SELECT {column},hash FROM {table}"))
            .expect("scan");
        let mut rows = stmt.query([]).expect("rows");
        while let Some(row) = rows.next().expect("next") {
            let bytes: Vec<u8> = row.get(0).expect("bytes");
            let hash: Vec<u8> = row.get(1).expect("hash");
            assert_eq!(Sha256::digest(&bytes).as_slice(), hash);
        }
    }
}
fn redb_scan(path: &Path) {
    let db = Database::open(path).expect("reopen");
    let tx = db.begin_read().expect("read");
    let table = tx.open_table(KV).expect("table");
    for row in table.iter().expect("scan") {
        let (_, value) = row.expect("row");
        let envelope = value.value();
        assert_eq!(envelope[0], 1);
        assert_eq!(Sha256::digest(&envelope[33..]).as_slice(), &envelope[1..33]);
    }
}
fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}
fn restore(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).expect("restore");
    std::fs::File::open(path)
        .expect("restored file")
        .sync_all()
        .expect("sync restored fixture outside timing");
}
fn opens(engine: &str, fixture: &Path, trial: &Path, work: &str) {
    let bytes = std::fs::read(fixture).expect("fixture");
    let wal = std::fs::read(fixture.with_extension("wal")).ok();
    let mut timings = Vec::new();
    for _ in 0..OPENS {
        for suffix in ["-wal", "-shm"] {
            match std::fs::remove_file(sidecar(trial, suffix)) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => panic!("remove previous trial sidecar: {e}"),
            }
        }
        restore(trial, &bytes);
        if let Some(wal) = &wal {
            restore(&sidecar(trial, "-wal"), wal);
        }
        std::fs::File::open(trial.parent().expect("parent"))
            .expect("directory")
            .sync_all()
            .expect("sync directory");
        let start = Instant::now();
        if engine == "sqlite" {
            sqlite_scan(trial);
        } else {
            redb_scan(trial);
        }
        timings.push(start.elapsed());
    }
    samples(&format!("{engine},crash_open_scan,{work}"), &mut timings);
}

#[test]
#[ignore = "ADR 0049 fsync latency benchmark, explicit opt-in"]
fn compare_formal_durable_workloads() {
    println!(
        "writes={WRITES}; opens={OPENS}; independent_transactions=true; engine_cache=cold; os_cache=warm; crash_fixture_restored_before_each_open=true"
    );
    // /tmp may be tmpfs: durability latency must be measured on the worktree filesystem.
    let bench_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/t35-bench");
    std::fs::create_dir_all(&bench_root).expect("benchmark directory");
    println!("fixture_root={}", bench_root.display());
    for work in ["action", "1024", "102400", "1048576", "unique", "dedup"] {
        let root = tempdir_in(&bench_root).expect("root");
        for engine in ["sqlite", "redb"] {
            let fixture = root.path().join(format!("{engine}.db"));
            crashed_fixture(engine, &fixture, work);
            // SQLite names WAL sidecars by suffix, not extension.
            if engine == "sqlite" {
                std::fs::rename(
                    root.path().join("sqlite.db-wal"),
                    fixture.with_extension("wal"),
                )
                .expect("WAL survives SIGKILL");
            }
            let trial = root.path().join(format!("{engine}-trial.db"));
            opens(engine, &fixture, &trial, work);
        }
    }
}
