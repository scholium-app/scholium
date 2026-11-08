use super::*;
use scholium_model::{BlockKind, DocumentId, NodeId, Revision, structured::*};

fn document() -> Arc<StructuredDocument> {
    Arc::new(StructuredDocument {
        document: DocumentId::fresh(),
        revision: Revision(7),
        blocks: vec![StructuredBlock {
            node: NodeId::fresh(),
            kind: BlockKind::Paragraph,
            content: vec![
                StructuredInline {
                    node: NodeId::fresh(),
                    body: InlineBody::Text {
                        text: "Export 中文 #literal $text$".into(),
                        style: TextStyle::Plain,
                    },
                },
                StructuredInline {
                    node: NodeId::fresh(),
                    body: InlineBody::Math {
                        root: MathNode {
                            node: NodeId::fresh(),
                            body: MathBody::Fraction {
                                numerator: Box::new(MathNode {
                                    node: NodeId::fresh(),
                                    body: MathBody::Text { text: "a".into() },
                                }),
                                denominator: Box::new(MathNode {
                                    node: NodeId::fresh(),
                                    body: MathBody::Text { text: "b".into() },
                                }),
                            },
                        },
                    },
                },
            ],
        }],
    })
}
fn request(target: PathBuf, input: Input) -> Request {
    Request {
        project: [0; 16],
        entry: "main.typ".into(),
        revision: 7,
        unsaved: true,
        target,
        input,
    }
}
#[test]
fn native_app_export_preserves_current_revision_and_matches_kernel_pdf_pixels() {
    let root = tempfile::tempdir().expect("temp");
    let target = root.path().join("paper.pdf");
    let document = document();
    let generated = scholium_typst::generate_source(&document, scholium_typst::SourceMode::Preview);
    let expected = scholium_typst::export_generated_pdf(&generated)
        .bytes
        .expect("reference PDF");
    let report = worker::run(request(target.clone(), Input::Native(document)));
    assert!(report.success, "{report:?}");
    assert_eq!(report.revision, 7);
    assert!(report.unsaved);
    assert_eq!(report.page_count, 1);
    assert_eq!(
        scholium_latex::view::render_pdf(&std::fs::read(target).expect("file")).expect("actual"),
        scholium_latex::view::render_pdf(&expected).expect("expected")
    );
}
#[test]
fn failed_export_preserves_existing_pdf_and_cleans_temporary_files() {
    let root = tempfile::tempdir().expect("temp");
    let target = root.path().join("paper.pdf");
    std::fs::write(&target, b"old complete PDF").expect("old file");
    let mut doc = document().as_ref().clone();
    doc.blocks[0].content[1].body = InlineBody::RawMath {
        source: "#panic(\"must never execute\")".into(),
    };
    let report = worker::run(request(target.clone(), Input::Native(Arc::new(doc))));
    assert!(!report.success);
    assert!(!report.diagnostics.items.is_empty());
    assert_eq!(
        std::fs::read(&target).expect("preserved"),
        b"old complete PDF"
    );
    assert_eq!(std::fs::read_dir(root.path()).expect("dir").count(), 1);
    let absent = root.path().join("absent/paper.pdf");
    assert!(worker::publish(&absent, b"complete bytes").is_err());
    assert!(!absent.exists());
}
#[test]
fn pdf_publication_rejects_non_pdf_targets_and_symlinks() {
    let root = tempfile::tempdir().expect("temp");
    assert!(worker::publish(&root.path().join("main.typ"), b"PDF").is_err());
    #[cfg(unix)]
    {
        let original = root.path().join("original.pdf");
        std::fs::write(&original, b"old").expect("old");
        let link = root.path().join("link.pdf");
        std::os::unix::fs::symlink(&original, &link).expect("link");
        assert!(worker::publish(&link, b"new").is_err());
        assert_eq!(std::fs::read(original).expect("read"), b"old");
    }
}
#[test]
fn export_diagnostics_merge_without_duplicate_preview_issues_and_expire_on_edit() {
    let mut state = crate::state::WorkspaceState {
        source_language: Some(scholium_storage::project::ProjectLanguage::Typst),
        ..Default::default()
    };
    let issue = crate::diagnostics::failure(DiagnosticKernel::Typst, "syntax error".into());
    state.problems.batch = DiagnosticBatch {
        revision: 7,
        items: vec![issue.clone()],
    };
    state.problems.export = Some(DiagnosticBatch {
        revision: 7,
        items: vec![issue],
    });
    assert_eq!(crate::diagnostics::current_batch(&state).items.len(), 1);
    state.problems.batch = DiagnosticBatch {
        revision: 8,
        items: vec![],
    };
    assert!(crate::diagnostics::current_batch(&state).items.is_empty());
}

#[cfg(test)]
#[path = "native_tests.rs"]
mod native;

#[test]
fn export_report_marks_unsaved_drafts_in_inactive_latex_members() {
    use scholium_storage::project::{ProjectHandle, SourceProject};
    let root = tempfile::tempdir().expect("project");
    std::fs::write(root.path().join("main.tex"), "main").expect("main");
    std::fs::write(root.path().join("chapter.tex"), "chapter").expect("member");
    let handle = ProjectHandle::open(&root.path().join("main.tex"), None).expect("handle");
    let mut source = SourceProject::open(handle).expect("source");
    assert!(!source_unsaved(&source).expect("clean"));
    source
        .select_resource(std::path::Path::new("chapter.tex"))
        .expect("select member");
    source.edit("unsaved member".into()).expect("draft");
    source
        .select_resource(std::path::Path::new("main.tex"))
        .expect("return to clean entry");
    assert!(!source.dirty(), "current entry is clean");
    assert!(source_unsaved(&source).expect("report sees all exported drafts"));
}
