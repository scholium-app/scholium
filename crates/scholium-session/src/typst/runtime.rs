//! Session-owned derived layout worker and complete Scene adoption.
use super::*;
use scholium_typst::editor::{EditorError, EditorWorker, Raster, StripUpdate};
use std::time::{Duration, Instant};

pub(super) struct Runtime {
    worker: EditorWorker,
    token: u64,
    pub(super) settle: raster::Settle,
    pub(super) sent: Option<(Instant, ViewRange)>,
    pub(super) shown_scale: Option<f64>,
}

impl EditorCore {
    /// Open a session with the real layout worker and a host wake callback.
    ///
    /// # Errors
    /// Returns storage/validation failures or failure to spawn the layout thread.
    pub fn open_with_wake(
        path: &Path,
        wake: impl Fn() + Send + 'static,
    ) -> Result<Self, SessionError> {
        Self::open_with_fonts(path, wake, scholium_typst::FontProfile::from_environment())
    }

    /// Open the same runtime with explicit, immutable font sources.
    ///
    /// # Errors
    /// Returns storage/validation failures or failure to spawn the layout thread.
    pub fn open_with_fonts(
        path: &Path,
        wake: impl Fn() + Send + 'static,
        fonts: scholium_typst::FontProfile,
    ) -> Result<Self, SessionError> {
        let mut core = Self::open(path)?;
        core.runtime = Some(Runtime {
            worker: EditorWorker::spawn_with_fonts(wake, fonts)?,
            token: 0,
            settle: Default::default(),
            sent: None,
            shown_scale: None,
        });
        Ok(core)
    }

    /// Submit the accepted snapshot and invalidate geometry until its Scene arrives.
    pub fn submit_layout(&mut self) {
        self.wanted = self.scene_stamp();
        self.set_geometry(None, None);
        if let Some(runtime) = &mut self.runtime {
            runtime.token = runtime.token.wrapping_add(1);
            runtime
                .worker
                .submit(self.snapshot.clone(), self.wanted, self.raster, self.view);
            runtime.sent = Some((Instant::now(), self.view));
        }
    }

    /// Poll the worker and adopt only the complete wanted identity.
    pub fn poll_layout(&mut self) -> bool {
        let result = self.runtime.as_ref().and_then(|r| r.worker.poll());
        result.is_some_and(|(stamp, outcome)| self.adopt_scene(stamp, outcome))
    }

    /// Adopt a completed layout through the same validation used by worker polling.
    pub fn adopt_scene(
        &mut self,
        stamp: SceneStamp,
        outcome: Result<EditorScene, EditorError>,
    ) -> bool {
        if stamp != self.wanted {
            return false;
        }
        match outcome {
            Ok(scene) => {
                if let Some(runtime) = &mut self.runtime {
                    runtime.settle.arrived();
                    runtime.shown_scale = Some(scene.sharp.px_per_pt);
                }
                self.overflows = self.find_overflows(&scene);
                self.set_geometry(Some(scene.geometry.clone()), Some(stamp));
                self.scene = Some(scene);
                true
            }
            Err(error) => {
                self.layout_error = Some(error.to_string());
                false
            }
        }
    }

    /// Last adopted Scene; it may remain visible while a fresh layout is pending.
    pub fn current_scene(&self) -> Option<&EditorScene> {
        self.scene.as_ref()
    }

    /// Whether the adopted Scene can supply current pointer and navigation geometry.
    pub fn scene_is_current(&self) -> bool {
        self.scene.as_ref().is_some_and(|s| s.stamp == self.wanted) && self.layout_error.is_none()
    }

    /// Drop the old picture when a history/new-document operation changes lifetimes.
    pub fn clear_scene(&mut self) {
        self.scene = None;
        self.set_geometry(None, None);
    }

    /// Synchronize immutable geometry from the session's adopted Scene.
    pub fn sync_scene_geometry(&mut self) {
        let current = self.scene.as_ref().filter(|s| s.stamp == self.wanted);
        let geometry = current.map(|s| s.geometry.clone());
        let stamp = current.map(|s| s.stamp);
        self.set_geometry(geometry, stamp);
    }

    /// Move raster strips to the host without transferring Scene ownership.
    pub fn take_scene_rasters(&mut self) -> Option<(Raster, Raster)> {
        fn take(raster: &mut Raster) -> Raster {
            Raster {
                px_per_pt: raster.px_per_pt,
                width: raster.width,
                height: raster.height,
                strips: std::mem::take(&mut raster.strips),
            }
        }
        let scene = self.scene.as_mut()?;
        Some((take(&mut scene.base), take(&mut scene.sharp)))
    }

    /// Opaque generation for platform drag invalidation.
    pub fn scene_token(&self) -> u64 {
        self.runtime.as_ref().map_or(0, |r| r.token)
    }

    /// Audit identity strings without exposing the stamp comparison policy to the host.
    pub fn scene_identities(&self) -> (String, Option<String>) {
        (
            format!("{:?}", self.wanted),
            self.scene.as_ref().map(|s| format!("{:?}", s.stamp)),
        )
    }

    /// Current visible range, measured in page points.
    pub fn visible_range(&self) -> ViewRange {
        self.view
    }
    /// Current raster request.
    pub fn raster_spec(&self) -> RasterSpec {
        self.raster
    }

    /// Texture-layer feedback: the sharp scale actually displayed after replacement coverage.
    pub fn raster_displayed(&mut self, scale: Option<f64>) {
        if let Some(runtime) = &mut self.runtime {
            runtime.shown_scale = scale;
        }
    }

    pub(super) fn viewport_changed(&mut self) -> Option<Duration> {
        let scene = self.scene.as_ref()?;
        let runtime = self.runtime.as_mut()?;
        let shown = runtime.shown_scale?;
        let now = Instant::now();
        let wanted = self.raster.resolve(scene.size_pt);
        let mut wait = None;
        match runtime.settle.step(now, wanted, Some(shown)) {
            raster::Step::Request => {
                runtime.worker.set_view(self.raster, self.view);
                runtime.sent = Some((now, self.view));
                return None;
            }
            raster::Step::Wait(after) => wait = Some(after),
            raster::Step::Idle => {}
        }
        const THROTTLE: Duration = Duration::from_millis(40);
        const EPSILON_PT: f64 = 0.5;
        let unsent = runtime.sent.is_none_or(|(_, sent)| {
            (sent.top_pt - self.view.top_pt).abs() >= EPSILON_PT
                || (sent.bottom_pt - self.view.bottom_pt).abs() >= EPSILON_PT
        });
        if !unsent {
            return wait;
        }
        let due = runtime.sent.map_or(Duration::ZERO, |(at, _)| {
            THROTTLE.saturating_sub(now.saturating_duration_since(at))
        });
        if due.is_zero() {
            runtime.worker.set_view(
                RasterSpec {
                    px_per_pt: shown,
                    ..self.raster
                },
                self.view,
            );
            runtime.sent = Some((now, self.view));
        } else {
            wait = Some(wait.map_or(due, |after| after.min(due)));
        }
        wait
    }

    /// Drain completed strips, filtering out any other Scene's raster.
    pub fn poll_strips(&mut self) -> Vec<StripUpdate> {
        let strips = self
            .runtime
            .as_ref()
            .map_or_else(Vec::new, |r| r.worker.poll_strips());
        if !strips.is_empty()
            && let Some(runtime) = &mut self.runtime
        {
            runtime.settle.arrived();
        }
        strips
            .into_iter()
            .filter(|update| self.strip_is_current(update.stamp))
            .collect()
    }

    /// Whether a strip belongs to the last adopted Scene.
    pub fn strip_is_current(&self, stamp: SceneStamp) -> bool {
        self.scene
            .as_ref()
            .is_some_and(|scene| scene.stamp == stamp)
    }
}
