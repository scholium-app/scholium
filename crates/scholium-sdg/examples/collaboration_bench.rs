//! Incremental and offline merge probes; see the crate README for reproduction.
mod bench_support;
mod collaboration;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(args.len(), 6, "engine mode workload actions-per-side batch");
    let current = env!("CARGO_PKG_NAME") != "scholium-sdg-bench-legacy";
    let count = args[4].parse().expect("action count");
    let batch = args[5].parse().expect("batch size");
    match args[1].as_str() {
        "loro" => collaboration::run::<bench_support::loro::Loro>(
            if current {
                "loro-1.17.1"
            } else {
                "loro-1.16.0"
            },
            &args[2],
            &args[3],
            count,
            batch,
        ),
        "yrs" => collaboration::run::<bench_support::yrs::Yrs>(
            if current { "yrs-0.28.0" } else { "yrs-0.27.4" },
            &args[2],
            &args[3],
            count,
            batch,
        ),
        _ => panic!("expected loro or yrs"),
    }
}
