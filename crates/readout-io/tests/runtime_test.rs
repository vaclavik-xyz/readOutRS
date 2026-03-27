use readout_core::measurement_mode::MeasurementMode;
use readout_core::types::{Command, MultimeterCommand, RuntimeEvent};
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

#[tokio::test]
async fn meter_command_changes_mode() {
    let mut config = AppConfiguration::default();
    config.use_simulator = true;
    config.multimeter_enabled = true;
    config.usbc_enabled = false;
    config.sample_rate_hz = 10;

    let cancel = CancellationToken::new();
    let (runtime, mut event_rx) = Runtime::new(config);
    let cmd_tx = runtime.command_sender();

    let cancel_clone = cancel.clone();
    let handle = tokio::spawn(async move {
        runtime.run(cancel_clone).await;
    });

    // Wait for initial MeterState event (emitted after connect) with mode=DcVoltage
    let mut saw_initial_state = false;
    let initial_timeout = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while let Ok(event) = event_rx.recv().await {
            if let RuntimeEvent::MeterState { mode, .. } = &event {
                assert_eq!(*mode, MeasurementMode::DcVoltage);
                saw_initial_state = true;
                break;
            }
        }
    })
    .await;
    assert!(initial_timeout.is_ok());
    assert!(saw_initial_state);

    // Send SetMode(Resistance) command
    cmd_tx
        .send(Command::Meter(MultimeterCommand::SetMode(
            MeasurementMode::Resistance,
        )))
        .await
        .expect("send meter command");

    // Wait for MeterState event with mode=Resistance
    let mut saw_resistance_state = false;
    let mode_timeout = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while let Ok(event) = event_rx.recv().await {
            if let RuntimeEvent::MeterState { mode, .. } = &event {
                if *mode == MeasurementMode::Resistance {
                    saw_resistance_state = true;
                    break;
                }
            }
        }
    })
    .await;
    assert!(mode_timeout.is_ok());
    assert!(saw_resistance_state);

    cancel.cancel();
    let _ = handle.await;
}
