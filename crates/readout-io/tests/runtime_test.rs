use readout_core::types::{Command, RuntimeEvent};
use readout_io::runtime::Runtime;
use readout_persistence::config::AppConfiguration;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn runtime_receives_measurements() {
    let mut config = AppConfiguration::default();
    config.use_simulator = true;
    config.multimeter_enabled = true;
    config.usbc_enabled = true;
    config.sample_rate_hz = 10;

    let cancel = CancellationToken::new();
    let (runtime, mut event_rx) = Runtime::new(config);

    let cancel_clone = cancel.clone();
    let handle = tokio::spawn(async move {
        runtime.run(cancel_clone).await;
    });

    let mut measurement_count = 0;
    let timeout = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while let Ok(event) = event_rx.recv().await {
            if matches!(event, RuntimeEvent::Measurement { .. }) {
                measurement_count += 1;
                if measurement_count >= 5 {
                    break;
                }
            }
        }
    })
    .await;

    assert!(timeout.is_ok());
    assert!(measurement_count >= 5);

    cancel.cancel();
    let _ = handle.await;
}

#[tokio::test]
async fn runtime_stop_command_shuts_down() {
    let mut config = AppConfiguration::default();
    config.use_simulator = true;
    config.multimeter_enabled = true;
    config.sample_rate_hz = 10;

    let cancel = CancellationToken::new();
    let (runtime, _event_rx) = Runtime::new(config);
    let cmd_tx = runtime.command_sender();

    let cancel_clone = cancel.clone();
    let handle = tokio::spawn(async move {
        runtime.run(cancel_clone).await;
    });

    // Give it time to start
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Send stop
    let _ = cmd_tx.send(Command::Stop).await;

    let result = tokio::time::timeout(std::time::Duration::from_secs(3), handle).await;
    assert!(result.is_ok());
}
