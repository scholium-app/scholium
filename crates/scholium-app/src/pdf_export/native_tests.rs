//! Opt-in host/sandbox comparison; missing installed engines fail instead of passing as skipped.
use super::*;
use std::{process::Command, time::Instant};

#[test]
#[ignore = "requires pdflatex, xelatex, lualatex, TeX fonts and FullCompilation sandbox"]
fn all_latex_export_engines_match_plain_engine_pixels_including_unsaved_chinese() {
    for (engine, chinese) in [("pdflatex", false), ("xelatex", false), ("lualatex", true)] {
        assert!(
            Command::new(engine)
                .arg("--version")
                .output()
                .expect("未运行：缺少 TeX 引擎")
                .status
                .success()
        );
        let project = tempfile::tempdir().expect("project");
        let plain = tempfile::tempdir().expect("plain build");
        let app_output = tempfile::tempdir().expect("export");
        let source = if chinese {
            "\\documentclass[UTF8,fontset=fandol]{ctexart}\n\\begin{document}\n中文与 English mixed text $\\frac{a}{b}+1$.\n\\end{document}\n"
        } else {
            "\\documentclass{article}\n\\begin{document}\nEnglish export $\\frac{a}{b}+1$.\n\\end{document}\n"
        };
        let edited = source.replace("English", "Unsaved English");
        let main = project.path().join("main.tex");
        std::fs::write(&main, source).expect("source");
        let mut snapshot = scholium_latex::SourceSession::open(project.path(), "main.tex")
            .expect("snapshot")
            .with_engine(match engine {
                "pdflatex" => scholium_latex::Engine::PdfLatex,
                "xelatex" => scholium_latex::Engine::XeLatex,
                _ => scholium_latex::Engine::LuaLatex,
            });
        snapshot
            .edit("main.tex", 0..source.len(), &edited)
            .expect("unsaved edit");
        let target = app_output.path().join("result.pdf");
        let started = Instant::now();
        let report = worker::run(request(target.clone(), Input::Latex(snapshot)));
        assert!(report.success, "{engine}: {report:?}");
        assert!(!report.bibliography_processed);
        assert_eq!(
            std::fs::read_to_string(&main).expect("unchanged disk"),
            source
        );
        std::fs::write(plain.path().join("main.tex"), edited).expect("reference source");
        for _ in 0..2 {
            let output = Command::new(engine)
                .current_dir(plain.path())
                .args([
                    "-interaction=nonstopmode",
                    "-halt-on-error",
                    "-no-shell-escape",
                    "-jobname=preview",
                    "main.tex",
                ])
                .output()
                .expect("plain engine");
            assert!(
                output.status.success(),
                "{engine}: {}",
                String::from_utf8_lossy(&output.stdout)
            );
        }
        let actual = scholium_latex::view::render_pdf(&std::fs::read(target).expect("app PDF"))
            .expect("actual pixels");
        let reference = scholium_latex::view::render_pdf(
            &std::fs::read(plain.path().join("preview.pdf")).expect("plain PDF"),
        )
        .expect("reference pixels");
        assert_eq!(actual.len(), reference.len(), "{engine} pages");
        for (a, b) in actual.iter().zip(&reference) {
            assert_eq!((a.width, a.height), (b.width, b.height));
            // Identical engine, fonts and hayro at 300 dpi: zero differing bytes is required.
            assert_eq!(a.rgba, b.rgba, "{engine} rendered pages must be identical");
        }
        eprintln!(
            "{engine}: {} pages, 300 dpi, 0 differing RGBA bytes; {:.2}s",
            actual.len(),
            started.elapsed().as_secs_f64()
        );
    }
}
