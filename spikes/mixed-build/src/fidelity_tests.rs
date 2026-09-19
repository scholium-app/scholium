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
fn foreign_inline_conversion_accepts_verified_inline_subset() {
    for host in [Dialect::Latex, Dialect::Typst] {
        let mut project = fixtures::make_refs(host);
        project.components[0].body = vec![Block::Equation {
            label: "eq:foreign".to_owned(),
            math: crate::ir::Math::Ident("x".to_owned()),
            display: false,
        }];
        project.components[0].placement = Placement::Inline;
        project.components[0].bridge = Bridge::Convert;
        project.body = vec![Block::IncludeSection {
            component: project.components[0].id.clone(),
        }];
        assert!(
            plan::plan(&project).is_ok(),
            "verified inline subset: {:?}",
            plan::plan(&project).err()
        );
    }
}

#[test]
fn conversion_never_silently_accepts_raw_or_macros() {
    for host in [Dialect::Latex, Dialect::Typst] {
        let mut project = fixtures::make_refs(host);
        let component = &mut project.components[0];
        component.bridge = Bridge::Convert;
        component.body.push(Block::Raw {
            dialect: component.dialect,
            text: "raw".into(),
        });
        let problems = plan::plan(&project).err().expect("conversion rejected");
        assert!(problems.iter().any(|p| p.code == "conversion-unsupported"));
    }
}

#[test]
fn vector_repetition_and_wrong_placement_are_rejected() {
    for host in [Dialect::Latex, Dialect::Typst] {
        let mut project = fixtures::make_refs(host);
        project.body.push(Block::ForeignFigure {
            label: "fig:repeat".into(),
            caption: "repeat".into(),
            component: project.components[0].id.clone(),
        });
        assert!(
            plan::plan(&project)
                .err()
                .expect("duplicate placement")
                .iter()
                .any(|p| p.code == "vector-placement-unsupported")
        );
        project.body.pop();
        project.components[0].placement = Placement::Inline;
        assert!(
            plan::plan(&project)
                .err()
                .expect("wrong placement")
                .iter()
                .any(|p| p.code == "vector-placement-unsupported")
        );
    }
}

#[test]
fn inline_equations_cannot_publish_unresolved_number_references() {
    let mut project = fixtures::make_refs(Dialect::Latex);
    if let Block::Equation { display, .. } = &mut project.body[1] {
        *display = false;
    }
    assert!(
        plan::plan(&project)
            .err()
            .expect("no inline numbering")
            .iter()
            .any(|p| p.code == "inline-target-unsupported")
    );
}
