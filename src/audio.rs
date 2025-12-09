use crate::playlist::Track;
use anyhow::Result;
use parking_lot::Mutex;

/// AudioEngine abstraction allows the UI to remain agnostic of playback backend.
pub trait AudioEngine: Send + Sync {
    /// Requests playback of a specific track asset.
    fn play(&self, track: &Track) -> Result<()>;
    /// Stops any playing audio immediately.
    fn stop(&self);
    /// Applies master volume changes.
    fn set_volume(&self, volume: f32);
}

/// SilentEngine simulates playback for deterministic CI and headless environments.
#[derive(Default)]
pub struct SilentEngine {
    /// Stores the last track title for telemetry and testing.
    last_track: Mutex<Option<String>>,
    /// Caches the most recent volume value for inspection.
    volume: Mutex<f32>,
}

impl SilentEngine {
    /// Creates a new silent engine with zeroed telemetry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the last requested track, primarily for assertions.
    pub fn last_track(&self) -> Option<String> {
        self.last_track.lock().clone()
    }
}

impl AudioEngine for SilentEngine {
    fn play(&self, track: &Track) -> Result<()> {
        *self.last_track.lock() = Some(track.title.clone());
        Ok(())
    }

    fn stop(&self) {
        *self.last_track.lock() = None;
    }

    fn set_volume(&self, volume: f32) {
        *self.volume.lock() = volume.clamp(0.0, 1.0);
    }
}
