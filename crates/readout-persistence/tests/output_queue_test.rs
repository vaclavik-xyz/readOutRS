use readout_persistence::output_queue::*;
use tokio::sync::mpsc;

#[tokio::test]
async fn queue_accepts_within_capacity() {
    let (tx, mut rx) = mpsc::channel::<String>(8);
    let queue = OutputWriteQueue::new(tx, 8);
    assert!(queue.try_send("hello".into()).is_ok());
    let msg = rx.recv().await.unwrap();
    assert_eq!(msg, "hello");
}

#[tokio::test]
async fn queue_drops_when_full() {
    let (tx, _rx) = mpsc::channel::<String>(2);
    let queue = OutputWriteQueue::new(tx, 2);
    assert!(queue.try_send("one".into()).is_ok());
    assert!(queue.try_send("two".into()).is_ok());
    // Third should report dropped
    let result = queue.try_send("three".into());
    assert!(result.is_err());
}
