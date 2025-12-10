use crate::audio::{AudioEngine, SilentEngine};
use crate::core::WinampCore;
use crate::equalizer::EQUALIZER_BANDS;
use crate::playlist::{Playlist, Track};
use anyhow::Result;
use eframe::egui;
use egui_plot::{Bar, BarChart, Plot};
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
}

impl WinampApp {
    /// Creates the egui app, loads demo tracks, and wires up rodio.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // Seed playlist with both file backed and synthetic demo tracks.
        let playlist = Self::build_initial_playlist();
        // Rodio engine gracefully degrades when running in headless CI.
        let audio: Box<dyn AudioEngine> = Box::new(SilentEngine::new());
        // Assemble the Winamp core and default status message.
        Self {
            core: WinampCore::new(playlist, audio),
            status: "Ready".to_string(),
            last_frame: Instant::now(),
        }
    }

    /// Builds the initial playlist including the bundled sample asset.
    fn build_initial_playlist() -> Playlist {
        // Start from an empty playlist for deterministic ordering.
        let mut playlist = Playlist::new();
        // Always include the bundled sine wave sample when available.
        if let Some(sample) = Self::sample_track_path() {
            playlist.add_track(Track::from_path(sample, 1));
        }
        // Add two synthetic demo tracks to flesh out the UI quickly.
        playlist.add_track(Track::demo("Neon Skyline", 2));
        playlist.add_track(Track::demo("LoFi Drip", 3));
        playlist
    }

    /// Resolves the bundled sample track relative to the manifest directory.
    fn sample_track_path() -> Option<PathBuf> {
        // Compose the absolute path once to support drag and drop later on.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/audio/sample.wav");
        if path.exists() {
            Some(path)
        } else {
            None
        }
    }

    /// Helper to mutate the status text whenever an action succeeds or fails.
    fn update_status(&mut self, action: &str, outcome: Result<()>) {
        // Store either success acknowledgement or the surfaced error.
        self.status = match outcome {
            Ok(_) => format!("{} ok", action),
            Err(err) => format!("{} failed: {}", action, err),
        };
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
            let progress = self.core.progress();
            ui.add(egui::ProgressBar::new(progress).show_percentage());
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
                    if ui.button("Add sample").clicked() {
                        if let Some(path) = Self::sample_track_path() {
                            let id = (self.core.playlist().len() + 100) as u64;
                            self.core
                                .playlist_mut()
                                .add_track(Track::from_path(path, id));
                            self.status = "sample cloned".to_string();
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
        // Central panel hosts the faux visualizer plus equalizer controls.
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Visualizer");
            Plot::new("spectrum")
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
                        .map(|(idx, level)| Bar::new(idx as f64, *level as f64).width(0.8))
                        .collect();
                    plot_ui.bar_chart(BarChart::new(bars));
                });
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Equalizer");
                if ui.button("Reset").clicked() {
                    self.core.equalizer_mut().reset();
                }
            });
            const ROW_SIZE: usize = EQUALIZER_BANDS / 2;
            let bands = self.core.equalizer().bands();
            for (chunk_idx, chunk) in bands.chunks(ROW_SIZE).enumerate() {
                ui.horizontal_wrapped(|ui| {
                    for (offset, gain_ref) in chunk.iter().enumerate() {
                        let band_idx = chunk_idx * ROW_SIZE + offset;
                        let mut gain = *gain_ref;
                        ui.vertical(|ui| {
                            ui.label(format!("B{}", band_idx + 1));
                            if ui
                                .add(egui::Slider::new(&mut gain, -12.0..=12.0).vertical())
                                .changed()
                            {
                                self.core.equalizer_mut().set_band(band_idx, gain);
                            }
                        });
                    }
                });
            }
        });
    }
}
