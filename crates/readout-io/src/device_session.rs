use crate::transport::DeviceTransport;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Send an event, returning false if the channel is closed (receiver dropped).
async fn send_event(tx: &mpsc::Sender<DeviceSessionEvent>, event: DeviceSessionEvent) -> bool {
    tx.send(event).await.is_ok()
}

// --- ReconnectPolicy ---

#[derive(Debug, Clone)]
pub struct ReconnectPolicy {
    pub enabled: bool,
    pub initial_delay_secs: f64,
    pub max_delay_secs: f64,
    pub multiplier: f64,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            initial_delay_secs: 0.5,
            max_delay_secs: 5.0,
            multiplier: 2.0,
        }
    }
}

impl ReconnectPolicy {
    pub fn delay_for_attempt(&self, attempt: u32) -> f64 {
        if attempt == 0 {
            return 0.0;
        }
        let raw = self.initial_delay_secs * self.multiplier.powi(attempt as i32 - 1);
        raw.min(self.max_delay_secs).clamp(0.0, 86_400.0)
    }
}

// --- Session State & Events ---

#[derive(Debug, Clone, PartialEq)]
pub enum SessionState {
    Idle,
    Connecting,
    Connected,
    Reconnecting { attempt: u32 },
    WaitingRetry { attempt: u32, delay_secs: f64 },
    Disconnected,
}

#[derive(Debug, Clone)]
pub enum DeviceSessionEvent {
    StateChanged(SessionState),
    FrameReceived(String),
    TransportError(String),
}

// --- DeviceSession ---

pub struct DeviceSession;

impl DeviceSession {
    pub async fn run<T: DeviceTransport>(
        mut transport: T,
        policy: ReconnectPolicy,
        event_tx: mpsc::Sender<DeviceSessionEvent>,
        cancel: CancellationToken,
    ) {
        let mut attempt: u32 = 0;

        loop {
            if cancel.is_cancelled() {
                break;
            }

            // --- Connecting ---
            if !send_event(&event_tx, DeviceSessionEvent::StateChanged(if attempt == 0 {
                SessionState::Connecting
            } else {
                SessionState::Reconnecting { attempt }
            })).await {
                return; // channel closed
            }

            match transport.open().await {
                Ok(()) => {
                    attempt = 0;
                    if !send_event(&event_tx, DeviceSessionEvent::StateChanged(SessionState::Connected)).await {
                        transport.close().await;
                        return;
                    }

                    // --- Read loop ---
                    loop {
                        tokio::select! {
                            _ = cancel.cancelled() => {
                                transport.close().await;
                                let _ = send_event(&event_tx, DeviceSessionEvent::StateChanged(SessionState::Disconnected)).await;
                                return;
                            }
                            result = transport.read_frame() => {
                                match result {
                                    Ok(Some(frame)) => {
                                        if !send_event(&event_tx, DeviceSessionEvent::FrameReceived(frame)).await {
                                            transport.close().await;
                                            return;
                                        }
                                    }
                                    Ok(None) => {
                                        // No data available — short sleep to prevent busy-spin.
                                        // EOF from transport is also Ok(None), so this avoids
                                        // a tight loop on a closed connection.
                                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                                    }
                                    Err(err) => {
                                        let _ = send_event(&event_tx, DeviceSessionEvent::TransportError(err.to_string())).await;
                                        transport.close().await;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                Err(err) => {
                    if !send_event(&event_tx, DeviceSessionEvent::TransportError(err.to_string())).await {
                        return;
                    }
                }
            }

            // --- Retry logic ---
            if !policy.enabled {
                let _ = send_event(&event_tx, DeviceSessionEvent::StateChanged(SessionState::Disconnected)).await;
                break;
            }

            attempt += 1;
            let delay = policy.delay_for_attempt(attempt);

            if !send_event(&event_tx, DeviceSessionEvent::StateChanged(
                SessionState::WaitingRetry {
                    attempt,
                    delay_secs: delay,
                },
            )).await {
                return;
            }

            if delay > 0.0 {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        let _ = send_event(&event_tx, DeviceSessionEvent::StateChanged(SessionState::Disconnected)).await;
                        return;
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_secs_f64(delay)) => {}
                }
            }
        }

        let _ = send_event(&event_tx, DeviceSessionEvent::StateChanged(SessionState::Disconnected)).await;
    }
}
