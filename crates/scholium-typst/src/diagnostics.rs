//! Resolve compiler spans against the current World; never reuse a previous source's ranges.
use crate::SourceBlock;
use scholium_model::diagnostic::{
    Diagnostic, DiagnosticKernel, DiagnosticRange, Severity, SourcePoint, resource_path,
};
use typst::{World, WorldExt, diag::SourceDiagnostic};

/// Convert Typst errors and warnings to kernel-neutral diagnostics.
/// `blocks` describes only the current main generated source; other files never inherit its nodes.
/// A detached/unknown span keeps its message without guessing a source location or node.
#[must_use]
pub fn map_diagnostics(
    world: &dyn World,
    items: &[SourceDiagnostic],
    blocks: &[SourceBlock],
) -> Vec<Diagnostic> {
    items
        .iter()
        .map(|item| {
            let id = item.span.id();
            let source = id.and_then(|id| world.source(id).ok());
            let bytes = world.range(item.span);
            let resource = id.and_then(|id| resource_path(id.vpath().get_without_slash()));
            let range = source
                .as_ref()
                .zip(bytes.as_ref())
                .filter(|_| resource.is_some())
                .and_then(|(source, bytes)| {
                    Some(DiagnosticRange {
                        start: SourcePoint::at(source.text(), bytes.start)?,
                        end: Some(SourcePoint::at(source.text(), bytes.end)?),
                        bytes: Some(bytes.clone()),
                    })
                });
            let node = bytes
                .as_ref()
                .filter(|_| range.is_some() && id == Some(world.main()))
                .and_then(|bytes| {
                    blocks
                        .iter()
                        .find(|block| {
                            block.range.start <= bytes.start
                                && bytes.end <= block.range.end
                                && bytes.start < block.range.end
                        })
                        .map(|block| block.node)
                });
            Diagnostic {
                severity: match item.severity {
                    typst::diag::Severity::Error => Severity::Error,
                    typst::diag::Severity::Warning => Severity::Warning,
                },
                kernel: DiagnosticKernel::Typst,
                resource,
                range,
                node,
                message: item.message.to_string(),
                code: None,
                log: None,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FontProfile, PreviewWorld};
    use scholium_model::NodeId;
    use typst::World;

    #[test]
    fn compiler_error_has_current_file_line_scalar_column_and_exact_node() {
        let world = PreviewWorld::with_fonts(FontProfile::Bundled);
        world.set_source("中文\n#missing_name".into());
        let output = typst::compile::<typst_layout::PagedDocument>(&world);
        let errors = output.output.expect_err("undefined identifier");
        let node = NodeId::fresh();
        let source = world.source(world.main()).expect("source");
        let blocks = [SourceBlock {
            node,
            range: 7..source.text().len(),
        }];
        let mapped = map_diagnostics(&world, &errors, &blocks);
        assert_eq!(mapped[0].resource.as_deref(), Some("main.typ"));
        let range = mapped[0].range.as_ref().expect("range");
        assert_eq!(range.start, SourcePoint { line: 2, column: 2 });
        assert_eq!(range.bytes, Some(8..20));
        assert_eq!(mapped[0].node, Some(node));
        assert_eq!(mapped[0].severity, Severity::Error);
        world.set_source("\n\n#missing_name".into());
        let errors = typst::compile::<typst_layout::PagedDocument>(&world)
            .output
            .expect_err("error");
        let mapped = map_diagnostics(&world, &errors, &[]);
        assert_eq!(mapped[0].range.as_ref().expect("range").start.line, 3);
        assert_eq!(mapped[0].node, None);
    }

    #[test]
    fn resident_compiler_exposes_the_revision_bound_error_and_node() {
        let mut compiler = crate::PreviewCompiler::spawn();
        let node = NodeId::fresh();
        compiler.submit_generated(
            scholium_model::DocumentId::fresh(),
            42,
            crate::GeneratedSource {
                source: "hello\n#missing_name".into(),
                blocks: vec![SourceBlock { node, range: 6..19 }],
                ..Default::default()
            },
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        loop {
            if let Some(crate::PreviewEvent::Compiled(outcome)) = compiler.poll() {
                assert_eq!(outcome.revision, 42);
                assert_eq!(outcome.diagnostics.revision, 42);
                let error = &outcome.diagnostics.items[0];
                assert_eq!(error.resource.as_deref(), Some("main.typ"));
                assert_eq!(error.range.as_ref().expect("range").start.line, 2);
                assert_eq!(error.node, Some(node));
                return;
            }
            assert!(std::time::Instant::now() < deadline, "compile deadline");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn detached_spans_never_guess_locations() {
        let world = PreviewWorld::with_fonts(FontProfile::Bundled);
        let error = SourceDiagnostic::error(typst::syntax::Span::detached(), "detached");
        let mapped = map_diagnostics(&world, &[error], &[]);
        assert!(mapped[0].range.is_none());
        assert!(mapped[0].resource.is_none());
        assert!(mapped[0].node.is_none());
    }
}
