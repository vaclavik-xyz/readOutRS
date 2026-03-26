#![cfg(feature = "soak")]

use readout_core::types::RuntimeEvent;
use readout_io::runtime::Runtime;
use readout_persistence::config::AppConfiguration;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn soak_smoke_simulated() {
    let mut config = AppConfiguration::default();
    config.use_simulator = true;
    config.multimeter_enabled = true;
    config.usbc_enabled = true;
    config.sample_rate_hz = 50; // max rate for stress

    let cancel = CancellationToken::new();
    let (runtime, mut event_rx) = Runtime::new(config);

    let cancel_clone = cancel.clone();
    let runtime_handle = tokio::spawn(async move {
        runtime.run(cancel_clone).await;
    });

    let target_frames = 400;
    let mut mm_count: u64 = 0;
    let mut usbc_count: u64 = 0;
    let mut error_count: u64 = 0;
    let start = Instant::now();
    let timeout = Duration::from_secs(60);

    loop {
        if start.elapsed() > timeout {
            break;
        }
        if mm_count >= target_frames && usbc_count >= target_frames {
            break;
        }

        match tokio::time::timeout(Duration::from_secs(5), event_rx.recv()).await {
            Ok(Ok(event)) => match event {
                RuntimeEvent::Measurement { device, .. } => match device {
                    readout_core::types::DeviceId::Multimeter => mm_count += 1,
                    readout_core::types::DeviceId::UsbC => usbc_count += 1,
                },
                RuntimeEvent::Error { .. } => error_count += 1,
                _ => {}
            },
            Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(n))) => {
                eprintln!("soak: lagged {n} events");
            }
            Ok(Err(_)) => break,
            Err(_) => {
                // Timeout — cancel runtime before panicking
                cancel.cancel();
                let _ = tokio::time::timeout(Duration::from_secs(5), runtime_handle).await;
                panic!("soak: timed out waiting for events");
            }
        }
    }

    let elapsed = start.elapsed();

    // Graceful shutdown — cancel and wait for runtime with timeout
    cancel.cancel();
    tokio::time::timeout(Duration::from_secs(10), runtime_handle)
        .await
        .expect("runtime did not shut down within 10s")
        .expect("runtime task panicked during soak test");

    // Report
    let report = serde_json::json!({
        "multimeter_frames": mm_count,
        "usbc_frames": usbc_count,
        "errors": error_count,
        "elapsed_secs": elapsed.as_secs_f64(),
        "mm_fps": mm_count as f64 / elapsed.as_secs_f64(),
        "usbc_fps": usbc_count as f64 / elapsed.as_secs_f64(),
    });
    eprintln!(
        "soak report: {}",
        serde_json::to_string_pretty(&report).unwrap()
    );

    // Write report to /tmp for CI artifact
    let _ = std::fs::write(
        "/tmp/soak-report.json",
        serde_json::to_string_pretty(&report).unwrap(),
    );

    // Assertions
    assert!(
        mm_count >= target_frames,
        "multimeter frames: {mm_count} < {target_frames}"
    );
    assert!(
        usbc_count >= target_frames,
        "usbc frames: {usbc_count} < {target_frames}"
    );
    assert!(error_count < 10, "too many errors: {error_count}");
}
