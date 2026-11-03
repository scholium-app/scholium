use super::*;
fn next(state: &mut u64) -> usize {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state as usize
}
#[test]
fn fixed_seed_three_actor_interleavings_converge_with_duplicates_undo_and_restore() {
    for seed in 0x5eed_0041..0x5eed_004b {
        exercise(seed);
    }
}
fn exercise(seed: u64) {
    let initial = fixture();
    let leaf = initial.blocks[0].content[0].node;
    let InlineBody::Math { root } = &initial.blocks[0].content[1].body else {
        panic!("math")
    };
    let math = root.node;
    let a = Replica::new(initial, 1).expect("bootstrap");
    let snapshot = a.snapshot().expect("snapshot");
    let mut replicas = vec![
        a,
        Replica::restore(&snapshot, 2).expect("fork B"),
        Replica::restore(&snapshot, 3).expect("fork C"),
    ];
    let mut actions = Vec::new();
    let mut rng = seed;
    for step in 0..120 {
        let peer = next(&mut rng) % 3;
        let before = replicas[peer].document().expect("before");
        let result = local_action(&mut replicas[peer], (leaf, math), next(&mut rng) % 6, peer);
        let action = match result {
            Ok(action) => action,
            Err(Error::Edit(scholium_document::EditError::WrongTarget)) => {
                assert_eq!(
                    replicas[peer].document().expect("rejection is atomic"),
                    before
                );
                None
            }
            Err(e) => panic!("seed {seed:#x}, step {step}: {e:?}"),
        };
        if let Some(action) = action {
            actions.push(action);
        }
        if !actions.is_empty() {
            let index = next(&mut rng) % actions.len();
            replicas[next(&mut rng) % 3]
                .receive(&actions[index])
                .unwrap_or_else(|e| panic!("seed {seed:#x} receive {index}: {e:?}"));
        }
        if step == 60 {
            let snapshot = replicas[peer].snapshot().expect("mid-stream snapshot");
            replicas[peer] =
                Replica::restore(&snapshot, peer as u64 + 1).expect("resume same writer");
        }
        for replica in &replicas {
            replica
                .document()
                .expect("valid projection")
                .validate()
                .expect("invariants");
        }
    }
    settle(&mut replicas, &actions, seed);
}
fn local_action(
    replica: &mut Replica,
    nodes: (NodeId, NodeId),
    choice: usize,
    peer: usize,
) -> Result<Option<Action>, Error> {
    match choice {
        0 => replica.undo(),
        1 => replica.redo(),
        2 => replica.edit(StructuralEdit::WrapFraction { node: nodes.1 }, "wrap"),
        _ => replica.edit(
            StructuralEdit::ReplaceText {
                leaf: nodes.0,
                start: 0,
                end: 0,
                text: char::from(b'A' + peer as u8).to_string(),
            },
            "typing",
        ),
    }
}
fn settle(replicas: &mut [Replica], actions: &[Action], seed: u64) {
    for replica in replicas.iter_mut() {
        for action in actions.iter().rev() {
            replica
                .receive(action)
                .unwrap_or_else(|e| panic!("seed {seed:#x} final delivery: {e:?}"));
        }
    }
    let expected = replicas[0].projection().expect("projection");
    for replica in &replicas[1..] {
        assert_eq!(
            expected,
            replica.projection().expect("projection"),
            "seed {seed:#x}"
        );
    }
}
