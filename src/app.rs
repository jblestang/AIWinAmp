use crate::audio::{AudioEngine, RodioEngine};
use crate::core::WinampCore;
use crate::playlist::{Playlist, Track};
use anyhow::Result;
use eframe::egui;
use egui_plot::{Bar, BarChart, Plot};
use rfd::FileDialog;
use std::path::PathBuf;
use std::time::Instant;

/// WinampApp glues the egui UI with the WinampCore state machine.
pub struct WinampApp {
    /// Core holds playlist, transport, equalizer, and audio plumbing.
    core: WinampCore,
    /// Human readable status banner for user feedback.
    status: String,
    /// Stores last frame instant so we can tick the core with delta time.
    last_frame: Instant,
    /// Next unique track id for imported audio.
    next_track_id: u64,
}

impl WinampApp {
    /// Creates the egui app, loads demo tracks, and wires up rodio.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // Seed playlist with both file backed and synthetic demo tracks.
        let (playlist, next_track_id) = Self::build_initial_playlist();
        // Rodio engine gracefully degrades when running in headless CI.
        let audio: Box<dyn AudioEngine> = RodioEngine::new_or_silent();
        // Assemble the Winamp core and default status message.
        Self {
            core: WinampCore::new(playlist, audio),
            status: "Ready".to_string(),
            last_frame: Instant::now(),
            next_track_id,
        }
    }

    /// Builds the initial playlist including all audio files from assets/audio directory.
    fn build_initial_playlist() -> (Playlist, u64) {
        // Start from an empty playlist for deterministic ordering.
        let mut playlist = Playlist::new();
        let mut next_id = 1;
        // Load all audio files from the assets/audio directory.
        let audio_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/audio");
        if let Ok(entries) = std::fs::read_dir(&audio_dir) {
            let mut audio_files: Vec<PathBuf> = entries
                .filter_map(|entry| {
                    entry.ok().and_then(|e| {
                        let path = e.path();
                        if path.is_file() {
                            // Check if it's an audio file by extension
                            let ext = path.extension()?.to_str()?.to_lowercase();
                            if matches!(ext.as_str(), "mp3" | "wav" | "flac" | "ogg" | "aac" | "m4a") {
                                Some(path)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                })
                .collect();
            // Sort for deterministic ordering
            audio_files.sort();
            // Add all audio files to the playlist
            for audio_file in audio_files {
                playlist.add_track(Track::from_path(audio_file, next_id));
                next_id += 1;
            }
        }
        (playlist, next_id)
    }

    /// Helper to mutate the status text whenever an action succeeds or fails.
    fn update_status(&mut self, action: &str, outcome: Result<()>) {
        // Store either success acknowledgement or the surfaced error.
        self.status = match outcome {
            Ok(_) => format!("{} ok", action),
            Err(err) => format!("{} failed: {}", action, err),
        };
    }

    /// Generates a new unique track identifier.
    fn allocate_track_id(&mut self) -> u64 {
        let id = self.next_track_id;
        self.next_track_id += 1;
        id
    }

    /// Adds a real audio file to the playlist and updates UI status.
    fn import_audio_track(&mut self, path: PathBuf) {
        let id = self.allocate_track_id();
        let track = Track::from_path(&path, id);
        let label = track.title.clone();
        self.core.playlist_mut().add_track(track);
        self.status = format!("Imported {}", label);
    }
}

impl eframe::App for WinampApp {
    /// Called once per frame; responsible for ticking core and rendering UI.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Compute frame delta in seconds for transport and visuals.
        let now = Instant::now();
        let delta = now.saturating_duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;
        // Tick the core and log any transient issues.
        if let Err(err) = self.core.tick(delta) {
            self.status = format!("tick failed: {}", err);
        }
        
        // Request continuous repaint when audio is playing (for smooth spectrum and progress updates)
        // Otherwise, egui will only repaint on user input (mouse/keyboard)
        let is_playing = matches!(self.core.transport(), crate::core::TransportState::Playing { .. });
        if is_playing {
            // Request repaint at ~30 FPS for smooth visuals (every ~33ms)
            ctx.request_repaint_after(std::time::Duration::from_millis(1000/25));
        }
        // Render top transport panel similar to Winamp's layout.
        egui::TopBottomPanel::top("transport").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("▶").clicked() {
                    let outcome = self.core.play();
                    self.update_status("play", outcome);
                }
                if ui.button("⏸").clicked() {
                    self.core.pause();
                    self.status = "paused".to_string();
                }
                if ui.button("⏹").clicked() {
                    self.core.stop();
                    self.status = "stopped".to_string();
                }
                if ui.button("⏮").clicked() {
                    let result = self.core.previous(true);
                    self.update_status("prev", result);
                }
                if ui.button("⏭").clicked() {
                    let result = self.core.next(true);
                    self.update_status("next", result);
                }
                let mut volume = self.core.volume();
                ui.label("Vol");
                if ui
                    .add(egui::Slider::new(&mut volume, 0.0..=1.0).clamp_to_range(true))
                    .changed()
                {
                    self.core.set_volume(volume);
                }
            });
            // Get progress from core (based on actual elapsed playback time, not mouse position)
            // This value is read-only and calculated from frame delta time
            let progress_value = self.core.progress();
            
            // Create a read-only progress bar that displays the actual playback position
            // The value comes from elapsed time tracking in core.tick(), not from mouse
            ui.add(
                egui::ProgressBar::new(progress_value)
                    .show_percentage()
                    .fill(egui::Color32::from_rgb(100, 150, 255))
            );
            
            // Add a label showing time to make it clear it's time-based
            if let Some(track) = self.core.playlist().active() {
                let current_secs = progress_value * track.duration_secs as f32;
                let total_secs = track.duration_secs;
                ui.label(format!("{:.0}:{:02.0} / {:.0}:{:02.0}", 
                    current_secs / 60.0, current_secs % 60.0,
                    total_secs as f32 / 60.0, total_secs as f32 % 60.0));
            }
            ui.label(&self.status);
        });
        // Playlist panel sits on the left to mimic Winamp's docking paradigm.
        egui::SidePanel::left("playlist")
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Add demo").clicked() {
                        let seed = (self.core.playlist().len() + 10) as u64;
                        self.core
                            .playlist_mut()
                            .add_track(Track::demo(&format!("New Track {}", seed), seed));
                        self.status = "demo track added".to_string();
                    }
                    if ui.button("Import audio").clicked() {
                        if let Some(path) = FileDialog::new()
                            .add_filter("Audio", &["mp3", "flac", "ogg", "wav", "aac", "m4a"])
                            .pick_file()
                        {
                            self.import_audio_track(path);
                        }
                    }
                });
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let active_index = self.core.playlist().active_index();
                    let track_rows: Vec<(usize, String, bool)> = self
                        .core
                        .playlist()
                        .tracks()
                        .iter()
                        .enumerate()
                        .map(|(idx, track)| {
                            let label =
                                format!("{:02}. {} - {}", idx + 1, track.title, track.artist);
                            let selected = Some(idx) == active_index;
                            (idx, label, selected)
                        })
                        .collect();
                    for (idx, label, selected) in track_rows {
                        if ui.selectable_label(selected, label).clicked() {
                            self.core.playlist_mut().set_active(idx);
                        }
                    }
                });
            });
        // Central panel hosts the visualizer plus equalizer controls.
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.heading("Spectrum Visualizer");
                // Increased height for more visual space
                Plot::new("spectrum")
                    .height(350.0)
                    .allow_drag(false)
                    .allow_zoom(false)
                    .include_y(0.0)
                    .include_y(1.0)
                    .show(ui, |plot_ui| {
                        let bars: Vec<_> = self
                            .core
                            .visual_spectrum()
                            .iter()
                            .enumerate()
                            .map(|(idx, level)| {
                                // Make bars more visible with better width
                                // Color gradient from blue (low) to red (high)
                                let r = (level * 255.0) as u8;
                                let g = 0u8;
                                let b = ((1.0 - level) * 255.0) as u8;
                                Bar::new(idx as f64, *level as f64)
                                    .width(0.9)
                                    .fill(egui::Color32::from_rgb(r, g, b))
                            })
                            .collect();
                        plot_ui.bar_chart(BarChart::new(bars));
                    });
                ui.separator();
                ui.heading("Equalizer");
                ui.horizontal(|ui| {
                    if ui.button("Reset").clicked() {
                        self.core.equalizer_mut().reset();
                        self.core.sync_eq_to_audio();
                    }
                });
                // Display all equalizer bands on a single horizontal line
                ui.horizontal_wrapped(|ui| {
                    let bands = self.core.equalizer().bands();
                    for (band_idx, gain_ref) in bands.iter().enumerate() {
                        let mut gain = *gain_ref;
                        ui.vertical(|ui| {
                            ui.label(format!("B{}", band_idx + 1));
                                        if ui
                                            .add(egui::Slider::new(&mut gain, -12.0..=12.0).vertical().show_value(false))
                                            .changed()
                                        {
                                            self.core.equalizer_mut().set_band(band_idx, gain);
                                            self.core.sync_eq_to_audio();
                                        }
                            // Show current value as text
                            ui.label(format!("{:.1}", gain));
                        });
                    }
                });
            });
        });
    }
}
