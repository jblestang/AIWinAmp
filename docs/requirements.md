# AIWinAmp Requirements and Traceability

| ID | Requirement | Priority | Verification | Notes |
| -- | ----------- | -------- | ------------ | ----- |
| R1 | The application shall render a Winamp-inspired main window with playback controls, progress bar, playlist panel, and mini visualizer within 500 ms of launch on a typical developer laptop. | Must | T1 (`tests/functional.rs::r1_boot_is_ready_for_render`) | Ensures UI scaffolding exists for users. |
| R2 | The software shall support adding tracks to a playlist, persisting their metadata, and highlighting the active track. | Must | T2 (`src/playlist.rs::tests::adds_tracks_and_auto_selects`) | Core playlist management requirement. |
| R3 | The software shall provide Play, Pause, Stop, Next, and Previous controls that mutate application state predictably even without an audio device. | Must | T3 (`src/core.rs::tests::play_starts_transport`, `navigation_respects_autoplay`) | Keeps UX deterministic in headless CI. |
| R4 | The player shall expose a master volume slider (0-100%) and a 16-band equalizer whose gains can be adjusted, clamped, and reset. | Should | T4 (`src/equalizer.rs::tests::clamps_gain_values`, `resets_to_flat`) | Equalizer is simulated but stateful for UI parity. |
| R5 | The software shall render a UV meter style visualization that reacts to simulated audio power at least 10 times per second. | Should | T1 (`tests/functional.rs::r5_visual_meter_changes_over_time`) | Visualization is derived from core telemetry. |
| R6 | The project shall bundle at least one sample audio track and provide a documented way to add custom assets. | Must | Documentation Review (README “Assets & Screenshots”) | Verified manually via README instructions. |
| R7 | Every functional requirement shall be covered by an automated test (unit or functional) with a trace ID recorded in this table. | Must | T1-T4 (see above) | Traceability enforcement. |

## Traceability References
- **T1** – Functional tests validating boot readiness and visual updates in `tests/functional.rs`.
- **T2** – Unit test suite covering playlist CRUD in `src/playlist.rs`.
- **T3** – Unit tests covering transport control state in `src/core.rs`.
- **T4** – Unit tests covering equalizer math in `src/equalizer.rs`.

This matrix is kept in sync with the code comments so each requirement is continuously validated by automated tests.
