use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Alarm audio player using rodio.
/// Plays a continuous tone while `active` flag is set.
/// Optional — if audio init fails, continues silently.
pub struct AlarmAudio {
    active: Arc<AtomicBool>,
    #[allow(dead_code)]
    volume_pct: Arc<AtomicU32>,
    #[cfg(feature = "audio")]
    #[allow(dead_code)]
    sink: Option<rodio::MixerDeviceSink>,
}

impl AlarmAudio {
    pub fn new() -> Self {
        let active = Arc::new(AtomicBool::new(false));
        let volume_pct = Arc::new(AtomicU32::new(50)); // 0-100

        #[cfg(feature = "audio")]
        {
            match rodio::DeviceSinkBuilder::open_default_sink() {
                Ok(sink) => {
                    // Add a single infinite gated sine source to the mixer.
                    // It produces sound only when `active` is true.
                    let source = GatedSine::new(660.0, active.clone(), volume_pct.clone());
                    sink.mixer().add(source);

                    Self {
                        active,
                        volume_pct,
                        sink: Some(sink),
                    }
                }
                Err(e) => {
                    tracing::warn!("Audio init failed (continuing silently): {e}");
                    Self {
                        active,
                        volume_pct,
                        sink: None,
                    }
                }
            }
        }
        #[cfg(not(feature = "audio"))]
        {
            Self { active, volume_pct }
        }
    }

    /// Start or stop the continuous alarm tone.
    pub fn set_active(&self, on: bool) {
        self.active.store(on, Ordering::Relaxed);
    }

    /// Set volume (0.0 to 1.0).
    pub fn set_volume(&self, volume: f32) {
        let pct = (volume.clamp(0.0, 1.0) * 100.0) as u32;
        self.volume_pct.store(pct, Ordering::Relaxed);
    }

    #[allow(dead_code)]
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }
}

impl Default for AlarmAudio {
    fn default() -> Self {
        Self::new()
    }
}

// --- Gated sine wave source ---
// Outputs sine samples when `gate` is true, silence when false.
// Runs forever — dropped only when the mixer sink is dropped.

#[cfg(feature = "audio")]
struct GatedSine {
    freq: f32,
    sample_rate: u32,
    phase: f32,
    gate: Arc<AtomicBool>,
    volume_pct: Arc<AtomicU32>,
}

#[cfg(feature = "audio")]
impl GatedSine {
    fn new(freq: f32, gate: Arc<AtomicBool>, volume_pct: Arc<AtomicU32>) -> Self {
        Self {
            freq,
            sample_rate: 44100,
            phase: 0.0,
            gate,
            volume_pct,
        }
    }
}

#[cfg(feature = "audio")]
impl Iterator for GatedSine {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let sample = if self.gate.load(Ordering::Relaxed) {
            let vol = self.volume_pct.load(Ordering::Relaxed) as f32 / 100.0;
            let value = (self.phase * 2.0 * std::f32::consts::PI).sin() * vol * 0.5;
            self.phase += self.freq / self.sample_rate as f32;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
            value
        } else {
            // Keep phase reset so tone starts cleanly
            self.phase = 0.0;
            0.0
        };
        Some(sample)
    }
}

#[cfg(feature = "audio")]
impl rodio::Source for GatedSine {
    fn current_span_len(&self) -> Option<usize> {
        None // infinite
    }

    fn channels(&self) -> rodio::ChannelCount {
        1.try_into().expect("1 channel")
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        self.sample_rate.try_into().expect("44100 sample rate")
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None // infinite
    }
}
