use crate::transport::DeviceTransport;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

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
        raw.min(self.max_delay_secs)
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
            let _ = event_tx
                .send(DeviceSessionEvent::StateChanged(if attempt == 0 {
                    SessionState::Connecting
                } else {
                    SessionState::Reconnecting { attempt }
                }))
                .await;

            match transport.open().await {
                Ok(()) => {
                    attempt = 0;
                    let _ = event_tx
                        .send(DeviceSessionEvent::StateChanged(SessionState::Connected))
                        .await;

                    // --- Read loop ---
                    loop {
                        tokio::select! {
                            _ = cancel.cancelled() => {
                                transport.close().await;
                                let _ = event_tx.send(DeviceSessionEvent::StateChanged(SessionState::Disconnected)).await;
                                return;
                            }
                            result = transport.read_frame() => {
                                match result {
                                    Ok(Some(frame)) => {
                                        let _ = event_tx.send(DeviceSessionEvent::FrameReceived(frame)).await;
                                    }
                                    Ok(None) => {
                                        // No data, continue
                                    }
                                    Err(err) => {
                                        let msg: String = err.to_string();
                                        let _ = event_tx.send(DeviceSessionEvent::TransportError(msg)).await;
                                        transport.close().await;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                Err(err) => {
                    let msg: String = err.to_string();
                    let _ = event_tx
                        .send(DeviceSessionEvent::TransportError(msg))
                        .await;
                }
            }

            // --- Retry logic ---
            if !policy.enabled {
                let _ = event_tx
                    .send(DeviceSessionEvent::StateChanged(SessionState::Disconnected))
                    .await;
                break;
            }

            attempt += 1;
            let delay = policy.delay_for_attempt(attempt);

            let _ = event_tx
                .send(DeviceSessionEvent::StateChanged(
                    SessionState::WaitingRetry {
                        attempt,
                        delay_secs: delay,
                    },
                ))
                .await;

            if delay > 0.0 {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        let _ = event_tx.send(DeviceSessionEvent::StateChanged(SessionState::Disconnected)).await;
                        return;
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_secs_f64(delay)) => {}
                }
            }
        }

        let _ = event_tx
            .send(DeviceSessionEvent::StateChanged(
                SessionState::Disconnected,
            ))
            .await;
    }
}
