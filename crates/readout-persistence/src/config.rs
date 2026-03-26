use serde::{Deserialize, Serialize};

// --- Enums ---

fn deserialize_case_insensitive<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + for<'a> TryFrom<&'a str>,
{
    let s = String::deserialize(deserializer)?;
    match T::try_from(s.as_str()) {
        Ok(v) => Ok(v),
        Err(_) => {
            tracing::warn!(
                value = %s,
                r#type = std::any::type_name::<T>(),
                "unknown config enum value, using default"
            );
            Ok(T::default())
        }
    }
}

macro_rules! case_insensitive_enum {
    ($name:ident { $($variant:ident => $str:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
        pub enum $name {
            $($variant),+
        }

        impl Default for $name {
            fn default() -> Self {
                // First variant is the default
                case_insensitive_enum!(@first $($variant),+)
            }
        }

        impl TryFrom<&str> for $name {
            type Error = ();
            fn try_from(s: &str) -> Result<Self, ()> {
                let lower = s.to_lowercase();
                $(
                    if lower == $str.to_lowercase() || lower == stringify!($variant).to_lowercase() {
                        return Ok(Self::$variant);
                    }
                )+
                Err(())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                deserialize_case_insensitive(deserializer)
            }
        }
    };
    (@first $first:ident $(, $rest:ident)*) => { Self::$first };
}

case_insensitive_enum!(ObsOutputMode {
    ValueOnly => "value_only",
    ValueAndUnit => "value_and_unit",
    CustomTemplate => "custom_template",
});

case_insensitive_enum!(DashboardDeviceVisibility {
    Both => "both",
    Multimeter => "multimeter",
    UsbC => "usbc",
});

case_insensitive_enum!(DashboardTheme {
    System => "system",
    Light => "light",
    Dark => "dark",
});

case_insensitive_enum!(PopoutDisplayMode {
    Mini => "mini",
    Compact => "compact",
    Detailed => "detailed",
});

case_insensitive_enum!(MacAlertSoundPreset {
    System => "system",
    Glass => "glass",
    Sosumi => "sosumi",
    Funk => "funk",
});

// --- PopoutWindowFrame ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PopoutWindowFrame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

// --- PopoutLayoutProfile ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PopoutLayoutProfile {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub multimeter_mode: PopoutDisplayMode,
    #[serde(default)]
    pub usbc_mode: PopoutDisplayMode,
    pub multimeter_frame: Option<PopoutWindowFrame>,
    pub usbc_frame: Option<PopoutWindowFrame>,
}

// --- AppConfiguration ---

/// Full application configuration, ported from Swift AppConfiguration.
/// Uses `#[serde(default)]` so missing JSON keys fall back to defaults.
/// Values are clamped on deserialization via custom Deserialize impl.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AppConfiguration {
    // Device ports
    #[serde(default)]
    pub multimeter_port: String,
    #[serde(default)]
    pub usbc_port: String,

    // Device enable/disable
    #[serde(default = "default_true")]
    pub multimeter_enabled: bool,
    #[serde(default)]
    pub usbc_enabled: bool,
    #[serde(default = "default_true")]
    pub multimeter_auto_reconnect: bool,
    #[serde(default = "default_true")]
    pub usbc_auto_reconnect: bool,
    #[serde(default)]
    pub use_simulator: bool,

    // Sampling
    #[serde(default = "default_sample_rate")]
    pub sample_rate_hz: u32,
    #[serde(default = "default_graph_history")]
    pub graph_history_seconds: u32,

    // Output queue
    #[serde(default = "default_queue_capacity")]
    pub output_queue_capacity: u32,
    #[serde(default = "default_queue_retries")]
    pub output_queue_max_retry_attempts: u32,

    // Short circuit detection
    #[serde(default = "default_short_threshold")]
    pub short_threshold: f64,
    #[serde(default)]
    pub beep_on_short_meter: bool,
    #[serde(default)]
    pub beep_on_short_pc: bool,

    // Audio
    #[serde(default = "default_beep_volume")]
    pub pc_beep_volume: f64,
    #[serde(default = "default_true")]
    pub dashboard_beep_master_enabled: bool,
    #[serde(default)]
    pub pc_beep_sound_preset: MacAlertSoundPreset,

    // DCV Alarms
    #[serde(default)]
    pub dcv_high_alarm_enabled: bool,
    #[serde(default = "default_high_alarm")]
    pub dcv_high_alarm_value: f64,
    #[serde(default)]
    pub dcv_low_alarm_enabled: bool,
    #[serde(default)]
    pub dcv_low_alarm_value: f64,
    #[serde(default)]
    pub beep_on_alarm: bool,

    // OBS output
    #[serde(default)]
    pub multimeter_output_file: String,
    #[serde(default)]
    pub usbc_output_file: String,
    #[serde(default)]
    pub multimeter_obs_output_mode: ObsOutputMode,
    #[serde(default)]
    pub usbc_obs_output_mode: ObsOutputMode,
    #[serde(default = "default_multimeter_obs_template")]
    pub multimeter_obs_custom_template: String,
    #[serde(default = "default_usbc_obs_template")]
    pub usbc_obs_custom_template: String,
    #[serde(default)]
    pub multimeter_value_label: String,
    #[serde(default)]
    pub usbc_value_label: String,

    // CSV logging
    #[serde(default)]
    pub multimeter_csv_logging_enabled: bool,
    #[serde(default)]
    pub usbc_csv_logging_enabled: bool,
    #[serde(default)]
    pub multimeter_csv_log_file_path: String,
    #[serde(default)]
    pub usbc_csv_log_file_path: String,

    // Dashboard UI
    #[serde(default)]
    pub dashboard_device_visibility: DashboardDeviceVisibility,
    #[serde(default)]
    pub dashboard_theme: DashboardTheme,
    #[serde(default = "default_true")]
    pub runtime_log_panel_visible: bool,
    #[serde(default = "default_true")]
    pub runtime_log_capture_enabled: bool,

    // Popout windows
    #[serde(default)]
    pub multimeter_popout_mode: PopoutDisplayMode,
    #[serde(default)]
    pub usbc_popout_mode: PopoutDisplayMode,
    pub multimeter_popout_frame: Option<PopoutWindowFrame>,
    pub usbc_popout_frame: Option<PopoutWindowFrame>,
    #[serde(default)]
    pub popout_alarm_emphasis_enabled: bool,
    #[serde(default)]
    pub popout_layout_profiles: Vec<PopoutLayoutProfile>,
    #[serde(default)]
    pub active_popout_layout_profile_name: String,
}

// --- Default value helpers ---

fn default_true() -> bool {
    true
}
fn default_sample_rate() -> u32 {
    10
}
fn default_graph_history() -> u32 {
    30
}
fn default_queue_capacity() -> u32 {
    256
}
fn default_queue_retries() -> u32 {
    3
}
fn default_short_threshold() -> f64 {
    2.0
}
fn default_beep_volume() -> f64 {
    0.5
}
fn default_high_alarm() -> f64 {
    12.0
}
fn default_multimeter_obs_template() -> String {
    "{value} {unit}".into()
}
fn default_usbc_obs_template() -> String {
    "{voltage} {current} {power}".into()
}

impl Default for AppConfiguration {
    fn default() -> Self {
        Self {
            multimeter_port: String::new(),
            usbc_port: String::new(),
            multimeter_enabled: true,
            usbc_enabled: false,
            multimeter_auto_reconnect: true,
            usbc_auto_reconnect: true,
            use_simulator: false,
            sample_rate_hz: 10,
            graph_history_seconds: 30,
            output_queue_capacity: 256,
            output_queue_max_retry_attempts: 3,
            short_threshold: 2.0,
            beep_on_short_meter: false,
            beep_on_short_pc: false,
            pc_beep_volume: 0.5,
            dashboard_beep_master_enabled: true,
            pc_beep_sound_preset: MacAlertSoundPreset::System,
            dcv_high_alarm_enabled: false,
            dcv_high_alarm_value: 12.0,
            dcv_low_alarm_enabled: false,
            dcv_low_alarm_value: 0.0,
            beep_on_alarm: false,
            multimeter_output_file: String::new(),
            usbc_output_file: String::new(),
            multimeter_obs_output_mode: ObsOutputMode::ValueOnly,
            usbc_obs_output_mode: ObsOutputMode::ValueOnly,
            multimeter_obs_custom_template: "{value} {unit}".into(),
            usbc_obs_custom_template: "{voltage} {current} {power}".into(),
            multimeter_value_label: String::new(),
            usbc_value_label: String::new(),
            multimeter_csv_logging_enabled: false,
            usbc_csv_logging_enabled: false,
            multimeter_csv_log_file_path: String::new(),
            usbc_csv_log_file_path: String::new(),
            dashboard_device_visibility: DashboardDeviceVisibility::Both,
            dashboard_theme: DashboardTheme::System,
            runtime_log_panel_visible: true,
            runtime_log_capture_enabled: true,
            multimeter_popout_mode: PopoutDisplayMode::Mini,
            usbc_popout_mode: PopoutDisplayMode::Mini,
            multimeter_popout_frame: None,
            usbc_popout_frame: None,
            popout_alarm_emphasis_enabled: false,
            popout_layout_profiles: Vec::new(),
            active_popout_layout_profile_name: String::new(),
        }
    }
}

impl AppConfiguration {
    /// Clamp numeric values to valid ranges.
    pub fn clamp_values(&mut self) {
        self.sample_rate_hz = self.sample_rate_hz.clamp(1, 50);
        self.graph_history_seconds = self.graph_history_seconds.clamp(5, 600);
        self.output_queue_capacity = self.output_queue_capacity.clamp(8, 2048);
        self.output_queue_max_retry_attempts = self.output_queue_max_retry_attempts.clamp(0, 10);
        self.pc_beep_volume = self.pc_beep_volume.clamp(0.0, 1.0);
        self.short_threshold = self.short_threshold.clamp(0.1, 1000.0);
        self.dcv_high_alarm_value = self.dcv_high_alarm_value.clamp(-1000.0, 1000.0);
        self.dcv_low_alarm_value = self.dcv_low_alarm_value.clamp(-1000.0, 1000.0);
        // Ensure low < high invariant
        if self.dcv_low_alarm_value >= self.dcv_high_alarm_value {
            self.dcv_low_alarm_value = self.dcv_high_alarm_value - 1.0;
        }
    }
}

// Custom Deserialize that applies clamping after deserialization
impl<'de> Deserialize<'de> for AppConfiguration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Use an inner struct with the same shape but derive(Deserialize)
        #[derive(Deserialize)]
        struct Inner {
            #[serde(default)]
            multimeter_port: String,
            #[serde(default)]
            usbc_port: String,
            #[serde(default = "default_true")]
            multimeter_enabled: bool,
            #[serde(default)]
            usbc_enabled: bool,
            #[serde(default = "default_true")]
            multimeter_auto_reconnect: bool,
            #[serde(default = "default_true")]
            usbc_auto_reconnect: bool,
            #[serde(default)]
            use_simulator: bool,
            #[serde(default = "default_sample_rate")]
            sample_rate_hz: u32,
            #[serde(default = "default_graph_history")]
            graph_history_seconds: u32,
            #[serde(default = "default_queue_capacity")]
            output_queue_capacity: u32,
            #[serde(default = "default_queue_retries")]
            output_queue_max_retry_attempts: u32,
            #[serde(default = "default_short_threshold")]
            short_threshold: f64,
            #[serde(default)]
            beep_on_short_meter: bool,
            #[serde(default)]
            beep_on_short_pc: bool,
            #[serde(default = "default_beep_volume")]
            pc_beep_volume: f64,
            #[serde(default = "default_true")]
            dashboard_beep_master_enabled: bool,
            #[serde(default)]
            pc_beep_sound_preset: MacAlertSoundPreset,
            #[serde(default)]
            dcv_high_alarm_enabled: bool,
            #[serde(default = "default_high_alarm")]
            dcv_high_alarm_value: f64,
            #[serde(default)]
            dcv_low_alarm_enabled: bool,
            #[serde(default)]
            dcv_low_alarm_value: f64,
            #[serde(default)]
            beep_on_alarm: bool,
            #[serde(default)]
            multimeter_output_file: String,
            #[serde(default)]
            usbc_output_file: String,
            #[serde(default)]
            multimeter_obs_output_mode: ObsOutputMode,
            #[serde(default)]
            usbc_obs_output_mode: ObsOutputMode,
            #[serde(default = "default_multimeter_obs_template")]
            multimeter_obs_custom_template: String,
            #[serde(default = "default_usbc_obs_template")]
            usbc_obs_custom_template: String,
            #[serde(default)]
            multimeter_value_label: String,
            #[serde(default)]
            usbc_value_label: String,
            #[serde(default)]
            multimeter_csv_logging_enabled: bool,
            #[serde(default)]
            usbc_csv_logging_enabled: bool,
            #[serde(default)]
            multimeter_csv_log_file_path: String,
            #[serde(default)]
            usbc_csv_log_file_path: String,
            #[serde(default)]
            dashboard_device_visibility: DashboardDeviceVisibility,
            #[serde(default)]
            dashboard_theme: DashboardTheme,
            #[serde(default = "default_true")]
            runtime_log_panel_visible: bool,
            #[serde(default = "default_true")]
            runtime_log_capture_enabled: bool,
            #[serde(default)]
            multimeter_popout_mode: PopoutDisplayMode,
            #[serde(default)]
            usbc_popout_mode: PopoutDisplayMode,
            multimeter_popout_frame: Option<PopoutWindowFrame>,
            usbc_popout_frame: Option<PopoutWindowFrame>,
            #[serde(default)]
            popout_alarm_emphasis_enabled: bool,
            #[serde(default)]
            popout_layout_profiles: Vec<PopoutLayoutProfile>,
            #[serde(default)]
            active_popout_layout_profile_name: String,
        }

        let inner = Inner::deserialize(deserializer)?;
        let mut config = AppConfiguration {
            multimeter_port: inner.multimeter_port,
            usbc_port: inner.usbc_port,
            multimeter_enabled: inner.multimeter_enabled,
            usbc_enabled: inner.usbc_enabled,
            multimeter_auto_reconnect: inner.multimeter_auto_reconnect,
            usbc_auto_reconnect: inner.usbc_auto_reconnect,
            use_simulator: inner.use_simulator,
            sample_rate_hz: inner.sample_rate_hz,
            graph_history_seconds: inner.graph_history_seconds,
            output_queue_capacity: inner.output_queue_capacity,
            output_queue_max_retry_attempts: inner.output_queue_max_retry_attempts,
            short_threshold: inner.short_threshold,
            beep_on_short_meter: inner.beep_on_short_meter,
            beep_on_short_pc: inner.beep_on_short_pc,
            pc_beep_volume: inner.pc_beep_volume,
            dashboard_beep_master_enabled: inner.dashboard_beep_master_enabled,
            pc_beep_sound_preset: inner.pc_beep_sound_preset,
            dcv_high_alarm_enabled: inner.dcv_high_alarm_enabled,
            dcv_high_alarm_value: inner.dcv_high_alarm_value,
            dcv_low_alarm_enabled: inner.dcv_low_alarm_enabled,
            dcv_low_alarm_value: inner.dcv_low_alarm_value,
            beep_on_alarm: inner.beep_on_alarm,
            multimeter_output_file: inner.multimeter_output_file,
            usbc_output_file: inner.usbc_output_file,
            multimeter_obs_output_mode: inner.multimeter_obs_output_mode,
            usbc_obs_output_mode: inner.usbc_obs_output_mode,
            multimeter_obs_custom_template: inner.multimeter_obs_custom_template,
            usbc_obs_custom_template: inner.usbc_obs_custom_template,
            multimeter_value_label: inner.multimeter_value_label,
            usbc_value_label: inner.usbc_value_label,
            multimeter_csv_logging_enabled: inner.multimeter_csv_logging_enabled,
            usbc_csv_logging_enabled: inner.usbc_csv_logging_enabled,
            multimeter_csv_log_file_path: inner.multimeter_csv_log_file_path,
            usbc_csv_log_file_path: inner.usbc_csv_log_file_path,
            dashboard_device_visibility: inner.dashboard_device_visibility,
            dashboard_theme: inner.dashboard_theme,
            runtime_log_panel_visible: inner.runtime_log_panel_visible,
            runtime_log_capture_enabled: inner.runtime_log_capture_enabled,
            multimeter_popout_mode: inner.multimeter_popout_mode,
            usbc_popout_mode: inner.usbc_popout_mode,
            multimeter_popout_frame: inner.multimeter_popout_frame,
            usbc_popout_frame: inner.usbc_popout_frame,
            popout_alarm_emphasis_enabled: inner.popout_alarm_emphasis_enabled,
            popout_layout_profiles: inner.popout_layout_profiles,
            active_popout_layout_profile_name: inner.active_popout_layout_profile_name,
        };
        config.clamp_values();
        Ok(config)
    }
}
