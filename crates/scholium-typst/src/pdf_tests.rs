use super::*;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{
    PixmapSettings, RenderCache, RenderSettings, hayro_interpret::InterpreterSettings,
    hayro_syntax::Pdf,
};

fn pixels(bytes: &[u8]) -> Vec<(u32, u32, Vec<u8>)> {
    let pdf = Pdf::new(bytes.to_vec()).expect("PDF");
    pdf.pages()
        .iter()
        .map(|page| {
            let pixels = hayro::render(
                page,
                &RenderCache::new(),
                &InterpreterSettings::default(),
                &RenderSettings::default(),
                &PixmapSettings {
                    x_scale: 300.0 / 72.0,
                    y_scale: 300.0 / 72.0,
                    bg_color: WHITE,
                },
            );
            (
                pixels.width().into(),
                pixels.height().into(),
                pixels.data_as_u8_slice().to_vec(),
            )
        })
        .collect()
}
#[test]
fn native_pdf_matches_direct_fork_library_with_zero_pixel_tolerance() {
    let generated = GeneratedSource {
        source: format!(
            "{}\nHello 中文 $frac(a,b)$\n#pagebreak()\nSecond page",
            crate::preamble()
        ),
        ..Default::default()
    };
    let exported = export_generated_pdf(&generated);
    let world = PreviewWorld::new();
    world.set_source(generated.source);
    let document = typst::compile::<PagedDocument>(&world)
        .output
        .expect("direct compile");
    let direct =
        typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default()).expect("direct PDF library");
    assert_eq!(exported.page_count, 2);
    assert_eq!(
        pixels(&exported.bytes.expect("app API PDF")),
        pixels(&direct)
    );
}
#[test]
fn worker_pdf_matches_direct_fork_library_on_the_same_resource_snapshot() {
    let files = BTreeMap::from([
        (
            "main.typ".into(),
            b"#include \"part.typ\"\n$ frac(1,2) $".to_vec(),
        ),
        ("part.typ".into(), b"Hello reference".to_vec()),
    ]);
    let exported = export_source_pdf("main.typ", &files).expect("snapshot");
    let world = crate::source_preview::snapshot_world("main.typ", &files).expect("world");
    let document = typst::compile::<PagedDocument>(&world)
        .output
        .expect("direct compile");
    let direct = typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default()).expect("direct PDF");
    assert_eq!(exported.page_count, document.pages().len());
    assert_eq!(
        pixels(&exported.bytes.expect("worker API PDF")),
        pixels(&direct)
    );
}
#[test]
fn broken_source_and_unfilled_slots_never_return_pdf_bytes() {
    let files = BTreeMap::from([("main.typ".into(), b"#let broken = (".to_vec())]);
    let exported = export_source_pdf("main.typ", &files).expect("envelope");
    assert!(exported.bytes.is_none());
    assert!(
        exported
            .diagnostics
            .iter()
            .any(|d| d.resource.as_deref() == Some("main.typ") && d.range.is_some())
    );
    let generated = GeneratedSource {
        holes: 1,
        ..Default::default()
    };
    assert!(export_generated_pdf(&generated).bytes.is_none());
}

#[test]
fn only_designed_fallback_font_warnings_are_dropped_from_native_export() {
    use scholium_model::diagnostic::{Diagnostic, DiagnosticKernel, Severity};
    let warning = |message: &str| Diagnostic {
        severity: Severity::Warning,
        kernel: DiagnosticKernel::Typst,
        resource: None,
        range: None,
        node: None,
        message: message.into(),
        code: None,
        log: None,
    };
    assert!(super::missing_fallback_family(&warning(
        "unknown font family: times new roman"
    )));
    assert!(super::missing_fallback_family(&warning(
        "unknown font family: simhei"
    )));
    assert!(!super::missing_fallback_family(&warning(
        "unknown font family: comic sans"
    )));
    assert!(!super::missing_fallback_family(&warning(
        "unknown variable: x"
    )));
    let mut error = warning("unknown font family: simsun");
    error.severity = Severity::Error;
    assert!(!super::missing_fallback_family(&error));
}
