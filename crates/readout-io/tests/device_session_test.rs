use readout_io::device_session::*;
use readout_io::transport::*;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

async fn recv_session_event(
    event_rx: &mut mpsc::Receiver<DeviceSessionEvent>,
) -> DeviceSessionEvent {
    tokio::time::timeout(Duration::from_secs(2), event_rx.recv())
        .await
        .expect("timed out waiting for device session event")
        .expect("device session event channel closed")
}

async fn await_session(handle: JoinHandle<()>) {
    tokio::time::timeout(Duration::from_secs(2), handle)
        .await
        .expect("device session did not stop")
        .expect("device session task panicked");
}

// Mock transport that fails N times then succeeds
struct MockTransport {
    fail_count: AtomicU32,
    max_failures: u32,
    frames_before_disconnect: u32,
    frame_count: AtomicU32,
}

impl MockTransport {
    fn new(max_failures: u32, frames_before_disconnect: u32) -> Self {
        Self {
            fail_count: AtomicU32::new(0),
            max_failures,
            frames_before_disconnect,
            frame_count: AtomicU32::new(0),
        }
    }

    fn always_ok(frames_before_disconnect: u32) -> Self {
        Self::new(0, frames_before_disconnect)
    }
}

impl DeviceTransport for MockTransport {
    async fn open(&mut self) -> Result<(), TransportError> {
        let count = self.fail_count.fetch_add(1, Ordering::SeqCst);
        if count < self.max_failures {
            return Err(TransportError::ConnectionLost("mock failure".into()));
        }
        self.frame_count.store(0, Ordering::SeqCst);
        Ok(())
    }

    async fn close(&mut self) {}

    async fn read_frame(&mut self) -> Result<Option<String>, TransportError> {
        let count = self.frame_count.fetch_add(1, Ordering::SeqCst);
        if count >= self.frames_before_disconnect {
            return Err(TransportError::ConnectionLost("disconnect".into()));
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        Ok(Some("test_frame".into()))
    }
}

#[tokio::test]
async fn connects_successfully() {
    let transport = MockTransport::always_ok(100);
    let (event_tx, mut event_rx) = mpsc::channel(32);
    let cancel = CancellationToken::new();
    let policy = ReconnectPolicy::default();

    let cancel_clone = cancel.clone();
    let handle = tokio::spawn(async move {
        DeviceSession::run(transport, policy, event_tx, cancel_clone).await;
    });

    let mut saw_connected = false;
    for _ in 0..10 {
        let event = recv_session_event(&mut event_rx).await;
        if matches!(
            event,
            DeviceSessionEvent::StateChanged(SessionState::Connected)
        ) {
            saw_connected = true;
            break;
        }
    }
    assert!(saw_connected);

    cancel.cancel();
    await_session(handle).await;
}

#[tokio::test]
async fn reconnects_after_failure() {
    let transport = MockTransport::new(2, 5);
    let (event_tx, mut event_rx) = mpsc::channel(64);
    let cancel = CancellationToken::new();
    let policy = ReconnectPolicy {
        enabled: true,
        initial_delay_secs: 0.01,
        max_delay_secs: 0.05,
        multiplier: 2.0,
    };

    let cancel_clone = cancel.clone();
    let handle = tokio::spawn(async move {
        DeviceSession::run(transport, policy, event_tx, cancel_clone).await;
    });

    let mut saw_connected = false;
    let mut saw_reconnecting = false;
    for _ in 0..30 {
        let event = recv_session_event(&mut event_rx).await;
        match &event {
            DeviceSessionEvent::StateChanged(SessionState::Reconnecting { .. }) => {
                saw_reconnecting = true;
            }
            DeviceSessionEvent::StateChanged(SessionState::Connected) => {
                saw_connected = true;
                break;
            }
            _ => {}
        }
    }
    assert!(saw_reconnecting);
    assert!(saw_connected);

    cancel.cancel();
    await_session(handle).await;
}

#[tokio::test]
async fn cancellation_stops_loop() {
    let transport = MockTransport::always_ok(10000);
    let (event_tx, _event_rx) = mpsc::channel(32);
    let cancel = CancellationToken::new();
    let policy = ReconnectPolicy::default();

    let cancel_clone = cancel.clone();
    let handle = tokio::spawn(async move {
        DeviceSession::run(transport, policy, event_tx, cancel_clone).await;
    });

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    cancel.cancel();
    await_session(handle).await;
}

#[tokio::test]
async fn backoff_delay_sequence() {
    let policy = ReconnectPolicy {
        enabled: true,
        initial_delay_secs: 0.5,
        max_delay_secs: 5.0,
        multiplier: 2.0,
    };

    assert_eq!(policy.delay_for_attempt(0), 0.0);
    assert!((policy.delay_for_attempt(1) - 0.5).abs() < 0.001);
    assert!((policy.delay_for_attempt(2) - 1.0).abs() < 0.001);
    assert!((policy.delay_for_attempt(3) - 2.0).abs() < 0.001);
    assert!((policy.delay_for_attempt(4) - 4.0).abs() < 0.001);
    assert!((policy.delay_for_attempt(5) - 5.0).abs() < 0.001); // capped
    assert!((policy.delay_for_attempt(10) - 5.0).abs() < 0.001); // still capped
}

#[tokio::test]
async fn emits_frames() {
    let transport = MockTransport::always_ok(5);
    let (event_tx, mut event_rx) = mpsc::channel(32);
    let cancel = CancellationToken::new();
    let policy = ReconnectPolicy::default();

    let cancel_clone = cancel.clone();
    let handle = tokio::spawn(async move {
        DeviceSession::run(transport, policy, event_tx, cancel_clone).await;
    });

    let mut frame_count = 0;
    for _ in 0..20 {
        let event = recv_session_event(&mut event_rx).await;
        if matches!(event, DeviceSessionEvent::FrameReceived(_)) {
            frame_count += 1;
            if frame_count >= 3 {
                break;
            }
        }
    }
    assert!(frame_count >= 3);

    cancel.cancel();
    await_session(handle).await;
}
