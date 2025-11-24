use std::{
    sync::{
        atomic::{
            AtomicBool,
            Ordering,
        },
        Arc,
    },
    thread::JoinHandle,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CancellableTask {
    FrequencyAnalysis,
}

pub struct TaskHandle {
    cancel_token: Arc<AtomicBool>,
    join_handle: Option<JoinHandle<()>>,
}

impl TaskHandle {
    pub fn new(cancel_token: Arc<AtomicBool>, join_handle: JoinHandle<()>) -> Self {
        Self { cancel_token, join_handle: Some(join_handle) }
    }

    pub fn cancel(&self) {
        self.cancel_token.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel_token.load(Ordering::Relaxed)
    }

    pub fn is_finished(&self) -> bool {
        self.join_handle.as_ref().map(|h| h.is_finished()).unwrap_or(true)
    }
}
