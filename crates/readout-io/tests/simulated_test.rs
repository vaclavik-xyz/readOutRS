use readout_io::simulated::*;
use readout_io::transport::*;

#[tokio::test]
async fn scpi_responds_to_idn() {
    let mut t = SimulatedScpiTransport::new(10);
    t.open().await.unwrap();
    let resp = t.query("*IDN?").await.unwrap();
    assert!(resp.is_some());
    assert!(resp.unwrap().contains("SIMULATED"));
}

#[tokio::test]
async fn scpi_responds_to_func() {
    let mut t = SimulatedScpiTransport::new(10);
    t.open().await.unwrap();
    let resp = t.query("FUNC?").await.unwrap();
    assert!(resp.is_some());
    let mode = resp.unwrap();
    // First mode should be VOLT:DC
    assert_eq!(mode, "VOLT:DC");
}

#[tokio::test]
async fn scpi_responds_to_meas() {
    let mut t = SimulatedScpiTransport::new(10);
    t.open().await.unwrap();
    let resp = t.query("MEAS?").await.unwrap();
    assert!(resp.is_some());
    let val = resp.unwrap();
    // Should be a parseable number
    assert!(val.trim().parse::<f64>().is_ok() || val.contains("OL"));
}

#[tokio::test]
async fn scpi_beeper_control() {
    let mut t = SimulatedScpiTransport::new(10);
    t.open().await.unwrap();
    // Enable beeper
    let _ = t.query("SYST:BEEP:STAT ON").await;
    let resp = t.query("SYST:BEEP:STAT?").await.unwrap().unwrap();
    assert_eq!(resp, "1");
    // Disable beeper
    let _ = t.query("SYST:BEEP:STAT OFF").await;
    let resp = t.query("SYST:BEEP:STAT?").await.unwrap().unwrap();
    assert_eq!(resp, "0");
}

#[tokio::test]
async fn scpi_mode_cycling() {
    let mut t = SimulatedScpiTransport::new(10);
    t.open().await.unwrap();

    // Read 220 samples to advance past first mode
    for _ in 0..220 {
        let _ = t.query("MEAS?").await;
    }
    let mode = t.query("FUNC?").await.unwrap().unwrap();
    assert_eq!(mode, "CURR:DC");
}

#[tokio::test]
async fn scpi_not_open_returns_error() {
    let mut t = SimulatedScpiTransport::new(10);
    let result = t.query("*IDN?").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn streaming_produces_valid_frames() {
    let mut t = SimulatedStreamingTransport::new(10);
    t.open().await.unwrap();
    let frame = t.read_frame().await.unwrap();
    assert!(frame.is_some());
    let f = frame.unwrap();
    assert_eq!(f.len(), 8);
    assert!(u32::from_str_radix(&f, 16).is_ok());
}

#[tokio::test]
async fn streaming_multiple_frames() {
    let mut t = SimulatedStreamingTransport::new(10);
    t.open().await.unwrap();
    for _ in 0..10 {
        let frame = t.read_frame().await.unwrap().unwrap();
        assert_eq!(frame.len(), 8);
    }
}

#[tokio::test]
async fn streaming_not_open_returns_error() {
    let mut t = SimulatedStreamingTransport::new(10);
    let result = t.read_frame().await;
    assert!(result.is_err());
}
