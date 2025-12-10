#![deny(warnings)]

use aiwinamp::WinampApp;
use eframe::{egui, NativeOptions};

/// Entry point creating the egui native window and spawning the Winamp clone UI.
pub fn main() {
    // Configure window defaults to mimic the original Winamp footprint.
    let native_options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([480.0, 320.0])
            .with_min_inner_size([360.0, 280.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    // Start the egui runtime and bubble up any failures for user visibility.
    if let Err(err) = eframe::run_native(
        "AIWinAmp",
        native_options,
        Box::new(|cc| Box::new(WinampApp::new(cc))),
    ) {
        eprintln!("AIWinAmp failed to launch: {err}");
    }
}
