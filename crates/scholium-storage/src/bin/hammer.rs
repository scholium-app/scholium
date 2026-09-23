//! ADR 0028 SIGKILL 探针：循环保存会话，供测试中途击杀。
//! 用法：`hammer <db路径> [最大次数]`；每提交一次向 stdout 打印 revision。

use scholium_model::RequestId;
use scholium_storage::SessionStore;

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: hammer <db> [iterations]");
        std::process::exit(2);
    };
    let iterations: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(10_000);
    let Ok(store) = SessionStore::open(std::path::Path::new(&path)) else {
        std::process::exit(3);
    };
    let mut requests: Vec<RequestId> = Vec::new();
    for revision in 1..=iterations {
        requests.push(RequestId::fresh());
        let snapshot = scholium_model::DocumentSnapshot {
            document: scholium_model::DocumentId::fresh(),
            revision: scholium_model::Revision(revision),
            blocks: vec![scholium_model::Block {
                node: scholium_model::NodeId::fresh(),
                kind: scholium_model::BlockKind::Paragraph,
                content: vec![scholium_model::Inline::Text(format!("内容 {revision}"))],
            }],
        };
        if store.save(&snapshot, &requests).is_err() {
            std::process::exit(4);
        }
        println!("{revision}");
    }
}
