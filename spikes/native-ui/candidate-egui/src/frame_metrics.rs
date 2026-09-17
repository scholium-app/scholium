//! Opt-in eframe CPU timing for real-window evidence; excludes GPU/compositor presentation.
#[derive(Default)]
pub(super) struct FrameMetrics {
    samples: Vec<f64>,
}

impl FrameMetrics {
    pub(super) fn observe(&mut self, frame: &eframe::Frame) {
        if std::env::var_os("SCHOLIUM_SPIKE_FRAME_METRICS").is_none() {
            return;
        }
        let Some(seconds) = frame.info().cpu_usage else {
            return;
        };
        self.samples.push(f64::from(seconds) * 1000.0);
        const WINDOW: usize = 120;
        if self.samples.len() == WINDOW {
            self.samples.sort_by(f64::total_cmp);
            println!(
                "[frame-cpu] samples={WINDOW} p95_ms={:.3} max_ms={:.3} excludes_gpu_present=true",
                self.samples[WINDOW * 95 / 100],
                self.samples[WINDOW - 1]
            );
            self.samples.clear();
        }
    }
}
