use crate::{CompileOutcome, EngineState, LiveEvent, mailbox};
use std::time::Duration;

fn paragraph(paragraph: u64, revision: u64) -> LiveEvent {
    LiveEvent::Paragraph {
        generation: 0,
        revision: Some(revision),
        paragraph,
        elapsed: None,
    }
}

#[tokio::test]
async fn superseded_previews_merge_and_distinct_overflow_is_explicit_and_bounded() {
    let (sender, mut receiver) = mailbox::channel();
    for revision in 0..1000 {
        sender
            .send(paragraph(1, revision))
            .await
            .expect("paragraph");
        sender
            .send(LiveEvent::Pages {
                generation: 0,
                revision: Some(revision),
                count: 1,
                outcome: CompileOutcome::Complete,
                elapsed: Duration::ZERO,
            })
            .await
            .expect("pages");
        assert_eq!(receiver.pending(), 2);
    }
    assert_eq!(receiver.recv().await, Some(paragraph(1, 999)));
    assert!(matches!(
        receiver.recv().await,
        Some(LiveEvent::Pages {
            revision: Some(999),
            ..
        })
    ));
    for identity in 0..1000 {
        sender
            .send(paragraph(identity, identity))
            .await
            .expect("distinct update");
        assert!(
            receiver.pending() <= 33,
            "32 events plus one coalesced resync marker"
        );
    }
    drop(sender);
    let mut marker = false;
    while let Some(event) = receiver.recv().await {
        marker |= matches!(event, LiveEvent::LayoutRequired { generation: 0 });
    }
    assert!(
        marker,
        "eviction must request complete layout resynchronization"
    );
}

#[tokio::test]
async fn lifecycle_backpressure_preserves_every_state_and_terminal_reason() {
    let (sender, mut receiver) = mailbox::channel();
    let producer = tokio::spawn(async move {
        for generation in 0..40 {
            sender
                .send(LiveEvent::Engine {
                    generation,
                    state: EngineState::Failed,
                    reason: Some(format!("failure {generation}")),
                })
                .await
                .expect("engine event");
        }
    });
    tokio::task::yield_now().await;
    assert_eq!(receiver.pending(), 32);
    assert!(
        !producer.is_finished(),
        "critical event overflow must apply backpressure"
    );
    for generation in 0..40 {
        let event = receiver.recv().await.expect("retained state");
        assert_eq!(
            event,
            LiveEvent::Engine {
                generation,
                state: EngineState::Failed,
                reason: Some(format!("failure {generation}"))
            }
        );
        assert!(receiver.pending() <= 32);
    }
    producer.await.expect("producer");
    assert_eq!(receiver.recv().await, None);
}
