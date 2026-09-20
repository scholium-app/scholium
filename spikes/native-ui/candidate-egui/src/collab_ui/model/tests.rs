use super::*;

#[test]
fn delayed_reversed_duplicate_updates_converge_and_undo_keeps_remote() -> Result<()> {
    let mut pair = Pair::new()?;
    pair.clients[0].draft.insert_str(0, "LOCAL");
    pair.apply(0)?;
    pair.clients[0].draft.push_str("TAIL");
    pair.apply(0)?;
    pair.clients[1].draft.insert_str(0, "REMOTE");
    pair.apply(1)?;
    assert_ne!(pair.clients[0].text(), pair.clients[1].text());
    assert_eq!(pair.pending(), 3);
    pair.deliver(true, true)?;
    assert_eq!(pair.clients[0].text(), pair.clients[1].text());
    assert!(pair.clients[0].text().contains("LOCAL"));
    pair.undo(0)?;
    pair.deliver(true, true)?;
    let text = pair.clients[0].text();
    assert_eq!(text, pair.clients[1].text());
    assert!(text.contains("REMOTE") && text.contains("LOCAL") && !text.contains("TAIL"));
    pair.undo(0)?;
    pair.deliver(false, false)?;
    assert_eq!(pair.clients[0].text(), "REMOTE共同文本");
    assert_eq!(pair.clients[0].text(), pair.clients[1].text());
    Ok(())
}

#[test]
fn dirty_draft_survives_remote_delivery_and_cannot_overwrite_it() -> Result<()> {
    let mut pair = Pair::new()?;
    pair.clients[0].draft = "DRAFT共同文本".into();
    pair.clients[1].draft = "远端共同文本".into();
    pair.apply(1)?;
    pair.deliver(false, true)?;
    assert_eq!(pair.clients[0].draft, "DRAFT共同文本");
    assert!(pair.apply(0).is_err());
    assert_eq!(pair.clients[0].text(), "远端共同文本");
    assert_eq!(pair.gate.status(0).accepted, 1);
    Ok(())
}

#[test]
fn language_barrier_drains_accepted_updates_and_quarantines_old_drafts() -> Result<()> {
    let mut pair = Pair::new()?;
    pair.clients[0].draft = "ACCEPTED共同文本".into();
    pair.apply(0)?;
    pair.clients[1].draft = "OLD共同文本".into();
    pair.switch()?;
    assert!(pair.finish_switch().is_err());
    assert!(pair.apply(1).is_err());
    pair.clients[1].composing = true;
    assert!(pair.deliver(false, false).is_err());
    assert!(pair.finish_switch().is_err());
    pair.clients[1].composing = false;
    pair.deliver(true, true)?;
    pair.finish_switch()?;
    assert_eq!(pair.gate.status(0).epoch, 2);
    assert_eq!(pair.clients[1].draft, "OLD共同文本");
    assert!(pair.apply(1).is_err());
    assert!(
        pair.undo(0).is_err(),
        "old-language undo needs a current permit"
    );
    pair.regenerate(1)?;
    pair.clients[1].draft.push_str("新语言");
    pair.apply(1)?;
    pair.deliver(false, false)?;
    assert_eq!(pair.clients[0].text(), pair.clients[1].text());
    Ok(())
}

#[test]
fn unicode_splices_and_unsupported_input_are_explicit() -> Result<()> {
    let mut pair = Pair::new()?;
    pair.clients[0].draft = "中𐐀共同文本".into();
    pair.apply(0)?;
    pair.clients[0].draft = "中𐐀文本".into();
    pair.apply(0)?;
    pair.deliver(true, true)?;
    assert_eq!(pair.clients[1].text(), "中𐐀文本");
    pair.clients[1].draft.push_str("\\input{bad}");
    assert!(pair.apply(1).is_err());
    assert_eq!(pair.gate.status(0).accepted, 2);
    Ok(())
}

#[test]
fn regenerating_same_epoch_draft_does_not_erase_local_undo() -> Result<()> {
    let mut pair = Pair::new()?;
    pair.clients[0].draft.push_str("local");
    pair.apply(0)?;
    pair.clients[0].draft.push_str("discard");
    pair.regenerate(0)?;
    pair.undo(0)?;
    pair.deliver(true, true)?;
    assert_eq!(pair.clients[0].text(), "共同文本");
    assert_eq!(pair.clients[0].text(), pair.clients[1].text());
    Ok(())
}

#[test]
fn offline_rejection_preserves_draft_and_creates_no_update() -> Result<()> {
    let mut pair = Pair::new()?;
    pair.gate.set_offline(0, true)?;
    pair.clients[0].draft.push_str("offline");
    assert!(pair.apply(0).is_err());
    assert_eq!(pair.clients[0].text(), "共同文本");
    assert_eq!(pair.clients[0].draft, "共同文本offline");
    assert_eq!(pair.pending(), 0);
    assert_eq!(pair.gate.status(0).accepted, 0);
    Ok(())
}
