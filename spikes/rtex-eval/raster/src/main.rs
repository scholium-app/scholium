//! T18 evaluation prototype (not product code): rasterize rtex display lists with tiny-skia and
//! skrifa, render PDF pages with hayro, and compare ink against an independent reference.
//!
//! ```text
//! rtex-eval-raster pages <layout.json> <outdir> [--dpi N] [--repeat N]  pages → page-N.pgm
//! rtex-eval-raster pdf <file.pdf> <page> <out.pgm> [--dpi N] hayro render of one page
//! rtex-eval-raster compare <a.pgm> <b.pgm>                   tolerant ink comparison
//! rtex-eval-raster serve [--dpi N]                          JSON lines: rasterize events
//! ```
//! Every command prints JSON on stdout.

mod compare;
mod dl;
mod fonts;
mod paint;
mod pdf;

use serde_json::{Value, json};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
}

fn dpi(args: &[String]) -> f64 {
    flag(args, "--dpi")
        .and_then(|v| v.parse().ok())
        .unwrap_or(150.0)
}

/// Page display lists of a `LayoutUpdate` event, or one bare page list.
fn page_lists(value: &Value) -> Vec<(Value, bool)> {
    match value.get("pages_changed").and_then(|p| p.as_array()) {
        Some(pages) => pages
            .iter()
            .filter_map(|p| {
                Some((
                    p.get("dl")?.clone(),
                    p.get("exact")?.as_bool().unwrap_or(false),
                ))
            })
            .collect(),
        None => vec![(value.clone(), true)],
    }
}

fn rasterize_page(
    dl: &Value,
    exact: bool,
    dpi: f64,
    cache: &mut fonts::FontCache,
    out: Option<&Path>,
) -> Value {
    let started = Instant::now();
    let list = dl::DisplayList::from_value(dl);
    let parse_us = started.elapsed().as_micros();
    let started = Instant::now();
    let Some((pixmap, stats)) = paint::page(&list, dpi, cache) else {
        return json!({"error": "page list without page size"});
    };
    let raster_us = started.elapsed().as_micros();
    let page = dl.get("page").and_then(|v| v.as_i64()).unwrap_or(0);
    if let Some(dir) = out {
        let gray = compare::Gray {
            width: pixmap.width() as usize,
            height: pixmap.height() as usize,
            ink: paint::ink(&pixmap),
        };
        let path = dir.join(format!("page-{page}.pgm"));
        if let Err(error) = compare::write_pgm(&path, &gray) {
            return json!({"page": page, "error": error.to_string()});
        }
    }
    json!({"page": page, "exact": exact, "parse_us": parse_us, "raster_us": raster_us,
           "size": [pixmap.width(), pixmap.height()], "stats": stats,
           "missing_fonts": cache.missing.iter().collect::<Vec<_>>()})
}

fn pages(args: &[String]) -> Result<(), String> {
    let (input, outdir) = (
        args.first().ok_or("input")?,
        PathBuf::from(args.get(1).ok_or("outdir")?),
    );
    let text = std::fs::read_to_string(input).map_err(|e| format!("{input}: {e}"))?;
    let value: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&outdir).map_err(|e| e.to_string())?;
    let mut cache = fonts::FontCache::default();
    // `--repeat N`: the first round pays font loading (cold), later rounds reuse the caches (warm).
    let rounds = flag(args, "--repeat")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1usize)
        .max(1);
    for round in 1..=rounds {
        for (dl, exact) in page_lists(&value) {
            let out = (round == rounds).then_some(outdir.as_path());
            let mut line = rasterize_page(&dl, exact, dpi(args), &mut cache, out);
            line["round"] = json!(round);
            println!("{line}");
        }
    }
    Ok(())
}

fn pdf_page(args: &[String]) -> Result<(), String> {
    let (file, page, out) = (
        args.first().ok_or("pdf")?,
        args.get(1).ok_or("page")?,
        args.get(2).ok_or("out")?,
    );
    let page: usize = page.parse().map_err(|_| "page must be a number")?;
    let rendered = pdf::render(Path::new(file), page, dpi(args))?;
    compare::write_pgm(Path::new(out), &rendered.gray).map_err(|e| e.to_string())?;
    println!(
        "{}",
        json!({"page": page, "load_us": rendered.load_us, "render_us": rendered.render_us,
                          "size": [rendered.gray.width, rendered.gray.height]})
    );
    Ok(())
}

fn compare_images(args: &[String]) -> Result<(), String> {
    let a = compare::read_pgm(Path::new(args.first().ok_or("a")?))?;
    let b = compare::read_pgm(Path::new(args.get(1).ok_or("b")?))?;
    println!(
        "{}",
        serde_json::to_string(&compare::compare(&a, &b)).map_err(|e| e.to_string())?
    );
    Ok(())
}

/// One request per line: `{"seq": n, "event": <ParagraphUpdate | LayoutUpdate>}`. The font and
/// outline caches stay warm across requests, as in a long-lived host.
fn serve(args: &[String]) -> Result<(), String> {
    let mut cache = fonts::FontCache::default();
    let stdout = std::io::stdout();
    for line in std::io::stdin().lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        let started = Instant::now();
        let request: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        let parse_us = started.elapsed().as_micros();
        let seq = request.get("seq").cloned().unwrap_or(Value::Null);
        let event = request.get("event").cloned().unwrap_or(Value::Null);
        let reply = match event.get("event").and_then(|e| e.as_str()) {
            Some("ParagraphUpdate") => {
                let started = Instant::now();
                let list = dl::DisplayList::from_value(event.get("dl").unwrap_or(&Value::Null));
                let fragments = event
                    .get("fragments")
                    .and_then(|f| f.as_array())
                    .cloned()
                    .unwrap_or_default();
                let (pixmaps, stats) = paint::unit(&list, &fragments, dpi(args), &mut cache);
                let pixels: u64 = pixmaps
                    .iter()
                    .map(|(_, p)| p.width() as u64 * p.height() as u64)
                    .sum();
                json!({"seq": seq, "kind": "unit", "parse_us": parse_us,
                       "raster_us": started.elapsed().as_micros(), "pixels": pixels, "stats": stats})
            }
            Some("LayoutUpdate") => {
                let started = Instant::now();
                let pages: Vec<Value> = page_lists(&event)
                    .iter()
                    .map(|(dl, exact)| rasterize_page(dl, *exact, dpi(args), &mut cache, None))
                    .collect();
                json!({"seq": seq, "kind": "layout", "parse_us": parse_us,
                       "raster_us": started.elapsed().as_micros(), "pages": pages})
            }
            _ => json!({"seq": seq, "kind": "ignored"}),
        };
        let mut out = stdout.lock();
        writeln!(out, "{reply}")
            .and_then(|_| out.flush())
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest = args.get(1..).unwrap_or(&[]);
    let result = match args.first().map(String::as_str) {
        Some("pages") => pages(rest),
        Some("pdf") => pdf_page(rest),
        Some("compare") => compare_images(rest),
        Some("serve") => serve(rest),
        _ => Err("usage: rtex-eval-raster pages|pdf|compare|serve …".to_string()),
    };
    if let Err(error) = result {
        eprintln!("rtex-eval-raster: {error}");
        std::process::exit(2);
    }
}
