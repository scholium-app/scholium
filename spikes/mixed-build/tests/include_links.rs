//! Actual rebuilds: included source shares the host's native label namespace.
use serde_json::json;
use std::{fs, path::PathBuf, process::Command};

#[test]
fn included_internal_symbols_link_both_ways_in_both_hosts() {
    for dialect in ["Latex", "Typst"] {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "out/include-links-{dialect}-{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("fixture");
        let snapshot = snapshot(dialect);
        fs::write(
            root.join("snapshot.json"),
            serde_json::to_vec(&snapshot).expect("json"),
        )
        .expect("write");
        let output = Command::new(env!("CARGO_BIN_EXE_scholium-spike-mixed-build"))
            .arg("--rebuild")
            .arg(&root)
            .arg(root.join("rebuilt"))
            .output()
            .expect("driver");
        assert!(
            output.status.success(),
            "{dialect}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let xml = inspect_pdf(&root);
        fs::write(root.join("links.xml"), &xml).expect("evidence");
        let pages: Vec<_> = xml.split("<page ").skip(1).collect();
        assert_eq!(pages.len(), 2, "{dialect}: pages");
        assert!(
            pages[0].contains("#2\""),
            "{dialect}: host link must reach internal symbol on page 2: {xml}"
        );
        assert!(
            pages[1].contains("#1\""),
            "{dialect}: included link must reach host on page 1: {xml}"
        );
        assert!(!xml.contains("??"), "unresolved output");
        assert_inline(&xml);
        println!(
            "{dialect}: native include links 1→2 and 2→1 verified; {}",
            root.display()
        );
    }
}

#[test]
fn foreign_inline_vector_preserves_host_line_and_destination() {
    for host in ["Latex", "Typst"] {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("out/foreign-inline-{host}-{}", std::process::id()));
        fs::create_dir_all(&root).expect("fixture");
        fs::write(
            root.join("snapshot.json"),
            serde_json::to_vec(&foreign_inline_snapshot(host)).expect("json"),
        )
        .expect("write");
        let output = Command::new(env!("CARGO_BIN_EXE_scholium-spike-mixed-build"))
            .args(["--rebuild"])
            .arg(&root)
            .arg(root.join("rebuilt"))
            .output()
            .expect("driver");
        assert!(
            output.status.success(),
            "{host}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let xml = inspect_pdf(&root);
        assert_inline(&xml);
        assert!(
            xml.contains("#1\""),
            "{host}: inline destination missing: {xml}"
        );
    }
}

fn assert_inline(xml: &str) {
    let xml = xml
        .replace("<i>", "")
        .replace("</i>", "")
        .replace("<a href=\"final.html#1\">", "")
        .replace("<a href=\"final.html#2\">", "")
        .replace("</a>", "");
    let left = xml
        .lines()
        .find(|line| line.contains("INLINELEFT"))
        .expect("left text");
    let right = xml
        .lines()
        .find(|line| line.contains("INLINERIGHT") || line.contains("INLINERIGH"))
        .expect("right text");
    let top = |line: &str| {
        line.split("top=\"")
            .nth(1)
            .expect("top")
            .split('"')
            .next()
            .expect("value")
            .parse::<i32>()
            .expect("coordinate")
    };
    assert_eq!(
        top(left),
        top(right),
        "inline component forced a line break: {xml}"
    );
    let math = xml
        .lines()
        .find(|line| line.contains(">x</text>") || line.contains(">𝑥</text>"))
        .expect("inline formula");
    let left_x = coordinate(left, "left");
    let math_x = coordinate(math, "left");
    let right_x = coordinate(right, "left");
    assert!(
        left_x < math_x && math_x < right_x,
        "formula is not between the words"
    );
    assert!(
        top(math) < top(left) + coordinate(left, "height")
            && top(left) < top(math) + coordinate(math, "height"),
        "formula is on another line"
    );
}

fn inspect_pdf(root: &std::path::Path) -> String {
    let input = root.join("rebuilt");
    let probe = root.join("probe");
    fs::create_dir_all(&probe).expect("probe");
    let output = Command::new("bash")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../toolchain-sandbox.sh"))
        .args([input.as_os_str(), probe.as_os_str()])
        .args([
            "10",
            "/usr/bin/pdftohtml",
            "-xml",
            "-stdout",
            "-i",
            "-q",
            "/project/final.pdf",
        ])
        .env("SCHOLIUM_SANDBOX_PROFILE", "pdf-probe")
        .output()
        .expect("probe");
    assert!(output.status.success(), "pdf inspection failed");
    String::from_utf8(output.stdout).expect("utf8")
}

fn coordinate(line: &str, name: &str) -> i32 {
    line.split(&format!("{name}=\""))
        .nth(1)
        .expect("attribute")
        .split('"')
        .next()
        .expect("value")
        .parse()
        .expect("coordinate")
}

fn snapshot(dialect: &str) -> serde_json::Value {
    let equation = |label| json!({"Equation":{"label":label,"math":{"Num":7},"display":true}});
    let reference = |target| json!({"Ref":{"target":target,"page":true}});
    json!({"schema":"scholium-spike-snapshot-v1","max_rounds":4,
        "project":{"name":"include-links","host":dialect,"page":[420,720],
        "macros":[],"unscoped_control":false,
        "body":[{"Para":"INLINELEFT"},{"IncludeSection":{"component":"inline"}},
            {"Para":"INLINERIGHT"},equation("eq:host"),reference("eq:inside"),"PageBreak",{"IncludeSection":{"component":"local"}}],
        "components":[{"id":"local","dialect":dialect,"scope":"local","bridge":"Include",
            "placement":"Block","depends_on":[],"body":[equation("eq:inside"),reference("eq:host")]},
            {"id":"inline","dialect":dialect,"scope":"inline","bridge":"Include","placement":"Inline",
            "depends_on":[],"body":[{"Equation":{"label":"eq:inline","math":{"Ident":"x"},"display":false}}]}]}})
}

fn foreign_inline_snapshot(host: &str) -> serde_json::Value {
    let foreign = if host == "Latex" { "Typst" } else { "Latex" };
    let mut value = snapshot(host);
    let components = value["project"]["components"]
        .as_array_mut()
        .expect("components");
    components[1]["dialect"] = json!(foreign);
    components[1]["bridge"] = json!("Vector");
    components[1]["body"] = json!([{"Equation":{"label":"eq:inline","math":{"Ident":"x"},"display":false}}, {"Ref":{"target":"eq:host","page":true}}]);
    value
}
