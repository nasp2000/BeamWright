//! Background work with progress and cancellation.
//!
//! Work runs on a worker thread and returns one value through a channel. The
//! interface polls it every frame and can stop waiting; the worker checks the
//! cancellation flag before publishing its result, so a cancelled import never
//! replaces the open document.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

/// A piece of work running on a worker thread.
#[derive(Debug)]
pub struct BackgroundTask<T> {
    label: String,
    cancel: Arc<AtomicBool>,
    receiver: Receiver<T>,
}

/// State of a background task at one point in time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskPoll<T> {
    /// Still running.
    Pending,
    /// Finished successfully with a result.
    Finished(T),
    /// Cancelled before publishing a result.
    Cancelled,
}

impl<T: Send + 'static> BackgroundTask<T> {
    /// Start `work` on a new thread.
    ///
    /// `work` receives the cancellation flag and returns `None` when it decides
    /// not to publish a result (either because it noticed the flag or because it
    /// had nothing to produce).
    pub fn spawn(
        label: impl Into<String>,
        work: impl FnOnce(&AtomicBool) -> Option<T> + Send + 'static,
    ) -> Self {
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&cancel);
        let (sender, receiver) = channel();
        std::thread::spawn(move || {
            if let Some(result) = work(&flag) {
                let _ = sender.send(result);
            }
        });
        Self {
            label: label.into(),
            cancel,
            receiver,
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    /// Ask the worker to stop; it will not publish a result after seeing this.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// Check for a result without blocking.
    pub fn poll(&self) -> TaskPoll<T> {
        match self.receiver.try_recv() {
            Ok(value) => TaskPoll::Finished(value),
            Err(TryRecvError::Empty) => TaskPoll::Pending,
            Err(TryRecvError::Disconnected) => TaskPoll::Cancelled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BackgroundTask, TaskPoll};
    use std::sync::atomic::AtomicBool;
    use std::time::{Duration, Instant};

    fn wait_for(task: &BackgroundTask<u32>, timeout: Duration) -> TaskPoll<u32> {
        let start = Instant::now();
        loop {
            match task.poll() {
                TaskPoll::Pending => {
                    assert!(start.elapsed() < timeout, "task did not finish");
                    std::thread::sleep(Duration::from_millis(5));
                }
                other => return other,
            }
        }
    }

    #[test]
    fn a_task_publishes_its_result() {
        let task = BackgroundTask::spawn("count", |_| Some(7));
        assert_eq!(
            wait_for(&task, Duration::from_secs(5)),
            TaskPoll::Finished(7)
        );
        assert_eq!(task.label(), "count");
    }

    #[test]
    fn a_cancelled_task_does_not_publish() {
        let task = BackgroundTask::spawn("slow", |cancel: &AtomicBool| {
            while !cancel.load(std::sync::atomic::Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(1));
            }
            None
        });
        task.cancel();
        assert!(task.is_cancelled());
        assert_eq!(wait_for(&task, Duration::from_secs(5)), TaskPoll::Cancelled);
    }

    #[test]
    fn work_may_decline_to_produce_a_value() {
        let task = BackgroundTask::spawn("nothing", |_| None);
        assert_eq!(wait_for(&task, Duration::from_secs(5)), TaskPoll::Cancelled);
    }
}
