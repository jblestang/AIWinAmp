use crate::audio::AudioEngine;
use crate::equalizer::Equalizer;
use crate::playlist::Playlist;
use crate::visuals::VisualMeter;
use anyhow::Result;

/// TransportState mirrors Winamp's classic play/pause/stop triad.
#[derive(Clone, Debug, PartialEq)]
pub enum TransportState {
    /// Nothing is playing and progress is reset.
    Stopped,
    /// Audio is playing and the float stores elapsed seconds.
    Playing { progress: f32 },
    /// Playback is paused but progress is remembered for resume.
    Paused { progress: f32 },
}

/// WinampCore centralizes playlist, transport, equalizer, and telemetry state.
pub struct WinampCore {
    /// Playlist provides deterministic ordering and metadata.
    playlist: Playlist,
    /// Audio backend can be rodio or a test double.
    audio: Box<dyn AudioEngine>,
    /// Current transport state powering buttons and the progress bar.
    transport: TransportState,
    /// User facing master volume stored as 0.0-1.0 float.
    volume: f32,
    /// Equalizer data drives sliders and subtle visual changes.
    equalizer: Equalizer,
    /// Visual meter synthesizes a faux UV display.
    visual_meter: VisualMeter,
    /// Cached last visual level to avoid recomputing mid frame.
    visual_level: f32,
}

impl WinampCore {
    /// Builds a core with provided playlist and audio backend.
    pub fn new(playlist: Playlist, audio: Box<dyn AudioEngine>) -> Self {
        // Start in the stopped state to mimic Winamp boot.
        Self {
            playlist,
            audio,
            transport: TransportState::Stopped,
            volume: 0.8,
            equalizer: Equalizer::new(),
            visual_meter: VisualMeter::new(0xAA55AA55),
            visual_level: 0.0,
        }
    }

    /// Exposes immutable playlist reference for UI rendering.
    pub fn playlist(&self) -> &Playlist {
        &self.playlist
    }

    /// Exposes mutable playlist reference for drag and drop interactions.
    pub fn playlist_mut(&mut self) -> &mut Playlist {
        &mut self.playlist
    }

    /// Returns the equalizer for slider rendering and editing.
    pub fn equalizer_mut(&mut self) -> &mut Equalizer {
        &mut self.equalizer
    }

    /// Returns current equalizer snapshot for display only contexts.
    pub fn equalizer(&self) -> &Equalizer {
        &self.equalizer
    }

    /// Returns current transport state.
    pub fn transport(&self) -> &TransportState {
        &self.transport
    }

    /// Returns current master volume.
    pub fn volume(&self) -> f32 {
        self.volume
    }

    /// Updates master volume and forwards it to the audio backend.
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
        self.audio.set_volume(self.volume);
    }

    /// Starts playback or resumes from pause while resetting progress when needed.
    pub fn play(&mut self) -> Result<()> {
        if self.playlist.is_empty() {
            return Ok(());
        }
        let progress = match self.transport {
            TransportState::Paused { progress } => progress,
            _ => 0.0,
        };
        if let Some(track) = self.playlist.active() {
            self.audio.play(track)?;
            self.transport = TransportState::Playing { progress };
        }
        Ok(())
    }

    /// Pauses playback but keeps the elapsed time for quick resume.
    pub fn pause(&mut self) {
        if let TransportState::Playing { progress } = self.transport {
            self.transport = TransportState::Paused { progress };
            self.audio.stop();
        }
    }

    /// Stops playback and resets progress to the start of the track.
    pub fn stop(&mut self) {
        self.audio.stop();
        self.transport = TransportState::Stopped;
    }

    /// Advances to the next track and optionally auto plays it.
    pub fn next(&mut self, autoplay: bool) -> Result<()> {
        if self.playlist.is_empty() {
            return Ok(());
        }
        self.playlist.advance();
        if autoplay {
            self.play()
        } else {
            self.transport = TransportState::Stopped;
            Ok(())
        }
    }

    /// Rewinds to the previous track.
    pub fn previous(&mut self, autoplay: bool) -> Result<()> {
        if self.playlist.is_empty() {
            return Ok(());
        }
        self.playlist.rewind();
        if autoplay {
            self.play()
        } else {
            self.transport = TransportState::Stopped;
            Ok(())
        }
    }

    /// Ticks the transport clock and loops tracks seamlessly.
    pub fn tick(&mut self, delta_seconds: f32) -> Result<()> {
        if let TransportState::Playing { progress } = &mut self.transport {
            *progress += delta_seconds.max(0.0);
            if let Some(track) = self.playlist.active() {
                if *progress >= track.duration_secs as f32 {
                    self.playlist.advance();
                    self.transport = TransportState::Playing { progress: 0.0 };
                    if let Some(next_track) = self.playlist.active() {
                        self.audio.play(next_track)?;
                    }
                }
            }
        }
        let normalized_transport = match self.transport {
            TransportState::Playing { progress } => self.progress_from_secs(progress),
            TransportState::Paused { progress } => self.progress_from_secs(progress),
            TransportState::Stopped => 0.0,
        };
        self.visual_level = self.visual_meter.sample(
            normalized_transport,
            self.equalizer.average_gain().abs() / 12.0,
        );
        Ok(())
    }

    /// Returns 0-1 progress for the currently active track.
    pub fn progress(&self) -> f32 {
        match self.transport {
            TransportState::Playing { progress } | TransportState::Paused { progress } => {
                self.progress_from_secs(progress)
            }
            TransportState::Stopped => 0.0,
        }
    }

    /// Returns the last sampled visual level.
    pub fn visual_level(&self) -> f32 {
        self.visual_level
    }

    /// Helper converting seconds to normalized progress.
    fn progress_from_secs(&self, secs: f32) -> f32 {
        if let Some(track) = self.playlist.active() {
            (secs / track.duration_secs.max(1) as f32).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playlist::Track;
    use parking_lot::Mutex;

    /// MockAudioEngine keeps track invocations for deterministic tests.
    struct MockAudioEngine {
        /// Captures last played track id for assertions.
        last_played: Mutex<Option<u64>>,
    }

    impl MockAudioEngine {
        fn new() -> Self {
            Self {
                last_played: Mutex::new(None),
            }
        }
    }

    impl AudioEngine for MockAudioEngine {
        fn play(&self, track: &Track) -> Result<()> {
            *self.last_played.lock() = Some(track.id);
            Ok(())
        }

        fn stop(&self) {
            *self.last_played.lock() = None;
        }

        fn set_volume(&self, _volume: f32) {}
    }

    /// R3:T3 - play transitions from stopped to playing and hits backend.
    #[test]
    fn play_starts_transport() {
        let mut playlist = Playlist::new();
        playlist.add_track(Track::demo("Demo", 1));
        let mut core = WinampCore::new(playlist, Box::new(MockAudioEngine::new()));
        core.play().unwrap();
        assert!(matches!(core.transport(), TransportState::Playing { .. }));
    }

    /// R3:T3 - tick advances and wraps to the next track.
    #[test]
    fn tick_wraps_tracks() {
        let mut playlist = Playlist::new();
        playlist.add_track(Track::demo("One", 1));
        playlist.add_track(Track::demo("Two", 2));
        let mut core = WinampCore::new(playlist, Box::new(MockAudioEngine::new()));
        core.play().unwrap();
        core.tick(10_000.0).unwrap();
        assert_eq!(core.playlist().active_index(), Some(1));
    }

    /// R3:T3 - next and previous honor autoplay flag.
    #[test]
    fn navigation_respects_autoplay() {
        let mut playlist = Playlist::new();
        playlist.add_track(Track::demo("One", 1));
        playlist.add_track(Track::demo("Two", 2));
        let mut core = WinampCore::new(playlist, Box::new(MockAudioEngine::new()));
        core.next(false).unwrap();
        assert_eq!(core.playlist().active_index(), Some(1));
        assert!(matches!(core.transport(), TransportState::Stopped));
        core.previous(false).unwrap();
        assert_eq!(core.playlist().active_index(), Some(0));
    }
}
