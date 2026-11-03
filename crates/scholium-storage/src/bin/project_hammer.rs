//! Deterministic SIGKILL/lock probe for formal storage integration tests.
use scholium_storage::project::{
    ActionRecord, ContentHash, IoHooks, IoPoint, OpenPolicy, ProjectStore, Resource, Revision,
};
use std::io::{self, Write};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

#[derive(Debug)]
struct Barrier {
    point: IoPoint,
    skip: usize,
    armed: AtomicBool,
    visits: AtomicUsize,
}
impl IoHooks for Barrier {
    fn check(&self, point: IoPoint) -> io::Result<()> {
        if point == self.point
            && self.armed.load(Ordering::SeqCst)
            && self.visits.fetch_add(1, Ordering::SeqCst) >= self.skip
        {
            println!("BOUNDARY");
            io::stdout().flush()?;
            let mut line = String::new();
            io::stdin().read_line(&mut line)?;
        }
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let root = std::path::Path::new(args.get(1).ok_or("missing root")?);
    let mode = args.get(2).ok_or("missing mode")?;
    let point = match mode.as_str() {
        "intent" => IoPoint::IntentCommitted,
        "rename" => IoPoint::Rename,
        "renamed" => IoPoint::Renamed,
        "directory" => IoPoint::SyncDirectory,
        _ => IoPoint::Commit,
    };
    let barrier = Arc::new(Barrier {
        point,
        skip: usize::from(mode == "finish"),
        armed: AtomicBool::new(false),
        visits: AtomicUsize::new(0),
    });
    let mut store = ProjectStore::open_with_io(root, OpenPolicy::Full, barrier.clone())?.store;
    if mode == "lock" {
        println!("LOCKED");
        io::stdout().flush()?;
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        return Ok(());
    }
    if mode == "actions" {
        return action_loop(&mut store, &barrier);
    }
    let resource = Resource::new([1; 16], "main.typ")?;
    store.materialize(&resource, Revision(1), b"old", None)?;
    store.save_draft(&resource, Some(ContentHash::of(b"old")), b"private draft")?;
    barrier.armed.store(true, Ordering::SeqCst);
    store.materialize(
        &resource,
        Revision(2),
        b"new",
        Some(ContentHash::of(b"old")),
    )?;
    Err("barrier should be killed".into())
}
fn action_loop(
    store: &mut ProjectStore,
    barrier: &Barrier,
) -> Result<(), Box<dyn std::error::Error>> {
    for n in 1_u64..=5 {
        store.append_action(&ActionRecord::new(n.to_le_bytes().to_vec()))?;
    }
    barrier.armed.store(true, Ordering::SeqCst);
    // Kill after SQLite has written the large next transaction, before its commit.
    store.append_action(&ActionRecord::new(vec![42; 8 * 1024 * 1024]))?;
    Err("barrier should be killed".into())
}
