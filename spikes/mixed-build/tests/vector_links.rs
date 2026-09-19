//! Inspect final PDFs with the same isolated parser, then compare independently
//! computed coordinate transforms with the source PDF annotations.
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[test]
fn vector_links_and_exact_destinations_survive_both_hosts() {
    for host in ["Latex", "Typst"] {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("out/vector-links-{host}-{}", std::process::id()));
        fs::create_dir_all(&root).expect("fixture");
        fs::write(
            root.join("snapshot.json"),
            serde_json::to_vec(&snapshot(host)).expect("json"),
        )
        .expect("snapshot");
        let run = Command::new(env!("CARGO_BIN_EXE_scholium-spike-mixed-build"))
            .arg("--rebuild")
            .arg(&root)
            .arg(root.join("built"))
            .output()
            .expect("rebuild");
        assert!(
            run.status.success(),
            "{host}: {}",
            String::from_utf8_lossy(&run.stderr)
        );
        let source: Value = serde_json::from_slice(
            &fs::read(root.join("built/foreign-geometry.json")).expect("source"),
        )
        .expect("geometry");
        let final_pdf = inspect(&root);
        assert_geometry(&source, &final_pdf);
        assert_external_text_link(&root);
        println!("{host}: exact vector links verified: {}", root.display());
    }
}
fn assert_external_text_link(root: &Path) {
    // Poppler independently confirms the reconstructed rectangle actually hits
    // painted component text, not just a mutually consistent overlay geometry.
    let run = Command::new("bash")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../toolchain-sandbox.sh"))
        .arg(root.join("input"))
        .arg(root.join("probe"))
        .args([
            "10",
            "/usr/bin/pdftohtml",
            "-xml",
            "-stdout",
            "-i",
            "-q",
            "/project/document.pdf",
        ])
        .env("SCHOLIUM_SANDBOX_PROFILE", "pdf-probe")
        .output()
        .expect("text probe");
    assert!(run.status.success());
    let xml = String::from_utf8(run.stdout).expect("xml");
    fs::write(root.join("links.xml"), &xml).expect("evidence");
    assert!(
        xml.lines()
            .any(|l| l.contains("EXTERNAL") && l.contains("href=\"https://example.org/fidelity\"")),
        "link rectangle does not cover component text: {xml}"
    );
}
fn inspect(root: &Path) -> Value {
    let tools = root.join("tools");
    let input = root.join("input");
    let output = root.join("probe");
    for dir in [&tools, &input, &output] {
        fs::create_dir(dir).expect("dir");
    }
    fs::copy(
        env!("CARGO_BIN_EXE_scholium-spike-mixed-build"),
        tools.join("driver"),
    )
    .expect("driver");
    fs::copy(root.join("built/final.pdf"), input.join("document.pdf")).expect("pdf");
    let run = Command::new("bash")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../toolchain-sandbox.sh"))
        .arg(input)
        .arg(&output)
        .args(["10", "/toolchain/driver", "--vector-worker"])
        .env("SCHOLIUM_SANDBOX_PROFILE", "pdf-probe")
        .env("SCHOLIUM_SPIKE_TOOLS", tools)
        .output()
        .expect("probe");
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    serde_json::from_slice(&fs::read(output.join("geometry.json")).expect("json"))
        .expect("geometry")
}
fn xy(value: &Value) -> [f64; 2] {
    [value[0].as_f64().expect("x"), value[1].as_f64().expect("y")]
}
fn close(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() < 0.3 && (a[1] - b[1]).abs() < 0.3
}
fn assert_geometry(source: &Value, final_pdf: &Value) {
    let a = xy(&source["anchors"]["eq:a"]);
    let b = xy(&source["anchors"]["eq:b"]);
    let fa = xy(&final_pdf["anchors"]["eq:a"]);
    let fb = xy(&final_pdf["anchors"]["eq:b"]);
    assert!(
        (fa[1] - fb[1]).abs() > 2.0,
        "distinct same-page targets collapsed"
    );
    let scale = (fa[1] - fb[1]) / (a[1] - b[1]);
    assert!(scale > 0.0 && scale < 1.0, "fixture must test scaling");
    let transform = |p: [f64; 2]| [fa[0] + scale * (p[0] - a[0]), fa[1] + scale * (p[1] - a[1])];
    let links = final_pdf["links"].as_array().expect("links");
    for p in [fa, fb] {
        assert!(
            links
                .iter()
                .any(|l| l["target"].get("Point").is_some_and(|v| close(xy(v), p))),
            "missing precise host-to-component link"
        );
    }
    let source_links = source["links"].as_array().expect("source links");
    assert!(source_links.len() >= 3);
    for link in source_links {
        if let Some(point) = link["target"].get("Point") {
            let expected = transform(xy(point));
            assert!(
                links.iter().any(|l| l["target"]
                    .get("Point")
                    .is_some_and(|v| close(xy(v), expected))),
                "lost internal exact destination: {expected:?}"
            );
        }
        let r = link["rect"].as_array().expect("rect");
        let lower = transform([r[0].as_f64().expect("x"), r[1].as_f64().expect("y")]);
        let upper = transform([r[2].as_f64().expect("x"), r[3].as_f64().expect("y")]);
        assert!(
            links.iter().any(|l| {
                let rect = l["rect"].as_array().expect("rect");
                let expected = [lower[0], lower[1], upper[0], upper[1]];
                rect.iter()
                    .zip(expected)
                    .all(|(v, e)| (v.as_f64().expect("coordinate") - e).abs() < 1.2)
            }),
            "click rectangle not transformed with vector"
        );
    }
    assert!(
        links
            .iter()
            .any(|l| l["target"]["Uri"] == "https://example.org/fidelity"),
        "external URI lost"
    );
    assert!(
        links.iter().all(|l| !l["target"]
            .get("Uri")
            .is_some_and(|s| s.as_str().expect("uri").starts_with("scholium-ref:"))),
        "temporary cross-document URI leaked"
    );
}
fn snapshot(host: &str) -> Value {
    let foreign = if host == "Latex" { "Typst" } else { "Latex" };
    let eq = |name| json!({"Equation":{"label":name,"math":{"Num":7},"display":true}});
    let reference = |name| json!({"Ref":{"target":name,"page":false}});
    let external = if foreign == "Latex" {
        "\\href{https://example.org/fidelity}{EXTERNAL}"
    } else {
        "#link(\"https://example.org/fidelity\")[EXTERNAL]"
    };
    json!({"schema":"scholium-spike-snapshot-v1","max_rounds":4,"project":{
        "name":"vector-links","host":host,"page":[520,800],"macros":[],"unscoped_control":false,
        "body":[eq("eq:host"),reference("eq:a"),reference("eq:b"),{"ForeignFigure":{"label":"fig:foreign","caption":"vector","component":"foreign"}}],
        "components":[{"id":"foreign","dialect":foreign,"scope":"foreign","bridge":"Vector","placement":"Block","depends_on":[],
            "body":[eq("eq:a"),{"Para":"Spacing between destinations"},eq("eq:b"),reference("eq:a"),reference("eq:b"),reference("eq:host"),{"Raw":{"dialect":foreign,"text":external}}]}]}})
}
