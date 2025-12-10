use aiwinamp::audio::AudioEngine;
use aiwinamp::core::{TransportState, WinampCore};
use aiwinamp::playlist::{Playlist, Track};
use anyhow::Result;

/// SilentAudio is a deterministic stub used for functional testing.
struct SilentAudio;

impl AudioEngine for SilentAudio {
    fn play(&self, _track: &Track) -> Result<()> {
        Ok(())
    }

    fn stop(&self) {}

    fn set_volume(&self, _volume: f32) {}
}

/// T1 verifies that the application boots with a populated playlist and neutral transport.
#[test]
fn r1_boot_is_ready_for_render() {
    let mut playlist = Playlist::new();
    playlist.add_track(Track::demo("Boot Track", 123));
    let core = WinampCore::new(playlist, Box::new(SilentAudio));
    assert!(core.playlist().len() >= 1);
    assert_eq!(core.transport(), &TransportState::Stopped);
}

/// T1/T5 verify that ticking the core updates the faux visualizer level.
#[test]
fn r5_visual_meter_changes_over_time() {
    let mut playlist = Playlist::new();
    playlist.add_track(Track::demo("Meter", 321));
    let mut core = WinampCore::new(playlist, Box::new(SilentAudio));
    core.play().unwrap();
    let before = core.visual_spectrum().clone();
    core.tick(0.5).unwrap();
    let after = core.visual_spectrum().clone();
    for level in after {
        assert!(level >= 0.0 && level <= 1.0);
    }
    assert_ne!(before.to_vec(), after.to_vec());
}
