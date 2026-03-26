use tokio::sync::mpsc;

pub struct OutputWriteQueue<T> {
    sender: mpsc::Sender<T>,
    _capacity: usize,
}

impl<T> OutputWriteQueue<T> {
    pub fn new(sender: mpsc::Sender<T>, capacity: usize) -> Self {
        Self {
            sender,
            _capacity: capacity,
        }
    }

    pub fn try_send(&self, item: T) -> Result<(), QueueFullError> {
        self.sender.try_send(item).map_err(|e| match e {
            mpsc::error::TrySendError::Full(_) => QueueFullError::Full,
            mpsc::error::TrySendError::Closed(_) => QueueFullError::Closed,
        })
    }
}

#[derive(Debug)]
pub enum QueueFullError {
    Full,
    Closed,
}
