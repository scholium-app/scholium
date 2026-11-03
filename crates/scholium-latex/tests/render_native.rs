//! Product E4/E7 acceptance. Missing native dependencies fail; never silently skip.
use scholium_build::{ResourceLimits, RtexToolchain, Sandbox};
use scholium_latex::{
    CompileOutcome, LiveConfig, LiveEvent, LiveSession, SourceSession,
    view::{self, Drawing, RasterOrigin, Renderer},
};
use std::{path::PathBuf, sync::Arc, time::Duration};
#[path = "../src/tests/pixels.rs"]
mod pixels;

fn required(key: &str) -> PathBuf {
    std::env::var_os(key)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("未运行：缺少 {key}"))
}
fn evidence() -> PathBuf {
    let p = required("SCHOLIUM_LATEX_EVIDENCE");
    std::fs::create_dir_all(&p).expect("evidence");
    p
}
async fn layout(live: &mut LiveSession, revision: u64) -> Arc<Drawing> {
    tokio::time::timeout(Duration::from_secs(90), async {
        loop {
            match live.next_event().await.expect("event") {
                LiveEvent::Pages {
                    revision: Some(r),
                    outcome: CompileOutcome::Complete,
                    drawing,
                    ..
                } if r == revision && drawing.reference_pdf().is_some() => return drawing,
                LiveEvent::Diagnostic { message } => eprintln!("diagnostic: {message}"),
                LiveEvent::Engine {
                    state: scholium_latex::EngineState::Failed,
                    reason,
                    ..
                } => panic!("engine failed: {reason:?}"),
                _ => {}
            }
        }
    })
    .await
    .expect("layout deadline")
}
fn compare(name: &str, page: &view::RasterPage, pdf: &[u8]) {
    let root = evidence();
    let pdf_path = root.join(format!("{name}.pdf"));
    std::fs::write(&pdf_path, pdf).expect("PDF");
    let prefix = root.join(name);
    assert!(
        std::process::Command::new("pdftoppm")
            .args(["-r", "300", "-gray", "-f", "1", "-l", "1", "-singlefile"])
            .arg(&pdf_path)
            .arg(&prefix)
            .status()
            .expect("poppler")
            .success()
    );
    let reference = pixels::read_pgm(&prefix.with_extension("pgm")).expect("reference");
    let diff = pixels::compare(&pixels::gray(page), &reference);
    eprintln!("{name}: {diff:?}, origin={:?}", page.origin);
    std::fs::write(
        root.join(format!("{name}.json")),
        serde_json::to_vec_pretty(&diff).expect("JSON"),
    )
    .expect("evidence");
    tiny_skia::Pixmap::from_vec(
        page.rgba.to_vec(),
        tiny_skia::IntSize::from_wh(page.width, page.height).expect("size"),
    )
    .expect("pixmap")
    .save_png(root.join(format!("{name}.png")))
    .expect("PNG");
    assert!(diff.ink_a > 0 && diff.ink_b > 0);
    assert!(diff.unmatched_fraction <= 0.01, "E4 {name}: {diff:?}");
}
#[tokio::test]
#[ignore = "requires pinned rtex, TeX, sandbox and independent Poppler"]
async fn english_chinese_live_edits_and_tikz_fallback_match_reference_pdf() {
    let sandbox = Arc::new(
        Sandbox::prepare(
            RtexToolchain {
                binary: required("SCHOLIUM_RTEX_BIN"),
                tex_directory: required("SCHOLIUM_RTEX_TEXDIR"),
                font_cache: required("SCHOLIUM_RTEX_FONT_CACHE"),
            },
            ResourceLimits::default(),
        )
        .await
        .expect("native sandbox"),
    );
    for (name, source, fallback) in [
        (
            "english",
            "\\documentclass{article}\n\\usepackage{amsmath}\n\\begin{document}\nFirst ordinary paragraph with inline $\\alpha+\\frac{1}{2}$ math and enough words.\n\nSecond ordinary paragraph remains unchanged.\n\\end{document}\n",
            false,
        ),
        (
            "chinese",
            "\\documentclass{ctexart}\n\\begin{document}\n这是中文段落，包含 ordinary English words 与公式 $\\alpha+1$，编辑后局部更新。\n\n第二段保持不变，并包含更多中文内容。\n\\end{document}\n",
            false,
        ),
        (
            "tikz",
            "\\documentclass{article}\n\\usepackage{tikz}\n\\begin{document}\n\\begin{tikzpicture}\\draw[red,thick] (0,0) rectangle (2,1);\\end{tikzpicture}\n\\end{document}\n",
            true,
        ),
    ] {
        let project = tempfile::tempdir().expect("project");
        let work = tempfile::tempdir().expect("work");
        std::fs::write(project.path().join("main.tex"), source).expect("source");
        let session = SourceSession::open(project.path(), "main.tex").expect("source");
        let mut live =
            LiveSession::start(session, sandbox.clone(), work.path(), LiveConfig::default())
                .await
                .expect("live");
        let initial = layout(&mut live, 0).await;
        let mut renderer = Renderer::default();
        let before = renderer
            .apply(&initial, None)
            .expect("initial pixels")
            .remove(0);
        compare(
            &format!("{name}-initial"),
            &before,
            initial.reference_pdf().expect("PDF"),
        );
        assert_eq!(
            before.origin,
            if fallback {
                RasterOrigin::PdfFallback
            } else {
                RasterOrigin::DisplayList
            }
        );
        if !fallback {
            let start = source.find("ordinary").expect("marker");
            live.edit("main.tex", start..start, "new ")
                .await
                .expect("edit");
            let paragraph = tokio::time::timeout(Duration::from_secs(30), async {
                loop {
                    if let Some(LiveEvent::Paragraph {
                        revision: Some(1),
                        drawing,
                        paragraph,
                        ..
                    }) = live.next_event().await
                    {
                        break (drawing, paragraph);
                    }
                }
            })
            .await
            .expect("paragraph deadline");
            let after = renderer
                .apply(&paragraph.0, Some(paragraph.1))
                .expect("live pixels")
                .remove(0);
            assert_ne!(before.rgba, after.rgba);
            assert_eq!(after.origin, RasterOrigin::DisplayList);
            live.request_layout().await.expect("reference layout");
            let reference = layout(&mut live, 1).await;
            compare(
                &format!("{name}-edited-live"),
                &after,
                reference.reference_pdf().expect("PDF"),
            );
        }
        live.cancel().await.expect("cancel");
    }
}
#[tokio::test]
#[ignore = "requires XeLaTeX/pdfLaTeX, sandbox and Poppler"]
async fn non_lua_engines_compile_immutable_revision_with_full_preview_origin_and_deadline() {
    let sandbox = Sandbox::prepare_full(
        required("SCHOLIUM_RTEX_FONT_CACHE"),
        ResourceLimits::default(),
    )
    .await
    .expect("full sandbox");
    for engine in ["xelatex", "pdflatex"] {
        let project = tempfile::tempdir().expect("project");
        let source = format!(
            "% !TeX program = {engine}\n\\documentclass{{article}}\n\\begin{{document}}Full preview $x^2$ old.\\end{{document}}\n"
        );
        let path = project.path().join("main.tex");
        std::fs::write(&path, &source).expect("source");
        let mut session = SourceSession::open(project.path(), "main.tex").expect("session");
        let start = source.find("old").expect("marker");
        session
            .edit("main.tex", start..start + 3, "NEW")
            .expect("edit");
        let preview = scholium_latex::full::compile(&session, &sandbox, Duration::from_secs(30))
            .await
            .expect("compile");
        assert_eq!(preview.revision, 1);
        assert_eq!(preview.engine, session.engine());
        assert!(!preview.pages.is_empty());
        assert!(
            preview
                .pages
                .iter()
                .all(|p| p.origin == RasterOrigin::FullCompilation)
        );
        assert!(preview.elapsed < Duration::from_secs(30));
        assert_eq!(std::fs::read_to_string(&path).expect("disk"), source);
        eprintln!(
            "{engine} full preview revision={} elapsed_ms={}",
            preview.revision,
            preview.elapsed.as_millis()
        );
        session.edit("main.tex",0..session.text("main.tex").expect("text").len(),&format!("% !TeX program = {engine}\n\\documentclass{{article}}\\begin{{document}}\\loop\\iftrue\\repeat\\end{{document}}")).expect("loop");
        let started = std::time::Instant::now();
        let error = scholium_latex::full::compile(&session, &sandbox, Duration::from_millis(150))
            .await
            .expect_err("infinite TeX must time out");
        assert!(
            matches!(
                error,
                scholium_latex::LatexError::Build(scholium_build::BuildError::Timeout(_))
            ),
            "{error:?}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "deadline must cancel whole tree promptly"
        );
    }
}
