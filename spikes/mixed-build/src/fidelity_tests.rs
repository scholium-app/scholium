use crate::{
    fixtures,
    ir::{Block, Bridge, Dialect, Placement},
    plan,
};

#[test]
fn inline_source_include_rejects_block_content() {
    for host in [Dialect::Latex, Dialect::Typst] {
        let mut project = fixtures::make_refs(host);
        let component = &mut project.components[0];
        component.dialect = host;
        component.bridge = Bridge::Include;
        component.placement = Placement::Inline;
        component.body = vec![Block::PageBreak];
        let problems = plan::plan(&project).err().expect("reject block content");
        assert!(
            problems
                .iter()
                .any(|p| p.code == "inline-content-unsupported")
        );
    }
}

#[test]
fn foreign_inline_vector_is_still_explicitly_rejected() {
    for host in [Dialect::Latex, Dialect::Typst] {
        let mut project = fixtures::make_refs(host);
        project.components[0].placement = Placement::Inline;
        let problems = plan::plan(&project)
            .err()
            .expect("unsupported vector baseline");
        assert!(
            problems
                .iter()
                .any(|p| p.code == "inline-embed-unsupported")
        );
    }
}
