use rand::{rngs::StdRng, Rng, SeedableRng};

/// VisualMeter synthesizes a pseudo audio power feed for the UV meter.
#[derive(Debug)]
pub struct VisualMeter {
    /// Internal RNG so the waveform feels alive between frames.
    rng: StdRng,
    /// Last computed level to apply decay between frames.
    level: f32,
}

impl VisualMeter {
    /// Builds a new meter seeded for deterministic tests.
    pub fn new(seed: u64) -> Self {
        // Level starts silent to avoid jumps when the UI appears.
        Self {
            rng: StdRng::seed_from_u64(seed),
            level: 0.0,
        }
    }

    /// Steps the meter forward based on transport energy and EQ boost.
    pub fn sample(&mut self, transport_energy: f32, eq_gain: f32) -> f32 {
        // Generate a light noise floor for subtle flicker.
        let noise: f32 = self.rng.gen_range(0.0..0.2);
        // Combine transport progress, eq gain, and noise into a level.
        let target = (transport_energy + eq_gain + noise).clamp(0.0, 1.0);
        // Apply decay so the meter glides instead of snapping.
        self.level = self.level * 0.8 + target * 0.2;
        // Return the cached level for rendering.
        self.level
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R5:T1 - level should stay within 0..=1 even with aggressive gains.
    #[test]
    fn level_remains_bounded() {
        // Deterministic seed keeps expectations stable.
        let mut meter = VisualMeter::new(42);
        for _ in 0..50 {
            let value = meter.sample(2.0, 2.0);
            assert!(value >= 0.0 && value <= 1.0);
        }
    }
}
