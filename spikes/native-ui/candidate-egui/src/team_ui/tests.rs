use super::*;

fn app() -> SpikeApp {
    let mut app = SpikeApp::new_layout_probe(&egui::Context::default(), None);
    app.team = Some(TeamUi::new(&app.core));
    app
}

#[test]
fn coordinator_and_reconcile_publish_together_or_not_at_all() {
    let mut app = app();
    let before = app.plain_text();
    app.source_buffer = app.source_buffer.replace("结构渲染夹具", "团队编辑夹具");
    app.commit_team_source();
    assert_ne!(app.plain_text(), before);
    assert_eq!(
        app.team
            .as_ref()
            .expect("team")
            .coordinator
            .status(0)
            .accepted,
        1
    );
    let accepted = app.plain_text();
    app.source_buffer.push_str("\ninvalid extra line");
    app.commit_team_source();
    assert_eq!(app.plain_text(), accepted);
    assert_eq!(
        app.team
            .as_ref()
            .expect("team")
            .coordinator
            .status(0)
            .accepted,
        1
    );
    assert!(app.source_buffer.ends_with("invalid extra line"));
}

#[test]
fn offline_and_nonactive_source_cannot_mutate_the_shared_body() {
    let mut app = app();
    let before = app.plain_text();
    app.source_buffer = app.source_buffer.replace("结构渲染夹具", "离线编辑夹具");
    app.team
        .as_mut()
        .expect("team")
        .coordinator
        .set_offline(0, true)
        .expect("offline");
    app.commit_team_source();
    assert_eq!(app.plain_text(), before);
    assert!(app.last_event.contains("NetworkUnreachable"));
    app.team
        .as_mut()
        .expect("team")
        .coordinator
        .set_offline(0, false)
        .expect("online");
    app.source = Session::new(&app.core, Dialect::Typst);
    app.source_buffer = app
        .source
        .generated
        .text
        .replace("结构渲染夹具", "异语言夹具");
    app.commit_team_source();
    assert!(app.last_event.contains("DialectNotActive"));
    assert_eq!(app.plain_text(), before);
}

#[test]
fn barrier_requires_all_members_and_refresh_does_not_rebase_an_old_draft() {
    let mut app = app();
    let before = app.plain_text();
    app.source_buffer = app.source_buffer.replace("结构渲染夹具", "旧草稿夹具");
    let draft = app.source_buffer.clone();
    let team = app.team.as_mut().expect("team");
    team.coordinator
        .request_switch(0, TeamDialect::Typst)
        .expect("switch");
    team.coordinator.acknowledge(0).expect("ack");
    assert!(team.coordinator.complete_switch().is_err());
    app.commit_team_source();
    assert_eq!(app.plain_text(), before);
    assert!(app.last_event.contains("冻结"));
    let team = app.team.as_mut().expect("team");
    for member in [1, 2] {
        team.coordinator.acknowledge(member).expect("ack");
    }
    team.coordinator.complete_switch().expect("complete");
    team.coordinator.refresh(0).expect("permit");
    app.commit_team_source();
    assert!(app.last_event.contains("SourceEpochStale"));
    assert_eq!(app.source_buffer, draft);
    assert_eq!(app.plain_text(), before);
    app.source = Session::new(&app.core, Dialect::Typst);
    app.source_buffer = app
        .source
        .generated
        .text
        .replace("结构渲染夹具", "新语言夹具");
    app.team_regenerated();
    app.commit_team_source();
    assert!(app.plain_text().contains("新语言夹具"));
}
