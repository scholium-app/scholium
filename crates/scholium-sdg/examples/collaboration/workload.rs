use super::{Edit, trace};
pub const NAMES: [&str; 8] = [
    "mixed",
    "text_insert",
    "text_delete",
    "node_create",
    "node_delete",
    "wrap",
    "unwrap",
    "cross_parent_move",
];
fn node(peer: usize, n: usize) -> String {
    format!("probe-{peer}-{n}")
}
fn create(peer: usize, n: usize) -> Edit {
    Edit::Create {
        id: node(peer, n),
        parent: format!("r{peer}"),
        kind: "symbol",
        text: "α",
    }
}
pub fn setup(name: &str, peer: usize, count: usize) -> Vec<Edit> {
    match name {
        "text_delete" => (0..count)
            .map(|_| Edit::Insert(format!("t{peer}")))
            .collect(),
        "node_delete" => (0..count).map(|n| create(peer, n)).collect(),
        "wrap" | "unwrap" => {
            let mut edits = Vec::new();
            for n in 0..count {
                edits.push(create(peer, n));
                if name == "unwrap" {
                    edits.push(Edit::Create {
                        id: format!("wrap-{peer}-{n}"),
                        parent: format!("r{peer}"),
                        kind: "root",
                        text: "",
                    });
                }
                if name == "unwrap" {
                    edits.push(Edit::Move {
                        id: node(peer, n),
                        parent: format!("wrap-{peer}-{n}"),
                    });
                }
            }
            edits
        }
        _ => Vec::new(),
    }
}
pub fn edits(name: &str, peer: usize, count: usize) -> Vec<Edit> {
    assert!(NAMES.contains(&name));
    if name == "mixed" {
        return trace(peer).into_iter().take(count).collect();
    }
    (0..count)
        .map(|n| match name {
            "text_insert" => Edit::Insert(format!("t{peer}")),
            "text_delete" => Edit::Remove(format!("t{peer}")),
            "node_create" => create(peer, n),
            "node_delete" => Edit::Delete(node(peer, n)),
            "wrap" => Edit::Wrap {
                id: node(peer, n),
                wrapper: format!("wrap-{peer}-{n}"),
                parent: format!("r{peer}"),
            },
            "unwrap" => Edit::Unwrap {
                id: node(peer, n),
                wrapper: format!("wrap-{peer}-{n}"),
                parent: format!("r{peer}"),
            },
            "cross_parent_move" => Edit::Move {
                id: format!("t{peer}"),
                parent: format!("p{}", if n % 2 == 0 { peer + 2 } else { peer }),
            },
            _ => unreachable!("validated workload"),
        })
        .collect()
}
