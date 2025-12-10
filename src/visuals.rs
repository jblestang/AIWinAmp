use rand::{rngs::StdRng, Rng, SeedableRng};

/// Number of bars to paint in the faux spectrum analyzer.
pub const VISUAL_BANDS: usize = 16;

/// VisualMeter synthesizes a pseudo spectrum for the UV analyzer.
#[derive(Debug)]
pub struct VisualMeter {
    /// Internal RNG so the waveform feels alive between frames.
    rng: StdRng,
    /// Cached band levels that we ease toward every frame.
    bands: [f32; VISUAL_BANDS],
}

impl VisualMeter {
    /// Builds a new meter seeded for deterministic tests.
    pub fn new(seed: u64) -> Self {
        // All bands start silent to avoid jumps when the UI appears.
        Self {
            rng: StdRng::seed_from_u64(seed),
            bands: [0.0; VISUAL_BANDS],
        }
    }

    /// Steps the meter forward and returns the latest spectrum snapshot.
    pub fn sample(&mut self, transport_energy: f32, eq_gain: f32) -> [f32; VISUAL_BANDS] {
        // Iterate through each band and synthesize a new target level.
        for (idx, level) in self.bands.iter_mut().enumerate() {
            // Low bands lean on transport energy while highs react to EQ gain.
            let band_ratio = idx as f32 / VISUAL_BANDS as f32;
            let energy_component = transport_energy * (1.0 - band_ratio * 0.5);
            let eq_component = eq_gain.abs() * (0.3 + band_ratio * 0.7);
            let noise: f32 = self.rng.gen_range(0.0..0.25);
            let target = (energy_component + eq_component + noise).clamp(0.0, 1.0);
            // Apply decay so the band glides instead of snapping.
            *level = *level * 0.65 + target * 0.35;
        }
        self.bands
    }

    /// Returns the current band levels without mutating state.
    pub fn bands(&self) -> &[f32; VISUAL_BANDS] {
        &self.bands
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R5:T1 - every band should stay within 0..=1 even with aggressive gains.
    #[test]
    fn spectrum_remains_bounded() {
        let mut meter = VisualMeter::new(42);
        for _ in 0..50 {
            let bands = meter.sample(2.0, 2.0);
            for value in bands {
                assert!(value >= 0.0 && value <= 1.0);
            }
        }
    }
}
