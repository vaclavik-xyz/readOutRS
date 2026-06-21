use readout_persistence::config::*;
use readout_persistence::config_store::{self, ConfigStoreError};

#[test]
fn default_config_is_valid() {
    let config = AppConfiguration::default();
    assert_eq!(config.sample_rate_hz, 10);
    assert!(!config.use_simulator);
    assert!(config.multimeter_enabled);
}

#[test]
fn obs_enabled_defaults_to_true() {
    let config = AppConfiguration::default();
    assert!(config.multimeter_obs_enabled);
    assert!(config.usbc_obs_enabled);
}

#[test]
fn obs_enabled_round_trips_through_json() {
    let mut config = AppConfiguration::default();
    config.multimeter_obs_enabled = false;
    config.usbc_obs_enabled = false;
    let json = serde_json::to_string(&config).unwrap();
    let loaded: AppConfiguration = serde_json::from_str(&json).unwrap();
    assert!(!loaded.multimeter_obs_enabled);
    assert!(!loaded.usbc_obs_enabled);
}

#[test]
fn obs_enabled_missing_in_json_defaults_to_true() {
    let json = r#"{"multimeter_output_file": "test.txt"}"#;
    let config: AppConfiguration = serde_json::from_str(json).unwrap();
    assert!(config.multimeter_obs_enabled);
    assert!(config.usbc_obs_enabled);
}

#[test]
fn deserialize_with_missing_keys_uses_defaults() {
    let json = r#"{"multimeter_port": "/dev/ttyUSB0"}"#;
    let config: AppConfiguration = serde_json::from_str(json).unwrap();
    assert_eq!(config.multimeter_port, "/dev/ttyUSB0");
    assert_eq!(config.sample_rate_hz, 10); // default
    assert!(!config.use_simulator); // default
}

#[test]
fn clamp_values_enforces_ranges() {
    let json = r#"{"sample_rate_hz": 999, "pc_beep_volume": 5.0}"#;
    let config: AppConfiguration = serde_json::from_str(json).unwrap();
    assert_eq!(config.sample_rate_hz, 50); // clamped to max
    assert!((config.pc_beep_volume - 1.0).abs() < 0.001); // clamped to max
}

#[test]
fn clamp_values_enforces_minimums() {
    let json = r#"{"sample_rate_hz": 0, "pc_beep_volume": -1.0}"#;
    let config: AppConfiguration = serde_json::from_str(json).unwrap();
    assert_eq!(config.sample_rate_hz, 1);
    assert!((config.pc_beep_volume - 0.0).abs() < 0.001);
}

#[test]
fn roundtrip_serialize_deserialize() {
    let original = AppConfiguration::default();
    let json = serde_json::to_string_pretty(&original).unwrap();
    let restored: AppConfiguration = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
}

#[test]
fn case_insensitive_theme_parsing() {
    let json = r#"{"dashboard_theme": "DARK"}"#;
    let config: AppConfiguration = serde_json::from_str(json).unwrap();
    assert_eq!(config.dashboard_theme, DashboardTheme::Dark);
}

#[test]
fn startup_load_treats_missing_default_config_as_first_run() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing-default.json");

    let loaded = config_store::load_for_startup(&path, false).unwrap();

    assert!(loaded.first_run);
    assert_eq!(loaded.config, AppConfiguration::default());
}

#[test]
fn startup_load_rejects_missing_explicit_config_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing-explicit.json");

    let err = config_store::load_for_startup(&path, true).unwrap_err();

    assert!(matches!(err, ConfigStoreError::ReadFailed(_)));
}

#[test]
fn startup_load_rejects_existing_malformed_config() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    std::fs::write(&path, "{not-json").unwrap();

    let err = config_store::load_for_startup(&path, false).unwrap_err();

    assert!(matches!(err, ConfigStoreError::ParseFailed(_)));
}
