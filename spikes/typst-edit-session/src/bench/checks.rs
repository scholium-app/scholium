//! Every incremental frame is checked against a full-tree projection and layout.

use crate::{
    kernel::{ProbeWorld, layout_in, page, render_options},
    session,
};
use scholium_spike_core::Document;
use typst::{
    foundations::Content,
    layout::{Abs, Frame, Size},
};

pub(super) fn compare(
    world: &ProbeWorld,
    document: &Document,
    content: &Content,
    frame: &Frame,
    size: Size,
    pixels: bool,
) -> Result<(), String> {
    let full = session::reference::project(document).map_err(|e| e.to_string())?;
    assert_eq!(*content, full);
    let reference = layout_in(world, &full, size)?;
    // Hash includes printable items, positions, cluster ranges, carets and edit bounds.
    assert_eq!(
        typst::utils::hash128(frame),
        typst::utils::hash128(&reference)
    );
    if pixels {
        let actual = typst_render::render(&page(frame), &render_options());
        let expected = typst_render::render(&page(&reference), &render_options());
        assert_eq!(actual.data(), expected.data());
        assert_eq!(actual.width(), expected.width());
        assert_eq!(actual.height(), expected.height());
    }
    Ok(())
}

pub(super) fn profiles(world: &mut ProbeWorld) -> Result<(), String> {
    let (document, _) = super::fixture::paragraphs(64, "profile");
    let mut session = session::ContentSession::default();
    session
        .apply(session::Update::initial(&document).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let content = session.content().map_err(|e| e.to_string())?;
    let first = layout_in(world, &content, super::region())?;
    let size = Size::new(Abs::pt(260.0), Abs::pt(40000.0));
    let before = typst_layout::editor_paragraph_layouts();
    let narrower = layout_in(world, &content, size)?;
    assert!(typst_layout::editor_paragraph_layouts() - before >= 64);
    assert_ne!(
        typst::utils::hash128(&first),
        typst::utils::hash128(&narrower)
    );
    compare(world, &document, &content, &narrower, size, true)?;
    world.library.styles.set(
        typst::text::TextElem::size,
        typst::text::TextSize(Abs::pt(17.0).into()),
    );
    let before = typst_layout::editor_paragraph_layouts();
    let styled = layout_in(world, &content, size)?;
    assert!(typst_layout::editor_paragraph_layouts() - before >= 64);
    compare(world, &document, &content, &styled, size, true)?;
    println!("PASS width_and_font_size_invalidate_layout_with_cached_content");
    Ok(())
}
