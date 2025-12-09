/// Equalizer emulates Winamp's five band control surface.
#[derive(Clone, Debug)]
pub struct Equalizer {
    /// Gain per band in decibels.
    bands: [f32; 5],
}

impl Equalizer {
    /// Constructs a neutral equalizer with flat gains.
    pub fn new() -> Self {
        // Initialize each band at 0 dB to avoid coloration.
        Self { bands: [0.0; 5] }
    }

    /// Sets a band gain while clamping to +/-12 dB like Winamp.
    pub fn set_band(&mut self, index: usize, gain: f32) {
        // Ignore invalid indexes so UI interactions remain safe.
        if index >= self.bands.len() {
            return;
        }
        // Clamp incoming gain so tests can rely on deterministic bounds.
        self.bands[index] = gain.clamp(-12.0, 12.0);
    }

    /// Resets every band to neutral for quick A/B comparisons.
    pub fn reset(&mut self) {
        // Assign a new array literal for clarity.
        self.bands = [0.0; 5];
    }

    /// Returns the immutable band array for UI rendering.
    pub fn bands(&self) -> [f32; 5] {
        // Copy the array because it is tiny and stack friendly.
        self.bands
    }

    /// Calculates the average gain which drives the fake visualizer.
    pub fn average_gain(&self) -> f32 {
        // Sum the array then divide by its length for a quick heuristic.
        self.bands.iter().sum::<f32>() / self.bands.len() as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R4:T4 - ensures clamping logic honors the +/-12 dB limits.
    #[test]
    fn clamps_gain_values() {
        // Prepare equalizer and exceed both extremes.
        let mut eq = Equalizer::new();
        eq.set_band(0, 50.0);
        eq.set_band(1, -50.0);
        // Validate actual stored gains after clamping.
        assert_eq!(eq.bands()[0], 12.0);
        assert_eq!(eq.bands()[1], -12.0);
    }

    /// R4:T4 - reset should restore flat response quickly.
    #[test]
    fn resets_to_flat() {
        // Modify multiple bands.
        let mut eq = Equalizer::new();
        eq.set_band(0, 3.0);
        eq.set_band(1, -2.0);
        // Trigger reset and expect zeros across the board.
        eq.reset();
        assert_eq!(eq.bands(), [0.0; 5]);
    }
}
