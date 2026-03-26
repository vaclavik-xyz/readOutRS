use readout_core::types::{Command, DeviceId, RuntimeEvent};
use readout_persistence::config::AppConfiguration;
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

use crate::multimeter_driver::MultimeterDriver;
use crate::simulated::{SimulatedScpiTransport, SimulatedStreamingTransport};
use crate::usbc_driver::UsbCDriver;

const EVENT_CHANNEL_CAPACITY: usize = 1024;
const COMMAND_CHANNEL_CAPACITY: usize = 64;

pub struct Runtime {
    config: AppConfiguration,
    event_tx: broadcast::Sender<RuntimeEvent>,
    command_tx: mpsc::Sender<Command>,
    command_rx: Option<mpsc::Receiver<Command>>,
}

impl Runtime {
    pub fn new(config: AppConfiguration) -> (Self, broadcast::Receiver<RuntimeEvent>) {
        let (event_tx, event_rx) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let (command_tx, command_rx) = mpsc::channel(COMMAND_CHANNEL_CAPACITY);

        let runtime = Self {
            config,
            event_tx,
            command_tx,
            command_rx: Some(command_rx),
        };

        (runtime, event_rx)
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

        // Spawn device tasks and retain handles for join
        let mm_handle = if self.config.multimeter_enabled {
            let event_tx = self.event_tx.clone();
            let cancel = device_cancel.clone();
            let sample_rate = self.config.sample_rate_hz;

            Some(tokio::spawn(async move {
                Self::run_multimeter(sample_rate, event_tx, cancel).await;
            }))
        } else {
            None
        };

        let usbc_handle = if self.config.usbc_enabled {
            let event_tx = self.event_tx.clone();
            let cancel = device_cancel.clone();
            let sample_rate = self.config.sample_rate_hz;

            Some(tokio::spawn(async move {
                Self::run_usbc(sample_rate, event_tx, cancel).await;
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
                        Some(other) => {
                            tracing::debug!(?other, "command not yet handled");
                        }
                    }
                }
            }
        }

        // Graceful shutdown: cancel device tasks and join them
        device_cancel.cancel();

        if let Some(h) = mm_handle {
            let _ = h.await;
        }
        if let Some(h) = usbc_handle {
            let _ = h.await;
        }
    }

    async fn run_multimeter(
        sample_rate: u32,
        event_tx: broadcast::Sender<RuntimeEvent>,
        cancel: CancellationToken,
    ) {
        let transport = SimulatedScpiTransport::new(sample_rate);
        let mut driver = MultimeterDriver::new(transport);

        let _ = event_tx.send(RuntimeEvent::ConnectionChanged {
            device: DeviceId::Multimeter,
            state: readout_core::types::ConnectionState::Connecting,
        });

        if driver.connect().await.is_err() {
            let _ = event_tx.send(RuntimeEvent::Error {
                device: DeviceId::Multimeter,
                message: "Failed to connect".into(),
            });
            return;
        }

        let _ = event_tx.send(RuntimeEvent::ConnectionChanged {
            device: DeviceId::Multimeter,
            state: readout_core::types::ConnectionState::Connected,
        });

        let mut consecutive_errors: u32 = 0;
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    driver.close().await;
                    let _ = event_tx.send(RuntimeEvent::ConnectionChanged {
                        device: DeviceId::Multimeter,
                        state: readout_core::types::ConnectionState::Disconnected,
                    });
                    return;
                }
                result = driver.poll() => {
                    match result {
                        Ok(measurement) => {
                            consecutive_errors = 0;
                            let _ = event_tx.send(RuntimeEvent::Measurement {
                                device: DeviceId::Multimeter,
                                value: measurement,
                            });
                        }
                        Err(e) => {
                            consecutive_errors += 1;
                            let _ = event_tx.send(RuntimeEvent::Error {
                                device: DeviceId::Multimeter,
                                message: e.to_string(),
                            });
                            if consecutive_errors >= 5 {
                                tracing::warn!("Multimeter: too many consecutive errors, disconnecting");
                                driver.close().await;
                                let _ = event_tx.send(RuntimeEvent::ConnectionChanged {
                                    device: DeviceId::Multimeter,
                                    state: readout_core::types::ConnectionState::Disconnected,
                                });
                                return;
                            }
                            // Backoff before retry
                            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                        }
                    }
                }
            }
        }
    }

    async fn run_usbc(
        sample_rate: u32,
        event_tx: broadcast::Sender<RuntimeEvent>,
        cancel: CancellationToken,
    ) {
        let transport = SimulatedStreamingTransport::new(sample_rate);
        let mut driver = UsbCDriver::new(transport);

        let _ = event_tx.send(RuntimeEvent::ConnectionChanged {
            device: DeviceId::UsbC,
            state: readout_core::types::ConnectionState::Connecting,
        });

        if driver.connect().await.is_err() {
            let _ = event_tx.send(RuntimeEvent::Error {
                device: DeviceId::UsbC,
                message: "Failed to connect".into(),
            });
            return;
        }

        let _ = event_tx.send(RuntimeEvent::ConnectionChanged {
            device: DeviceId::UsbC,
            state: readout_core::types::ConnectionState::Connected,
        });

        let mut consecutive_errors: u32 = 0;
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    driver.close().await;
                    let _ = event_tx.send(RuntimeEvent::ConnectionChanged {
                        device: DeviceId::UsbC,
                        state: readout_core::types::ConnectionState::Disconnected,
                    });
                    return;
                }
                result = driver.read_measurement() => {
                    match result {
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
                                tracing::warn!("USB-C: too many consecutive errors, disconnecting");
                                driver.close().await;
                                let _ = event_tx.send(RuntimeEvent::ConnectionChanged {
                                    device: DeviceId::UsbC,
                                    state: readout_core::types::ConnectionState::Disconnected,
                                });
                                return;
                            }
                            // Backoff before retry
                            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                        }
                    }
                }
            }
        }
    }
}
