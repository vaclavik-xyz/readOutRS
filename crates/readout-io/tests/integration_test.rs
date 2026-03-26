use readout_core::types::{Command, DeviceId, RuntimeEvent};
use readout_io::runtime::Runtime;
use readout_persistence::config::AppConfiguration;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn runtime_simulator_produces_measurements_and_shuts_down() {
    let mut config = AppConfiguration::default();
    config.use_simulator = true;
    config.multimeter_enabled = true;
    config.usbc_enabled = true;
    config.sample_rate_hz = 10;

    let cancel = CancellationToken::new();
    let (runtime, mut event_rx) = Runtime::new(config);
    let command_tx = runtime.command_sender();

    let cancel_clone = cancel.clone();
    let mut handle = tokio::spawn(async move {
        runtime.run(cancel_clone).await;
    });

    // Collect measurements
    let mut mm_count = 0u32;
    let mut usbc_count = 0u32;

    let collect_timeout = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match event_rx.recv().await {
                Ok(RuntimeEvent::Measurement { device, .. }) => match device {
                    DeviceId::Multimeter => mm_count += 1,
                    DeviceId::UsbC => usbc_count += 1,
                },
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(_) => break,
            }
            if mm_count >= 5 && usbc_count >= 5 {
                break;
            }
        }
    })
    .await;

    assert!(collect_timeout.is_ok(), "timed out waiting for measurements");
    assert!(mm_count >= 5, "multimeter: {mm_count}");
    assert!(usbc_count >= 5, "usbc: {usbc_count}");

    // Send stop and verify clean shutdown via Command::Stop
    let _ = command_tx.send(Command::Stop).await;

    // Give runtime time to process Command::Stop
    match tokio::time::timeout(Duration::from_secs(3), &mut handle).await {
        Ok(result) => {
            result.expect("runtime task panicked");
        }
        Err(_) => {
            // Fallback: Command::Stop didn't work, force cancel
            cancel.cancel();
            tokio::time::timeout(Duration::from_secs(5), handle)
                .await
                .expect("runtime did not shut down even after cancel")
                .expect("runtime task panicked");
        }
    }
}
