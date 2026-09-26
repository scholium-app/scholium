use super::*;

// Observations only: this debug/headless host is not a release latency gate.
audit!(
    c01_measure_input_and_warm_compile_at_three_document_sizes,
    {
        for paragraphs in [1, 50, 200] {
            let source =
                vec!["中文 scientific writing with $alpha + x/2$ and ordinary text."; paragraphs]
                    .join("\n");
            let mut h = Harness::new(&source);
            h.compile();
            h.select(0, 0);
            let mut input_ms = Vec::new();
            let mut compile_ms = Vec::new();
            for iteration in 0..40 {
                let start = Instant::now();
                h.frame(vec![Event::Text("x".into())]);
                input_ms.push(start.elapsed().as_secs_f64() * 1000.0);
                if iteration % 4 == 3 {
                    let start = Instant::now();
                    h.compile();
                    compile_ms.push(start.elapsed().as_secs_f64() * 1000.0);
                }
            }
            let text = h.texts()[0].clone();
            assert!(text.starts_with(&"x".repeat(40)), "input lost: {text}");
            sample("input_frame", paragraphs, input_ms);
            sample("compile_wait", paragraphs, compile_ms);
        }
    }
);

fn sample(metric: &str, paragraphs: usize, mut values: Vec<f64>) {
    values.sort_by(f64::total_cmp);
    let percentile = |percent: usize| values[(values.len() * percent).div_ceil(100) - 1];
    println!(
        "METRIC {metric} paragraphs={paragraphs} n={} p50_ms={:.3} p95_ms={:.3} max_ms={:.3}",
        values.len(),
        percentile(50),
        percentile(95),
        values[values.len() - 1]
    );
}
