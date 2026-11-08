use serde_json::json;
#[test]
fn display_lists_reject_huge_dimensions_offsets_and_counts_before_cloning() {
    for value in [
        json!({"event":"LayoutUpdate", "pages_total":257}),
        json!({"event":"ParagraphUpdate", "dl":{"lines":[{"x":1e30}]}}),
        json!({"event":"ParagraphUpdate", "dl":{"lines":[{"xs":[1e30]}]}}),
        json!({"event":"ParagraphUpdate", "dl":{"lines":[{"baselines":[u64::MAX]}]}}),
        json!({"event":"LayoutUpdate", "pages_changed":[{"page":0}]}),
        json!({"event":"ParagraphUpdate", "fragments":[{"page":1,"first_line":1,"last_line":u64::MAX,"xs":[],"baselines":[]}]}),
    ] {
        assert!(crate::view::validate::message(&value).is_err(), "{value}");
    }
}
#[test]
fn protocol_identifiers_and_timestamps_are_not_drawing_coordinates() {
    assert!(
        crate::view::validate::message(&json!({
            "event":"LayoutUpdate", "generation":u64::MAX, "timestamp_ns":u64::MAX,
            "pages_total":1, "pages_changed":[{"page":1,"dl":{"lines":[]}}]
        }))
        .is_ok()
    );
}
#[test]
fn pathological_log_is_rejected_without_building_unbounded_diagnostics() {
    let result = crate::log::parse_log(&"! error\n".repeat(100_000));
    assert!(result.len() <= 1025);
    assert!(
        result
            .last()
            .is_some_and(|d| d.code.as_deref() == Some("latex.log.limit"))
    );
}
#[tokio::test]
async fn protocol_truncation_malformed_unknown_and_oversized_inputs_are_bounded() {
    for bytes in [
        b"{".to_vec(),
        b"{bad}\n".to_vec(),
        b"{\"event\":\"Unknown\"}\n".to_vec(),
        vec![b'x'; 65],
    ] {
        let (send, mut recv) = tokio::sync::mpsc::channel(8);
        crate::protocol::read(
            bytes.as_slice(),
            send,
            (64, 1024),
            Default::default(),
            false,
        )
        .await;
        let first = recv.recv().await.expect("error event");
        assert!(!matches!(first, crate::protocol::Incoming::Message(_)));
    }
}
