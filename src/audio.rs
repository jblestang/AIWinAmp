use crate::playlist::Track;
use anyhow::{Context, Result};
use parking_lot::Mutex;
use rodio::{Decoder, OutputStream, Sink, Source};
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use std::time::Duration;

/// AudioEngine abstraction allows the UI to remain agnostic of playback backend.
pub trait AudioEngine: Send + Sync {
    /// Requests playback of a specific track asset.
    fn play(&self, track: &Track) -> Result<()>;
    /// Stops any playing audio immediately.
    fn stop(&self);
    /// Applies master volume changes (should work dynamically during playback).
    fn set_volume(&self, volume: f32);
    /// Updates equalizer band gains (should work dynamically during playback).
    fn set_eq_bands(&self, bands: [f32; 16]);
    /// Returns the current audio spectrum for visualization.
    fn get_spectrum(&self) -> Option<Vec<f32>>;
}

/// SilentEngine simulates playback for deterministic CI and headless environments.
#[derive(Default)]
pub struct SilentEngine {
    /// Stores the last track title for telemetry and testing.
    last_track: Mutex<Option<String>>,
    /// Caches the most recent volume value for inspection.
    volume: Mutex<f32>,
}

impl SilentEngine {
    /// Creates a new silent engine with zeroed telemetry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the last requested track, primarily for assertions.
    pub fn last_track(&self) -> Option<String> {
        self.last_track.lock().clone()
    }
}

impl AudioEngine for SilentEngine {
    fn play(&self, track: &Track) -> Result<()> {
        *self.last_track.lock() = Some(track.title.clone());
        Ok(())
    }

    fn stop(&self) {
        *self.last_track.lock() = None;
    }

    fn set_volume(&self, volume: f32) {
        *self.volume.lock() = volume.clamp(0.0, 1.0);
    }

    fn set_eq_bands(&self, _bands: [f32; 16]) {
        // Silent engine doesn't apply EQ
    }

    fn get_spectrum(&self) -> Option<Vec<f32>> {
        None
    }
}

/// RodioEngine provides real audio playback using the rodio library.
pub struct RodioEngine {
    /// Output stream handle that must be kept alive.
    _stream: OutputStream,
    /// Stream handle for creating sinks.
    stream_handle: rodio::OutputStreamHandle,
    /// Sink for controlling playback.
    sink: Arc<Mutex<Option<Sink>>>,
    /// Current volume level (can be changed dynamically).
    volume: Arc<Mutex<f32>>,
    /// Equalizer band gains (can be changed dynamically).
    eq_bands: Arc<Mutex<[f32; 16]>>,
    /// Spectrum analyzer for real-time audio visualization.
    spectrum_analyzer: Arc<Mutex<SpectrumAnalyzer>>,
}

/// Spectrum analyzer that processes audio samples for visualization.
struct SpectrumAnalyzer {
    /// Buffer of recent audio samples (mono, downmixed from stereo).
    sample_buffer: Vec<f32>,
    /// Current spectrum levels.
    spectrum: Vec<f32>,
    /// Sample rate in Hz.
    sample_rate: u32,
    /// Sample counter for frequency analysis.
    sample_count: usize,
}

impl SpectrumAnalyzer {
    fn new(sample_rate: u32) -> Self {
        Self {
            sample_buffer: Vec::new(),
            spectrum: vec![0.0; 16], // 16 bands for visualization
            sample_rate,
            sample_count: 0,
        }
    }

    fn add_sample(&mut self, sample: f32) {
        self.sample_buffer.push(sample.abs());
        self.sample_count += 1;
        
        // Keep buffer size reasonable (about 0.05 seconds of audio for faster response)
        let max_buffer_size = (self.sample_rate as usize / 20).max(1024);
        if self.sample_buffer.len() > max_buffer_size {
            self.sample_buffer.remove(0);
        }

        // Update spectrum more frequently for better responsiveness (every 64 samples for faster updates)
        if self.sample_count % 64 == 0 && self.sample_buffer.len() >= 128 {
            let buffer_len = self.sample_buffer.len();
            let num_bands = 16;
            
            // Use a simpler but more responsive approach: divide buffer into bands
            let samples_per_band = buffer_len / num_bands;
            
            for band in 0..num_bands {
                let start = band * samples_per_band;
                let end = ((band + 1) * samples_per_band).min(buffer_len);
                
                if start < end {
                    // Calculate RMS energy for this band
                    let chunk = &self.sample_buffer[start..end];
                    let rms = (chunk.iter().map(|&x| x * x).sum::<f32>() / chunk.len() as f32).sqrt();
                    
                    // Increased scaling for more visible movement (30x instead of 20x)
                    let scaled = (rms * 30.0).min(1.0);
                    // Very aggressive response - 20% old, 80% new for very dynamic movement
                    self.spectrum[band] = self.spectrum[band] * 0.2 + scaled * 0.8;
                } else {
                    // Faster decay when no samples for more visible drops
                    self.spectrum[band] *= 0.75;
                }
            }
        } else if self.sample_count % 32 == 0 {
            // Apply faster decay even when not updating full spectrum
            for band in &mut self.spectrum {
                *band *= 0.9; // Faster decay for more movement
            }
        }
    }

    fn get_spectrum(&self) -> Vec<f32> {
        self.spectrum.clone()
    }
}

/// Custom source that applies dynamic volume control.
struct DynamicVolumeSource<S> {
    source: S,
    volume: Arc<Mutex<f32>>,
}

impl<S> DynamicVolumeSource<S>
where
    S: Source<Item = f32> + Send,
{
    fn new(source: S, volume: Arc<Mutex<f32>>) -> Self {
        Self { source, volume }
    }
}

impl<S> Source for DynamicVolumeSource<S>
where
    S: Source<Item = f32> + Send,
{
    fn current_frame_len(&self) -> Option<usize> {
        self.source.current_frame_len()
    }

    fn channels(&self) -> u16 {
        self.source.channels()
    }

    fn sample_rate(&self) -> u32 {
        self.source.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.source.total_duration()
    }
}

impl<S> Iterator for DynamicVolumeSource<S>
where
    S: Source<Item = f32> + Send,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.source.next()?;
        let volume = *self.volume.lock();
        Some(sample * volume)
    }
}

/// Custom source that applies equalizer gains with smooth frequency analysis.
struct EQSource<S> {
    source: S,
    eq_bands: Arc<Mutex<[f32; 16]>>,
    sample_rate: u32,
    sample_buffer: Vec<f32>,
    buffer_size: usize,
    sample_counter: usize,
    channel_count: u16,
    channel_counter: u16,
    last_gain: f32,
}

impl<S> EQSource<S>
where
    S: Source<Item = f32> + Send,
{
    fn new(source: S, eq_bands: Arc<Mutex<[f32; 16]>>, sample_rate: u32) -> Self {
        let channel_count = source.channels();
        // Use a buffer for frequency analysis (about 10ms of audio)
        let buffer_size = (sample_rate as usize / 100).max(256);
        Self {
            source,
            eq_bands,
            sample_rate,
            sample_buffer: Vec::with_capacity(buffer_size),
            buffer_size,
            sample_counter: 0,
            channel_count,
            channel_counter: 0,
            last_gain: 1.0,
        }
    }

    /// Gets the EQ gain for the current sample with proper frequency analysis.
    fn get_eq_gain(&mut self) -> f32 {
        let bands = *self.eq_bands.lock();
        
        // Check if any EQ is applied
        let max_gain = bands.iter().map(|&g| g.abs()).fold(0.0f32, f32::max);
        if max_gain < 0.01 {
            // No EQ applied, return neutral
            self.last_gain = 1.0;
            return 1.0;
        }
        
        // Need enough samples for frequency analysis
        if self.sample_buffer.len() < 64 {
            return self.last_gain;
        }
        
        // Use a simple but effective frequency analysis: divide buffer into 16 bands
        // Each band represents a frequency range (logarithmic distribution)
        let buffer_len = self.sample_buffer.len();
        let num_bands = 16;
        let mut band_energies = [0.0f32; 16];
        
        // Calculate energy for each frequency band
        for (i, &sample) in self.sample_buffer.iter().enumerate() {
            // Map sample position to frequency band (logarithmic)
            // Earlier samples = lower frequencies, later samples = higher frequencies
            let position = i as f32 / buffer_len as f32;
            // Use logarithmic mapping for better frequency distribution
            let log_pos = (position * 15.0 + 0.5).ln() / 15.0_f32.ln();
            let band_idx = ((log_pos * num_bands as f32).clamp(0.0, (num_bands - 1) as f32)) as usize;
            band_energies[band_idx] += sample * sample;
        }
        
        // Normalize energies and calculate weighted gain
        let total_energy: f32 = band_energies.iter().sum();
        if total_energy < 0.0001 {
            return self.last_gain;
        }
        
        // Calculate weighted average of EQ gains based on frequency content
        let mut weighted_gain_db = 0.0;
        let mut weight_sum = 0.0;
        
        for (band_idx, &energy) in band_energies.iter().enumerate() {
            let weight = energy / total_energy;
            weighted_gain_db += bands[band_idx] * weight;
            weight_sum += weight;
        }
        
        let gain_db = if weight_sum > 0.0 { weighted_gain_db / weight_sum } else { 0.0 };
        
        // Convert dB to linear gain (full impact, no reduction)
        let gain_linear = 10.0_f32.powf(gain_db / 20.0);
        
        // Apply moderate smoothing for smooth transitions (not too aggressive)
        let smoothing = 0.9; // 90% old, 10% new - responsive but smooth
        self.last_gain = self.last_gain * smoothing + gain_linear * (1.0 - smoothing);
        
        // Allow full range of EQ effect (±12dB = 0.25 to 4.0 linear)
        self.last_gain = self.last_gain.clamp(0.25, 4.0);
        
        self.last_gain
    }
}

impl<S> Source for EQSource<S>
where
    S: Source<Item = f32> + Send,
{
    fn current_frame_len(&self) -> Option<usize> {
        self.source.current_frame_len()
    }

    fn channels(&self) -> u16 {
        self.source.channels()
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        self.source.total_duration()
    }
}

impl<S> Iterator for EQSource<S>
where
    S: Source<Item = f32> + Send,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.source.next()?;
        
        // Add sample to buffer for frequency analysis (mono, use one channel)
        if self.channel_counter == 0 {
            self.sample_buffer.push(sample);
            if self.sample_buffer.len() > self.buffer_size {
                self.sample_buffer.remove(0);
            }
        }
        
        // Get EQ gain with smooth interpolation
        let eq_gain = self.get_eq_gain();
        
        // Update channel and sample counters
        self.channel_counter = (self.channel_counter + 1) % self.channel_count;
        if self.channel_counter == 0 {
            self.sample_counter += 1;
        }
        
        Some(sample * eq_gain)
    }
}

/// Custom source that captures samples for spectrum analysis.
struct CapturingSource<S> {
    source: S,
    analyzer: Arc<Mutex<SpectrumAnalyzer>>,
    channel_count: u16,
    sample_counter: u16,
}

impl<S> CapturingSource<S>
where
    S: Source<Item = f32> + Send,
{
    fn new(source: S, analyzer: Arc<Mutex<SpectrumAnalyzer>>) -> Self {
        let channel_count = source.channels();
        Self {
            source,
            analyzer,
            channel_count,
            sample_counter: 0,
        }
    }
}

impl<S> Source for CapturingSource<S>
where
    S: Source<Item = f32> + Send,
{
    fn current_frame_len(&self) -> Option<usize> {
        self.source.current_frame_len()
    }

    fn channels(&self) -> u16 {
        self.source.channels()
    }

    fn sample_rate(&self) -> u32 {
        self.source.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.source.total_duration()
    }
}

impl<S> Iterator for CapturingSource<S>
where
    S: Source<Item = f32> + Send,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.source.next()?;
        
        // For stereo sources, only analyze one channel to avoid double processing
        // For mono, analyze every sample
        if self.channel_count == 1 || self.sample_counter == 0 {
            self.analyzer.lock().add_sample(sample.abs());
        }
        
        self.sample_counter = (self.sample_counter + 1) % self.channel_count;
        
        Some(sample)
    }
}

impl RodioEngine {
    /// Creates a new RodioEngine, falling back to SilentEngine on error.
    pub fn new() -> Result<Self> {
        let (_stream, stream_handle) = OutputStream::try_default()
            .context("Failed to create audio output stream")?;
        
        let sink = Sink::try_new(&stream_handle)
            .context("Failed to create audio sink")?;
        
        // Default sample rate (will be updated when playing a file)
        let sample_rate = 44100;
        
        Ok(Self {
            _stream,
            stream_handle,
            sink: Arc::new(Mutex::new(Some(sink))),
            volume: Arc::new(Mutex::new(1.0)),
            eq_bands: Arc::new(Mutex::new([0.0; 16])),
            spectrum_analyzer: Arc::new(Mutex::new(SpectrumAnalyzer::new(sample_rate))),
        })
    }

    /// Attempts to create a RodioEngine, falling back to SilentEngine on failure.
    pub fn new_or_silent() -> Box<dyn AudioEngine> {
        match Self::new() {
            Ok(engine) => Box::new(engine),
            Err(_) => Box::new(SilentEngine::new()),
        }
    }
}

// SAFETY: RodioEngine is only used from the main egui thread, so it's safe to mark as Send+Sync
unsafe impl Send for RodioEngine {}
unsafe impl Sync for RodioEngine {}

impl AudioEngine for RodioEngine {
    fn play(&self, track: &Track) -> Result<()> {
        // Stop any currently playing audio
        self.stop();
        
        // Get the file path
        let path = track.path.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Track has no file path"))?;
        
        // Open and decode the audio file
        let file = File::open(path)
            .with_context(|| format!("Failed to open audio file: {}", path.display()))?;
        let decoder = Decoder::new(BufReader::new(file))
            .with_context(|| format!("Failed to decode audio file: {}", path.display()))?;
        
        // Get sample rate for analyzer
        let sample_rate = decoder.sample_rate();
        *self.spectrum_analyzer.lock() = SpectrumAnalyzer::new(sample_rate);
        
        // Convert decoder to f32 samples
        let decoder_f32 = decoder.convert_samples();
        
        // Apply EQ first, then volume, then capture for spectrum
        let eq_source = EQSource::new(decoder_f32, Arc::clone(&self.eq_bands), sample_rate);
        let volume_source = DynamicVolumeSource::new(eq_source, Arc::clone(&self.volume));
        let capturing_source = CapturingSource::new(volume_source, Arc::clone(&self.spectrum_analyzer));
        
        let source = capturing_source;
        
        // Create a new sink and play
        let mut sink_guard = self.sink.lock();
        // Recreate sink if needed
        if sink_guard.is_none() {
            *sink_guard = Some(Sink::try_new(&self.stream_handle)
                .context("Failed to recreate audio sink")?);
        }
        if let Some(ref sink) = *sink_guard {
            sink.append(source);
            sink.play();
        }
        
        Ok(())
    }

    fn stop(&self) {
        let sink_guard = self.sink.lock();
        if let Some(ref sink) = *sink_guard {
            sink.stop();
        }
    }

    fn set_volume(&self, volume: f32) {
        *self.volume.lock() = volume.clamp(0.0, 1.0);
        // Volume is now applied dynamically via DynamicVolumeSource
    }

    fn set_eq_bands(&self, bands: [f32; 16]) {
        *self.eq_bands.lock() = bands;
        // EQ is now applied dynamically via EQSource
    }

    fn get_spectrum(&self) -> Option<Vec<f32>> {
        Some(self.spectrum_analyzer.lock().get_spectrum())
    }
}
