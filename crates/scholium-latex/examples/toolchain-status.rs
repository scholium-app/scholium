//! Print the same cached status consumed by the future environment UI.
use scholium_latex::toolchain::{DetectionConfig, ToolchainDetector};
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let detector = ToolchainDetector::new(DetectionConfig::from_environment()?);
    let started = Instant::now();
    loop {
        if let Some(status) = detector.cached() {
            let issues: Vec<_> = status
                .issues
                .iter()
                .map(|i| {
                    serde_json::json!({
                        "message": i.message.id, "arguments": i.message.arguments,
                        "action": i.action.id, "action_arguments": i.action.arguments,
                    })
                })
                .collect();
            let engines: Vec<_> = status
                .engines
                .iter()
                .map(|p| {
                    serde_json::json!({
                        "name": p.name, "path": p.path, "version": p.version,
                    })
                })
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "rtex": {"present": status.rtex.present, "valid": status.rtex.valid,
                        "commit": status.rtex.commit, "version": status.rtex.version,
                        "directory": status.rtex.directory},
                    "tex_live": {"found":status.tex_live.found, "version":status.tex_live.version,
                        "kpsewhich":status.tex_live.kpsewhich},
                    "engines":engines, "font_cache_warm":status.font_cache_warm,
                    "development_override":status.development_override, "issues":issues,
                }))?
            );
            return Ok(());
        }
        if started.elapsed() > Duration::from_secs(8) {
            return Err("detector deadline exceeded".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
