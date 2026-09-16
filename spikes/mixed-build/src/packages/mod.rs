//! Experimental snapshot packages; this is not a persistent product format.
mod clean;
use std::fs;
use std::io;
use std::path::Path;
use std::process::ExitCode;

use crate::{
    build,
    diag::Expect,
    fixtures,
    ir::{Bridge, Dialect, Project},
    plan, verify,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    schema: String,
    max_rounds: usize,
    project: Project,
}

pub(crate) fn dispatch() -> Option<ExitCode> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = args.first()?;
    let result = match mode.as_str() {
        "--packages" if args.len() == 1 => verify_packages(),
        "--rebuild" if args.len() == 3 => rebuild(Path::new(&args[1]), Path::new(&args[2])),
        "--packages" | "--rebuild" => Err(io::Error::other(
            "usage: --packages | --rebuild PACKAGE OUTPUT",
        )),
        _ => return None,
    };
    Some(match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("[FAIL] {error}");
            ExitCode::FAILURE
        }
    })
}

fn validate(project: &Project) -> io::Result<()> {
    for component in &project.components {
        let name = &component.id;
        if name.is_empty()
            || name == "."
            || name == ".."
            || name == "main"
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
        {
            return Err(io::Error::other("unsafe or reserved component filename"));
        }
    }
    Ok(())
}

fn rebuild(package: &Path, output: &Path) -> io::Result<()> {
    let snapshot: Snapshot = serde_json::from_slice(&fs::read(package.join("snapshot.json"))?)?;
    if snapshot.schema != "scholium-spike-snapshot-v1" || !(1..=8).contains(&snapshot.max_rounds) {
        return Err(io::Error::other("unsupported schema or round budget"));
    }
    validate(&snapshot.project)?;
    if output.exists() {
        return Err(io::Error::other("rebuild output must not exist"));
    }
    let plan =
        plan::plan(&snapshot.project).map_err(|errors| io::Error::other(format!("{errors:?}")))?;
    let result = build::build(&snapshot.project, &plan, output, snapshot.max_rounds);
    if result.outcome != crate::diag::Outcome::Success {
        return Err(io::Error::other(format!("{:?}", result.diagnostics)));
    }
    Ok(())
}

fn extension(dialect: Dialect) -> &'static str {
    match dialect {
        Dialect::Latex => "tex",
        Dialect::Typst => "typ",
    }
}

fn export(project: &Project, built: &Path, package: &Path, mixed: bool) -> io::Result<()> {
    validate(project)?;
    fs::create_dir(package)?;
    let main = format!("main.{}", extension(project.host));
    fs::copy(built.join(&main), package.join(&main))?;
    for component in &project.components {
        let source = format!("{}.{}", component.id, extension(component.dialect));
        fs::copy(built.join(&source), package.join(&source))?;
        if !mixed && matches!(component.bridge, Bridge::Vector) {
            let artifact = format!("{}.pdf", component.id);
            fs::copy(built.join(&artifact), package.join(&artifact))?;
        }
    }
    if mixed {
        let snapshot = Snapshot {
            schema: "scholium-spike-snapshot-v1".to_owned(),
            max_rounds: build::MAX_ROUNDS,
            project: project.clone(),
        };
        fs::write(
            package.join("snapshot.json"),
            serde_json::to_vec_pretty(&snapshot)?,
        )?;
    }
    let command = match (mixed, project.host) {
        (true, _) => "scholium-spike-mixed-build --rebuild . ../rebuilt",
        (false, Dialect::Latex) => "xelatex -no-shell-escape main.tex (run twice)",
        (false, Dialect::Typst) => "typst compile --root . main.typ final.pdf",
    };
    fs::write(
        package.join("BUILD.txt"),
        format!(
            "Experimental snapshot, not a product storage format.\nBuild: {command}\n\
         Dependencies: TeX Live 2026 (XeLaTeX, ctex/xeCJK, pgfplots, hyperref, standalone), Typst 0.15.1 where needed; Noto Serif CJK SC and Latin Modern fonts.\n\
         Standard package needs only its target compiler. Foreign source regeneration needs the other compiler.\n\
         Mixed package needs this version of the spike driver plus XeLaTeX; Typst 0.15.1 is linked in the driver.\n\
         Mixed snapshot.json is the authority for regeneration; included source is a preserved generated projection. Hand-editing that projection is not imported.\n\
         Embedded foreign content stays artifact-only in the target language. Internal component links/reflow are not preserved.\n"
        ),
    )?;
    Ok(())
}

fn verify_packages() -> io::Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("out/packages");
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    fs::create_dir_all(&root)?;
    let tools = clean::tools(&root)?;
    let mut count = 0;
    for fixture in fixtures::all()
        .iter()
        .filter(|f| f.expect == Expect::Success && !f.leak_expected)
    {
        for host in fixture.hosts {
            let project = (fixture.make)(*host);
            let name = format!("{}-{}", fixture.id, host.name());
            let built = root.join(format!("{name}-original"));
            let evidence = verify::run(fixture, *host, &built);
            if !evidence.passed() {
                return Err(io::Error::other(format!("original {name} failed")));
            }
            for mixed in [false, true] {
                let kind = if mixed { "mixed" } else { "standard" };
                let package = root.join(format!("{name}-{kind}"));
                export(&project, &built, &package, mixed)?;
                clean::rebuild_and_compare(&package, &built, &tools, *host, mixed)?;
                println!("[PASS] {name}/{kind}: clean rebuild, pages/text/links/vector checked");
                count += 1;
                if fixture.id == "G2-plot-foreign" && !mixed {
                    let broken = root.join(format!("{name}-missing-resource"));
                    export(&project, &built, &broken, false)?;
                    let vector = project
                        .components
                        .iter()
                        .find(|c| matches!(c.bridge, Bridge::Vector))
                        .ok_or_else(|| io::Error::other("missing vector fixture"))?;
                    fs::remove_file(broken.join(format!("{}.pdf", vector.id)))?;
                    let error = clean::rebuild_and_compare(&broken, &built, &tools, *host, false)
                        .expect_err("missing component must prevent reconstruction");
                    if !error.to_string().contains("clean build") {
                        return Err(error);
                    }
                    println!("[PASS] {name}: missing component rejected by compiler");
                }
            }
        }
    }
    println!(
        "packages: {count}/{count} passed; artifacts {}",
        root.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn traversal_component_is_rejected_before_export() {
        let project = crate::fixtures::all()
            .into_iter()
            .find(|f| f.id == "G2-plot-foreign")
            .expect("fixture");
        let mut project = (project.make)(crate::ir::Dialect::Latex);
        project.components[0].id = "../escape".to_owned();
        assert!(super::validate(&project).is_err());
    }
}
