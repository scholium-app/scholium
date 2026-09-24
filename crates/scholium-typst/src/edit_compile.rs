//! Tolerant on-page feedback for incomplete formulas; never rewrites authority.
use crate::{
    PreviewWorld,
    projection::{self, Projection},
};
use scholium_model::DocumentSnapshot;
use typst::WorldExt;
use typst_layout::PagedDocument;

pub(super) fn compile(
    world: &PreviewWorld,
    mut projection: Projection,
    snapshot: Option<&DocumentSnapshot>,
) -> typst::diag::SourceResult<(PagedDocument, Projection, Option<String>)> {
    let mut drafts = Vec::new();
    let mut messages = Vec::new();
    loop {
        world.set_source(projection.source.clone());
        match typst::compile::<PagedDocument>(world).output {
            Ok(document) => {
                return Ok((
                    document,
                    projection,
                    (!messages.is_empty()).then(|| messages.join("\n")),
                ));
            }
            Err(diagnostics) => {
                let error = diagnostics
                    .iter()
                    .map(|d| d.message.to_string())
                    .collect::<Vec<_>>()
                    .join("\n");
                let Some(snapshot) = snapshot else {
                    return Err(diagnostics);
                };
                let mut affected = Vec::new();
                for diagnostic in &diagnostics {
                    let Some(range) = world.range(diagnostic.span) else {
                        return Err(diagnostics);
                    };
                    let Some(formula) = projection.formulas.iter().find(|formula| {
                        formula.source.start <= range.start && range.end <= formula.source.end
                    }) else {
                        return Err(diagnostics);
                    };
                    affected.push(formula.key);
                }
                if affected.is_empty() {
                    return Err(diagnostics);
                }
                for key in affected {
                    if !drafts.contains(&key) {
                        drafts.push(key);
                    }
                }
                messages.push(error);
                // Each retry removes at least one failing formula from strict
                // evaluation. Healthy formulas keep their normal layout.
                projection = projection::generate_with_drafts(snapshot, true, &drafts);
            }
        }
    }
}
