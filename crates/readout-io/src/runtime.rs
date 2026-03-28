use readout_core::types::{Command, DeviceId, RuntimeEvent};
use readout_persistence::config::AppConfiguration;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

use crate::multimeter_driver::MultimeterDriver;
use crate::serial_transport::SerialTransport;
use crate::simulated::{SimulatedScpiTransport, SimulatedStreamingTransport};
use crate::transport::DeviceTransport;
use crate::transport::ScpiTransport;
use crate::usbc_driver::UsbCDriver;

const EVENT_CHANNEL_CAPACITY: usize = 1024;
const COMMAND_CHANNEL_CAPACITY: usize = 64;
const MULTIMETER_BAUD_RATE: u32 = 115_200;
const USBC_BAUD_RATE: u32 = 9_600;

pub struct Runtime {
    config: AppConfiguration,
    event_tx: broadcast::Sender<RuntimeEvent>,
    command_tx: mpsc::Sender<Command>,
    command_rx: Option<mpsc::Receiver<Command>>,
    meter_beep_flag: Arc<AtomicBool>,
}

impl Runtime {
    pub fn new(config: AppConfiguration) -> (Self, broadcast::Receiver<RuntimeEvent>) {
        let (event_tx, event_rx) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let (command_tx, command_rx) = mpsc::channel(COMMAND_CHANNEL_CAPACITY);
        let meter_beep_flag = Arc::new(AtomicBool::new(config.beep_on_short_meter));

        let runtime = Self {
            config,
            event_tx,
            command_tx,
            command_rx: Some(command_rx),
            meter_beep_flag,
        };

        (runtime, event_rx)
    }

    /// Shared flag for live meter beep toggling from the GUI.
    pub fn meter_beep_flag(&self) -> Arc<AtomicBool> {
        self.meter_beep_flag.clone()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<RuntimeEvent> {
        self.event_tx.subscribe()
    }

    pub fn command_sender(&self) -> mpsc::Sender<Command> {
        self.command_tx.clone()
    }

    pub async fn run(mut self, cancel: CancellationToken) {
        let mut command_rx = self.command_rx.take().expect("run called only once");
        let device_cancel = CancellationToken::new();

        // Channel for forwarding commands to the USB-C device task
        let (usbc_cmd_tx, usbc_cmd_rx) = mpsc::channel::<Command>(16);

        // Channel for forwarding commands to the multimeter device task
        let (mm_cmd_tx, mm_cmd_rx) = mpsc::channel::<readout_core::types::MultimeterCommand>(16);

        // Spawn device tasks and retain handles for join
        let mm_handle = if self.config.multimeter_enabled {
            let event_tx = self.event_tx.clone();
            let cancel = device_cancel.clone();
            let sample_rate = self.config.sample_rate_hz;
            let use_simulator = self.config.use_simulator;
            let port = self.config.multimeter_port.clone();
            let meter_beep = self.config.beep_on_short_meter;
            let meter_beep_flag = self.meter_beep_flag.clone();
            let alert_config = readout_core::alerts::AlertConfiguration {
                short_threshold: self.config.short_threshold,
                dcv_high_alarm_enabled: self.config.dcv_high_alarm_enabled,
                dcv_high_alarm_value: self.config.dcv_high_alarm_value,
                dcv_low_alarm_enabled: self.config.dcv_low_alarm_enabled,
                dcv_low_alarm_value: self.config.dcv_low_alarm_value,
            };

            let auto_reconnect = self.config.multimeter_auto_reconnect;
            Some(tokio::spawn(async move {
                Self::run_multimeter(
                    use_simulator,
                    port,
                    sample_rate,
                    meter_beep,
                    meter_beep_flag,
                    alert_config,
                    event_tx,
                    cancel,
                    mm_cmd_rx,
                    auto_reconnect,
                )
                .await;
            }))
        } else {
            None
        };

        let usbc_handle = if self.config.usbc_enabled {
            let event_tx = self.event_tx.clone();
            let cancel = device_cancel.clone();
            let sample_rate = self.config.sample_rate_hz;
            let use_simulator = self.config.use_simulator;
            let port = self.config.usbc_port.clone();
            let auto_reconnect = self.config.usbc_auto_reconnect;

            Some(tokio::spawn(async move {
                Self::run_usbc(
                    use_simulator,
                    port,
                    sample_rate,
                    event_tx,
                    cancel,
                    usbc_cmd_rx,
                    auto_reconnect,
                )
                .await;
            }))
        } else {
            None
        };

        // Command loop
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    break;
                }
                cmd = command_rx.recv() => {
                    match cmd {
                        Some(Command::Stop) | None => {
                            break;
                        }
                        Some(Command::Rescan) => {
                            // Port discovery — for now just log
                            let _ = self.event_tx.send(RuntimeEvent::Log {
                                level: readout_core::types::LogLevel::Info,
                                message: "Port rescan requested".into(),
                            });
                        }
                        Some(cmd @ Command::ResetEnergy { .. }) => {
                            let _ = usbc_cmd_tx.send(cmd).await;
                        }
                        Some(Command::Meter(cmd)) => {
                            let _ = mm_cmd_tx.send(cmd).await;
                        }
                        Some(other) => {
                            tracing::debug!(?other, "command not yet handled");
                        }
                    }
                }
            }
        }

        // Graceful shutdown: cancel device tasks and join them
        device_cancel.cancel();

        if let Some(h) = mm_handle
            && let Err(e) = h.await
        {
            tracing::error!("Multimeter task failed: {e}");
        }
        if let Some(h) = usbc_handle
            && let Err(e) = h.await
        {
            tracing::error!("USB-C task failed: {e}");
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_multimeter(
        use_simulator: bool,
        port: String,
        sample_rate: u32,
        meter_beep: bool,
        meter_beep_flag: Arc<AtomicBool>,
        alert_config: readout_core::alerts::AlertConfiguration,
        event_tx: broadcast::Sender<RuntimeEvent>,
        cancel: CancellationToken,
        mut cmd_rx: mpsc::Receiver<readout_core::types::MultimeterCommand>,
        auto_reconnect: bool,
    ) {
        tracing::info!(use_simulator, %port, sample_rate, "Starting multimeter task");
        if use_simulator {
            let transport = SimulatedScpiTransport::new(sample_rate);
            let mut driver = MultimeterDriver::new(transport);
            driver.set_meter_beep(meter_beep);
            driver.set_alert_config(alert_config);
            Self::multimeter_loop(
                &mut driver,
                meter_beep_flag,
                event_tx,
                cancel,
                &mut cmd_rx,
                auto_reconnect,
            )
            .await;
        } else {
            let transport = SerialTransport::new(port, MULTIMETER_BAUD_RATE);
            let mut driver = MultimeterDriver::new(transport);
            driver.set_meter_beep(meter_beep);
            driver.set_alert_config(alert_config);
            Self::multimeter_loop(
                &mut driver,
                meter_beep_flag,
                event_tx,
                cancel,
                &mut cmd_rx,
                auto_reconnect,
            )
            .await;
        }
    }

    async fn multimeter_loop<T: ScpiTransport>(
        driver: &mut MultimeterDriver<T>,
        meter_beep_flag: Arc<AtomicBool>,
        event_tx: broadcast::Sender<RuntimeEvent>,
        cancel: CancellationToken,
        cmd_rx: &mut mpsc::Receiver<readout_core::types::MultimeterCommand>,
        auto_reconnect: bool,
    ) {
        let mut reconnect_delay = std::time::Duration::from_millis(500);
        let mut prev_alarm = readout_core::types::AlarmState::None;
        let mut current_beep_state = meter_beep_flag.load(Ordering::Relaxed);

        loop {
            if cancel.is_cancelled() {
                break;
            }

            let _ = event_tx.send(RuntimeEvent::ConnectionChanged {
                device: DeviceId::Multimeter,
                state: readout_core::types::ConnectionState::Connecting,
            });

            if let Err(e) = driver.connect().await {
                let _ = event_tx.send(RuntimeEvent::Error {
                    device: DeviceId::Multimeter,
                    message: format!("Failed to connect: {e}"),
                });
            } else {
                let _ = event_tx.send(RuntimeEvent::ConnectionChanged {
                    device: DeviceId::Multimeter,
                    state: readout_core::types::ConnectionState::Connected,
                });
                reconnect_delay = std::time::Duration::from_millis(500);

                // Emit initial MeterState after connect
                let identity = driver.query_identity().await;
                emit_meter_state(driver, &event_tx, identity).await;

                let mut consecutive_errors: u32 = 0;
                loop {
                    if cancel.is_cancelled() {
                        driver.close().await;
                        let _ = event_tx.send(RuntimeEvent::ConnectionChanged {
                            device: DeviceId::Multimeter,
                            state: readout_core::types::ConnectionState::Disconnected,
                        });
                        return;
                    }

                    // Drain pending meter commands
                    while let Ok(cmd) = cmd_rx.try_recv() {
                        Self::handle_meter_command(driver, &event_tx, cmd).await;
                    }

                    // Live meter beep toggle
                    let desired = meter_beep_flag.load(Ordering::Relaxed);
                    if desired != current_beep_state {
                        driver.set_beeper(desired).await;
                        current_beep_state = desired;
                    }

                    match driver.poll().await {
                        Ok(measurement) => {
                            consecutive_errors = 0;
                            let new_alarm = measurement.alarm_state;
                            let _ = event_tx.send(RuntimeEvent::Measurement {
                                device: DeviceId::Multimeter,
                                value: measurement,
                            });
                            if new_alarm != prev_alarm {
                                if new_alarm == readout_core::types::AlarmState::None {
                                    let _ = event_tx.send(RuntimeEvent::AlarmCleared {
                                        device: DeviceId::Multimeter,
                                    });
                                } else {
                                    let _ = event_tx.send(RuntimeEvent::AlarmTriggered {
                                        device: DeviceId::Multimeter,
                                        alarm: new_alarm,
                                    });
                                }
                                prev_alarm = new_alarm;
                            }
                        }
                        Err(e) => {
                            consecutive_errors += 1;
                            let _ = event_tx.send(RuntimeEvent::Error {
                                device: DeviceId::Multimeter,
                                message: e.to_string(),
                            });
                            if consecutive_errors >= 5 {
                                tracing::warn!(
                                    "Multimeter: too many consecutive errors, will reconnect"
                                );
                                driver.close().await;
                                prev_alarm = readout_core::types::AlarmState::None;
                                break;
                            }
                            tokio::select! {
                                _ = cancel.cancelled() => {
                                    driver.close().await;
                                    let _ = event_tx.send(RuntimeEvent::ConnectionChanged {
                                        device: DeviceId::Multimeter,
                                        state: readout_core::types::ConnectionState::Disconnected,
                                    });
                                    return;
                                }
                                _ = tokio::time::sleep(std::time::Duration::from_millis(200)) => {}
                            }
                        }
                    }
                }
            }

            if !auto_reconnect {
                break;
            }

            // Reconnect with cancellation-aware backoff
            let _ = event_tx.send(RuntimeEvent::ConnectionChanged {
                device: DeviceId::Multimeter,
                state: readout_core::types::ConnectionState::Reconnecting,
            });
            tokio::select! {
                _ = cancel.cancelled() => { break; }
                _ = tokio::time::sleep(reconnect_delay) => {}
            }
            reconnect_delay = (reconnect_delay * 2).min(std::time::Duration::from_secs(5));
        }

        let _ = event_tx.send(RuntimeEvent::ConnectionChanged {
            device: DeviceId::Multimeter,
            state: readout_core::types::ConnectionState::Disconnected,
        });
    }

    async fn handle_meter_command<T: ScpiTransport>(
        driver: &mut MultimeterDriver<T>,
        event_tx: &broadcast::Sender<RuntimeEvent>,
        cmd: readout_core::types::MultimeterCommand,
    ) {
        use readout_core::types::MultimeterCommand;
        match cmd {
            MultimeterCommand::QueryIdentity => {
                let identity = driver.query_identity().await;
                emit_meter_state(driver, event_tx, identity).await;
            }
            MultimeterCommand::SetMode(mode) => {
                if let Err(e) = driver.set_mode(mode).await {
                    tracing::warn!("set_mode failed: {e}");
                }
                emit_meter_state(driver, event_tx, None).await;
            }
            MultimeterCommand::SetRange(range) => {
                if let Err(e) = driver.set_range(range).await {
                    tracing::warn!("set_range failed: {e}");
                }
                emit_meter_state(driver, event_tx, None).await;
            }
            MultimeterCommand::SetRate(rate) => {
                if let Err(e) = driver.set_rate(rate).await {
                    tracing::warn!("set_rate failed: {e}");
                }
                emit_meter_state(driver, event_tx, None).await;
            }
            MultimeterCommand::SetDualDisplay(enabled) => {
                if let Err(e) = driver.set_dual_display(enabled).await {
                    tracing::warn!("set_dual_display failed: {e}");
                }
                emit_meter_state(driver, event_tx, None).await;
            }
            MultimeterCommand::SetNull(enabled) => {
                if let Err(e) = driver.set_null(enabled).await {
                    tracing::warn!("set_null failed: {e}");
                }
                emit_meter_state(driver, event_tx, None).await;
            }
            MultimeterCommand::SetDcFilter(enabled) => {
                if let Err(e) = driver.set_dc_filter(enabled).await {
                    tracing::warn!("set_dc_filter failed: {e}");
                }
                emit_meter_state(driver, event_tx, None).await;
            }
            MultimeterCommand::SetAutoImpedance(enabled) => {
                if let Err(e) = driver.set_auto_impedance(enabled).await {
                    tracing::warn!("set_auto_impedance failed: {e}");
                }
                emit_meter_state(driver, event_tx, None).await;
            }
            MultimeterCommand::SetContinuityThreshold(ohms) => {
                if let Err(e) = driver.set_continuity_threshold(ohms).await {
                    tracing::warn!("set_continuity_threshold failed: {e}");
                }
            }
            MultimeterCommand::SetTempSensorType(sensor) => {
                if let Err(e) = driver.set_temp_sensor_type(sensor).await {
                    tracing::warn!("set_temp_sensor_type failed: {e}");
                }
            }
            MultimeterCommand::SetTempUnit(unit) => {
                if let Err(e) = driver.set_temp_unit(unit).await {
                    tracing::warn!("set_temp_unit failed: {e}");
                }
            }
            MultimeterCommand::StartMath(func) => {
                if let Err(e) = driver.start_math(func).await {
                    tracing::warn!("start_math failed: {e}");
                }
                emit_meter_state(driver, event_tx, None).await;
            }
            MultimeterCommand::StopMath => {
                if let Err(e) = driver.stop_math().await {
                    tracing::warn!("stop_math failed: {e}");
                }
                emit_meter_state(driver, event_tx, None).await;
            }
            MultimeterCommand::QueryMathStats => {
                // Query fresh stats and emit full state in one call
                let stats = driver.query_math_stats().await;
                let mut state = driver.query_state().await;
                state.math_stats = stats;
                let _ = event_tx.send(RuntimeEvent::MeterState {
                    identity: None,
                    mode: state.mode,
                    range_label: state.range_label,
                    rate: state.rate,
                    auto_range: state.auto_range,
                    dual_display: state.dual_display,
                    null_enabled: state.null_enabled,
                    dc_filter: state.dc_filter,
                    auto_impedance: state.auto_impedance,
                    math_function: state.math_function,
                    math_stats: state.math_stats,
                });
            }
            MultimeterCommand::SetDbReference(reference) => {
                if let Err(e) = driver.set_db_reference(reference).await {
                    tracing::warn!("set_db_reference failed: {e}");
                }
            }
            MultimeterCommand::SetRemoteMode(remote) => {
                if let Err(e) = driver.set_remote_mode(remote).await {
                    tracing::warn!("set_remote_mode failed: {e}");
                }
            }
            MultimeterCommand::ResetDevice => {
                if let Err(e) = driver.reset_device().await {
                    tracing::warn!("reset_device failed: {e}");
                }
                emit_meter_state(driver, event_tx, None).await;
            }
        }
    }

    async fn run_usbc(
        use_simulator: bool,
        port: String,
        sample_rate: u32,
        event_tx: broadcast::Sender<RuntimeEvent>,
        cancel: CancellationToken,
        mut cmd_rx: mpsc::Receiver<Command>,
        auto_reconnect: bool,
    ) {
        tracing::info!(use_simulator, %port, sample_rate, "Starting USB-C task");
        if use_simulator {
            let transport = SimulatedStreamingTransport::new(sample_rate);
            let mut driver = UsbCDriver::new(transport);
            Self::usbc_loop(&mut driver, event_tx, cancel, &mut cmd_rx, auto_reconnect).await;
        } else {
            let transport = SerialTransport::new(port, USBC_BAUD_RATE);
            let mut driver = UsbCDriver::new(transport);
            Self::usbc_loop(&mut driver, event_tx, cancel, &mut cmd_rx, auto_reconnect).await;
        }
    }

    async fn usbc_loop<T: DeviceTransport>(
        driver: &mut UsbCDriver<T>,
        event_tx: broadcast::Sender<RuntimeEvent>,
        cancel: CancellationToken,
        cmd_rx: &mut mpsc::Receiver<Command>,
        auto_reconnect: bool,
    ) {
        let mut reconnect_delay = std::time::Duration::from_millis(500);

        loop {
            if cancel.is_cancelled() {
                break;
            }

            let _ = event_tx.send(RuntimeEvent::ConnectionChanged {
                device: DeviceId::UsbC,
                state: readout_core::types::ConnectionState::Connecting,
            });

            if let Err(e) = driver.connect().await {
                let _ = event_tx.send(RuntimeEvent::Error {
                    device: DeviceId::UsbC,
                    message: format!("Failed to connect: {e}"),
                });
            } else {
                let _ = event_tx.send(RuntimeEvent::ConnectionChanged {
                    device: DeviceId::UsbC,
                    state: readout_core::types::ConnectionState::Connected,
                });
                reconnect_delay = std::time::Duration::from_millis(500);

                let mut consecutive_errors: u32 = 0;
                loop {
                    if cancel.is_cancelled() {
                        driver.close().await;
                        let _ = event_tx.send(RuntimeEvent::ConnectionChanged {
                            device: DeviceId::UsbC,
                            state: readout_core::types::ConnectionState::Disconnected,
                        });
                        return;
                    }

                    // Drain pending commands before the next blocking read.
                    // `read_measurement()` is not cancel-safe for serial transports because
                    // the transport temporarily takes ownership of the underlying port.
                    while let Ok(cmd) = cmd_rx.try_recv() {
                        Self::handle_usbc_command(driver, &event_tx, cmd);
                    }

                    match driver.read_measurement().await {
                        Ok(measurement) => {
                            consecutive_errors = 0;
                            let _ = event_tx.send(RuntimeEvent::Measurement {
                                device: DeviceId::UsbC,
                                value: measurement,
                            });
                        }
                        Err(e) => {
                            consecutive_errors += 1;
                            let _ = event_tx.send(RuntimeEvent::Error {
                                device: DeviceId::UsbC,
                                message: e.to_string(),
                            });
                            if consecutive_errors >= 5 {
                                tracing::warn!(
                                    "USB-C: too many consecutive errors, will reconnect"
                                );
                                driver.close().await;
                                break;
                            }
                            tokio::select! {
                                _ = cancel.cancelled() => {
                                    driver.close().await;
                                    let _ = event_tx.send(RuntimeEvent::ConnectionChanged {
                                        device: DeviceId::UsbC,
                                        state: readout_core::types::ConnectionState::Disconnected,
                                    });
                                    return;
                                }
                                _ = tokio::time::sleep(std::time::Duration::from_millis(200)) => {}
                            }
                        }
                    }
                }
            }

            if !auto_reconnect {
                break;
            }

            let _ = event_tx.send(RuntimeEvent::ConnectionChanged {
                device: DeviceId::UsbC,
                state: readout_core::types::ConnectionState::Reconnecting,
            });
            tokio::select! {
                _ = cancel.cancelled() => { break; }
                _ = tokio::time::sleep(reconnect_delay) => {}
            }
            reconnect_delay = (reconnect_delay * 2).min(std::time::Duration::from_secs(5));
        }

        let _ = event_tx.send(RuntimeEvent::ConnectionChanged {
            device: DeviceId::UsbC,
            state: readout_core::types::ConnectionState::Disconnected,
        });
    }

    fn handle_usbc_command<T: DeviceTransport>(
        driver: &mut UsbCDriver<T>,
        event_tx: &broadcast::Sender<RuntimeEvent>,
        cmd: Command,
    ) {
        if let Command::ResetEnergy { .. } = cmd {
            driver.reset_energy();
            let _ = event_tx.send(RuntimeEvent::Log {
                level: readout_core::types::LogLevel::Info,
                message: "USB-C energy counter reset".into(),
            });
        }
    }
}

async fn emit_meter_state<T: ScpiTransport>(
    driver: &mut MultimeterDriver<T>,
    event_tx: &broadcast::Sender<RuntimeEvent>,
    identity: Option<String>,
) {
    let state = driver.query_state().await;
    let _ = event_tx.send(RuntimeEvent::MeterState {
        identity,
        mode: state.mode,
        range_label: state.range_label,
        rate: state.rate,
        auto_range: state.auto_range,
        dual_display: state.dual_display,
        null_enabled: state.null_enabled,
        dc_filter: state.dc_filter,
        auto_impedance: state.auto_impedance,
        math_function: state.math_function,
        math_stats: state.math_stats,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::TransportError;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CancellationUnsafeUsbTransport {
        open_count: Arc<AtomicUsize>,
        port_present: bool,
        frame_delay: std::time::Duration,
    }

    impl CancellationUnsafeUsbTransport {
        fn new(open_count: Arc<AtomicUsize>, frame_delay: std::time::Duration) -> Self {
            Self {
                open_count,
                port_present: false,
                frame_delay,
            }
        }
    }

    impl DeviceTransport for CancellationUnsafeUsbTransport {
        async fn open(&mut self) -> Result<(), TransportError> {
            self.open_count.fetch_add(1, Ordering::SeqCst);
            self.port_present = true;
            Ok(())
        }

        async fn close(&mut self) {
            self.port_present = false;
        }

        async fn read_frame(&mut self) -> Result<Option<String>, TransportError> {
            if !self.port_present {
                return Err(TransportError::NotOpen);
            }

            // Mimic the serial transport bug shape: the underlying port is temporarily taken
            // out of the transport and only put back when the read future completes.
            self.port_present = false;
            tokio::time::sleep(self.frame_delay).await;
            self.port_present = true;
            Ok(Some("03E80BB8".into()))
        }
    }

    #[tokio::test]
    async fn reset_energy_does_not_force_usbc_reconnect() {
        let open_count = Arc::new(AtomicUsize::new(0));
        let transport = CancellationUnsafeUsbTransport::new(
            open_count.clone(),
            std::time::Duration::from_millis(100),
        );
        let mut driver = UsbCDriver::new(transport);
        let (event_tx, mut event_rx) = broadcast::channel(64);
        let (cmd_tx, mut cmd_rx) = mpsc::channel(8);
        let cancel = CancellationToken::new();

        let cancel_clone = cancel.clone();
        let handle = tokio::spawn(async move {
            Runtime::usbc_loop(&mut driver, event_tx, cancel_clone, &mut cmd_rx, true).await;
        });

        let mut saw_connected = false;
        while let Ok(event) =
            tokio::time::timeout(std::time::Duration::from_secs(1), event_rx.recv()).await
        {
            match event.expect("event channel closed unexpectedly") {
                RuntimeEvent::ConnectionChanged {
                    device: DeviceId::UsbC,
                    state: readout_core::types::ConnectionState::Connected,
                } => {
                    saw_connected = true;
                    break;
                }
                _ => {}
            }
        }
        assert!(saw_connected);

        // Send reset while the next read is already in flight.
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        cmd_tx
            .send(Command::ResetEnergy {
                device: DeviceId::UsbC,
            })
            .await
            .expect("reset command send");

        let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(1300);
        let mut saw_reset_log = false;
        let mut saw_reconnect = false;
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(50), event_rx.recv()).await
            {
                Ok(Ok(RuntimeEvent::Log { message, .. }))
                    if message == "USB-C energy counter reset" =>
                {
                    saw_reset_log = true;
                }
                Ok(Ok(RuntimeEvent::ConnectionChanged {
                    device: DeviceId::UsbC,
                    state: readout_core::types::ConnectionState::Reconnecting,
                })) => {
                    saw_reconnect = true;
                    break;
                }
                Ok(Ok(_)) => {}
                Ok(Err(_)) => break,
                Err(_) => {}
            }
        }

        cancel.cancel();
        let _ = handle.await;

        assert!(saw_reset_log);
        assert!(!saw_reconnect);
        assert_eq!(open_count.load(Ordering::SeqCst), 1);
    }
}
