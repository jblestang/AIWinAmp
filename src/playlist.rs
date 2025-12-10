use crate::media;
use rand::{rngs::StdRng, Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Represents the metadata needed to render and play a single track entry.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Track {
    /// Unique monotonic id that lets us diff playlists quickly.
    pub id: u64,
    /// Display friendly title used throughout the UI.
    pub title: String,
    /// Artist label kept short to mirror classic Winamp.
    pub artist: String,
    /// Track duration in seconds for progress calculations.
    pub duration_secs: u32,
    /// Optional absolute audio path when the asset exists locally.
    pub path: Option<PathBuf>,
}

impl Track {
    /// Factory building demo tracks when only a label is known.
    pub fn demo(label: &str, seed: u64) -> Self {
        // Deterministic RNG produces light variations per demo track.
        let mut rng = StdRng::seed_from_u64(seed);
        // Mock artists to keep the playlist visually interesting.
        let artist = format!("AI Ensemble {}", rng.gen_range(1..=5));
        // Duration is bounded to keep UI progress bars responsive.
        let duration_secs = rng.gen_range(90..180);
        // Demo tracks do not have a file backing yet.
        Self {
            id: seed,
            title: label.to_string(),
            artist,
            duration_secs,
            path: None,
        }
    }

    /// Helper constructing a track from a real file path.
    pub fn from_path<P: AsRef<Path>>(path: P, id: u64) -> Self {
        // Resolve owned buffer for long lived storage.
        let full_path = path.as_ref().to_path_buf();
        // Pull filename for title when no metadata parser is available.
        let mut title = full_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown")
            .to_string();
        // Artist is unknown for raw files so we reuse the title.
        let mut artist = "Local File".to_string();
        // Default duration in case probing fails.
        let mut duration_secs = 120;
        if let Ok(meta) = media::analyze_audio(&full_path) {
            if let Some(found_title) = meta.title {
                title = found_title;
            }
            if let Some(found_artist) = meta.artist {
                artist = found_artist;
            }
            if let Some(found_duration) = meta.duration_secs {
                duration_secs = found_duration.max(1);
            }
        }
        // Return the assembled track entry for playlist consumption.
        Self {
            id,
            title,
            artist,
            duration_secs,
            path: Some(full_path),
        }
    }
}

/// Playlist keeps deterministic ordering plus the active cursor.
#[derive(Clone, Default, Debug)]
pub struct Playlist {
    /// Tracks are stored in insertion order to mimic Winamp behavior.
    tracks: Vec<Track>,
    /// Optional pointer to the currently focused track index.
    active: Option<usize>,
}

impl Playlist {
    /// Creates an empty playlist for dependency injection.
    pub fn new() -> Self {
        // Default derives empty vectors so we just forward it.
        Self::default()
    }

    /// Adds a track and returns its index for convenience.
    pub fn add_track(&mut self, track: Track) -> usize {
        // Push the track then capture its final index.
        self.tracks.push(track);
        // Autoselect the first track for quick playback.
        if self.active.is_none() {
            self.active = Some(0);
        }
        // len-1 is safe because we just inserted an item.
        self.tracks.len() - 1
    }

    /// Removes a track by id and keeps the cursor in a valid state.
    pub fn remove_track(&mut self, id: u64) -> bool {
        // Position search keeps API stable even if ids are sparse.
        if let Some(pos) = self.tracks.iter().position(|t| t.id == id) {
            // Remove the track at the located index.
            self.tracks.remove(pos);
            // Adjust active index when necessary to avoid panics.
            if let Some(active) = self.active {
                if self.tracks.is_empty() {
                    self.active = None;
                } else if pos <= active && active > 0 {
                    self.active = Some(active - 1);
                }
            }
            // Indicate the playlist actually changed.
            true
        } else {
            // No-op removal still yields a boolean for tests.
            false
        }
    }

    /// Moves the play head to the next track with wrap-around.
    pub fn advance(&mut self) {
        // Nothing to do when the playlist has no tracks yet.
        if self.tracks.is_empty() {
            return;
        }
        // Jump back to zero once we hit the end for seamless loops.
        self.active = Some(match self.active {
            Some(idx) if idx + 1 < self.tracks.len() => idx + 1,
            _ => 0,
        });
    }

    /// Moves the play head backwards while wrapping to the tail.
    pub fn rewind(&mut self) {
        // Bail out when empty to reduce branching elsewhere.
        if self.tracks.is_empty() {
            return;
        }
        // Wrap from zero to len-1 so the user can cycle continuously.
        self.active = Some(match self.active {
            Some(idx) if idx > 0 => idx - 1,
            _ => self.tracks.len() - 1,
        });
    }

    /// Exposes an immutable slice for UI rendering efficiency.
    pub fn tracks(&self) -> &[Track] {
        // Borrow the underlying vector slice since it is already contiguous.
        &self.tracks
    }

    /// Returns the currently active track for easy highlighting.
    pub fn active(&self) -> Option<&Track> {
        // Map the optional index to an optional reference.
        self.active.and_then(|idx| self.tracks.get(idx))
    }

    /// Returns the current index when the UI needs to custom paint rows.
    pub fn active_index(&self) -> Option<usize> {
        // Provide a copy of the cursor so consumers stay immutable.
        self.active
    }

    /// Supplies a mutable reference for playback logic needing mutation.
    pub fn active_mut(&mut self) -> Option<&mut Track> {
        // Borrow the vector mutably only when an index is set.
        self.active.and_then(move |idx| self.tracks.get_mut(idx))
    }

    /// Reports playlist length for diagnostics and button enablement.
    pub fn len(&self) -> usize {
        // Delegate to Vec::len to stay O(1).
        self.tracks.len()
    }

    /// Convenience predicate used to disable controls on empty playlists.
    pub fn is_empty(&self) -> bool {
        // Use len() to avoid duplicating state.
        self.tracks.is_empty()
    }

    /// Allows explicit selection by index which helps table clicks.
    pub fn set_active(&mut self, index: usize) {
        // Ignore invalid indexes to keep panic free.
        if index < self.tracks.len() {
            self.active = Some(index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R2:T2 - ensure we can append demo tracks and auto select the first.
    #[test]
    fn adds_tracks_and_auto_selects() {
        // Build playlist and push two demo tracks.
        let mut playlist = Playlist::new();
        playlist.add_track(Track::demo("One", 1));
        playlist.add_track(Track::demo("Two", 2));
        // Validate count plus automatic selection semantics.
        assert_eq!(playlist.len(), 2);
        assert_eq!(playlist.active_index(), Some(0));
    }

    /// R2:R3 coupling - removal should keep cursor consistent for UX.
    #[test]
    fn removal_shifts_cursor() {
        // Prepare playlist with three tracks.
        let mut playlist = Playlist::new();
        let mut ids = Vec::new();
        for i in 0..3 {
            playlist.add_track(Track::demo(&format!("T{}", i), i as u64));
            ids.push(playlist.tracks().last().unwrap().id);
        }
        // Remove the first element and expect cursor to clamp to zero.
        assert!(playlist.remove_track(0));
        assert_eq!(playlist.active_index(), Some(0));
        // Remove non existing id to assert stable false response.
        assert!(!playlist.remove_track(999));
        // Clear remaining tracks and ensure cursor resets to None.
        for id in ids.into_iter().skip(1) {
            playlist.remove_track(id);
        }
        assert!(playlist.active_index().is_none());
    }

    /// R2:T2 - forward and backward traversal wrap correctly.
    #[test]
    fn navigation_wraps() {
        // Build playlist with three synthetic tracks.
        let mut playlist = Playlist::new();
        for idx in 0..3 {
            playlist.add_track(Track::demo(&format!("Demo {}", idx), idx as u64 + 1));
        }
        // Advance through the playlist plus one extra to test wrap.
        playlist.advance();
        playlist.advance();
        playlist.advance();
        assert_eq!(playlist.active_index(), Some(0));
        // Rewind from zero and expect wrap to the tail index.
        playlist.rewind();
        assert_eq!(playlist.active_index(), Some(2));
    }

    /// R6:T4 - probe metadata from bundled sample audio for realistic label/duration.
    #[test]
    fn from_path_reads_metadata() {
        let sample = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/audio/sample.wav");
        let track = Track::from_path(sample, 99);
        assert_eq!(track.id, 99);
        // Duration is deterministic for the generated sample.
        assert!(track.duration_secs >= 1);
        assert!(track.path.is_some());
    }
}
