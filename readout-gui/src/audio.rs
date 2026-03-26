/// Alarm audio player using rodio.
/// Optional — if audio init fails, continues silently.
pub struct AlarmAudio {
    #[cfg(feature = "audio")]
    sink: Option<rodio::MixerDeviceSink>,
}

impl AlarmAudio {
    pub fn new() -> Self {
        #[cfg(feature = "audio")]
        {
            match rodio::DeviceSinkBuilder::open_default_sink() {
                Ok(sink) => Self { sink: Some(sink) },
                Err(e) => {
                    tracing::warn!("Audio init failed (continuing silently): {e}");
                    Self { sink: None }
                }
            }
        }
        #[cfg(not(feature = "audio"))]
        {
            Self {}
        }
    }

    /// Play a simple beep tone for alarm notification.
    #[allow(unused_variables)]
    pub fn beep(&self, volume: f32) {
        #[cfg(feature = "audio")]
        {
            use rodio::source::Source;
            if let Some(ref sink) = self.sink {
                let source = rodio::source::SineWave::new(880.0)
                    .take_duration(std::time::Duration::from_millis(200))
                    .amplify(volume);
                sink.mixer().add(source);
            }
        }
    }
}

impl Default for AlarmAudio {
    fn default() -> Self {
        Self::new()
    }
}
