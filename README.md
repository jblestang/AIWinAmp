# AIWinAmp

AIWinAmp is a lightweight Winamp-inspired desktop music player written in Rust + egui. It focuses on a deterministic simulation of Winamp's signature transport, playlist, equalizer, and UV meter so it runs headless in CI while remaining faithful to the feel of the original.

![AIWinAmp Screenshot](assets/screenshots/v4_audio.png)

## Features
- Winamp-style transport bar with Play/Pause/Stop/Prev/Next controls and progress bar.
- Playlist management with demo tracks, row selection, and easy demo/sample injection.
- Simulated 16-band equalizer with clamped sliders plus instant reset.
- Audio import workflow that probes MP3/FLAC/OGG/WAV/AAC metadata before queueing tracks.
- Procedural UV-style spectrum visualizer reacting to transport progress and EQ boost.
- Silent audio backend that can be swapped for a native engine without changing UI logic.
- Comprehensive unit + functional tests mapped to requirements in `docs/requirements.md`.

## Requirements and Traceability
Requirement IDs (R1-R7) and their verifying tests (T1-T4) live in `docs/requirements.md`. Each test cites the matching requirement in its doc comment so you can trace coverage quickly.

## Toolchain
Some transitive crates (e.g., new Wayland stack) opt into the draft 2024 edition, so building requires the latest nightly Rust.

```bash
rustup toolchain install nightly
cargo +nightly test
cargo +nightly run
```

## Running the App
```bash
cargo +nightly run
```
The binary launches a 480x320 egui window that mirrors the Winamp transport and playlist layout. Buttons mutate the in-memory core, and the visualizer pulses with synthesized energy.

## Testing
```bash
cargo +nightly test
```
- Unit tests cover playlist navigation (R2/T2), transport transitions (R3/T3), equalizer math (R4/T4), and visual meter bounds (R5/T1).
- Functional tests (`tests/functional.rs`) ensure the core boots with a populated playlist and that the visualizer reacts to ticks (R1, R5).

## Assets & Screenshots
- `assets/audio/sample.wav` – bundled sine-wave sample for demo playlist entries.
- `assets/screenshots/v4_audio.png` – latest capture showing spectrum analyzer, equalizer, and import controls (generated headlessly via Xvfb).
- `assets/screenshots/v3_spectrum.png`, `v2_real.png`, `v1_mock.png` – earlier snapshots kept for historical reference.

### Capturing a Fresh Screenshot
The repository ships with `scripts/capture_screenshot.sh`, which runs the app under `xvfb-run` and uses ImageMagick’s `import` to save a PNG for documentation.

```bash
sudo apt-get install -y libxkbcommon-x11-0 imagemagick xdotool  # once per machine
./scripts/capture_screenshot.sh assets/screenshots/v4_audio.png
```

## Importing Audio
- Use the **Import audio** button in the playlist panel to add MP3, FLAC, OGG/Vorbis, WAV, or AAC/M4A files via a native file picker.
- Symphonia probes metadata (title, artist, duration) so imported tracks stay well-labeled even without embedded tags.
- Imported files are stored with persistent IDs, so repeat selections won’t clobber playlist ordering.

## Comment Density
Per the project requirement, the code follows roughly a 1:2 comment-to-code line ratio. Doc comments highlight how each struct/function satisfies reflected requirements.
