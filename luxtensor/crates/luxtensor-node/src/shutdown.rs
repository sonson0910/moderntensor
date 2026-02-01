//! Graceful Shutdown Handler
//!
//! Handles clean shutdown of blockchain node:
//! - Saves mempool state to disk
//! - Closes database connections
//! - Disconnects from peers gracefully
//! - Flushes metrics

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::broadcast;
use tracing::info;

/// Shutdown signal sender/receiver
pub type ShutdownSender = broadcast::Sender<()>;
pub type ShutdownReceiver = broadcast::Receiver<()>;

/// Graceful shutdown coordinator
pub struct ShutdownHandler {
    /// Flag indicating shutdown is in progress
    shutdown_flag: Arc<AtomicBool>,
    /// Broadcast channel for shutdown signal
    sender: ShutdownSender,
}

impl ShutdownHandler {
    /// Create new shutdown handler
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1);
        Self {
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            sender,
        }
    }

    /// Get a receiver for shutdown signals
    pub fn subscribe(&self) -> ShutdownReceiver {
        self.sender.subscribe()
    }

    /// Check if shutdown is in progress
    pub fn is_shutting_down(&self) -> bool {
        self.shutdown_flag.load(Ordering::SeqCst)
    }

    /// Get shutdown flag for sharing
    pub fn shutdown_flag(&self) -> Arc<AtomicBool> {
        self.shutdown_flag.clone()
    }

    /// Trigger shutdown
    pub fn shutdown(&self) {
        if self.shutdown_flag.swap(true, Ordering::SeqCst) {
            // Already shutting down
            return;
        }
        info!("🛑 Initiating graceful shutdown...");
        let _ = self.sender.send(());
    }
}

impl Default for ShutdownHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Shutdown task that runs cleanup operations
pub struct ShutdownTask<F>
where
    F: FnOnce() + Send + 'static,
{
    name: String,
    task: Option<F>,
}

impl<F> ShutdownTask<F>
where
    F: FnOnce() + Send + 'static,
{
    /// Create new shutdown task
    pub fn new(name: &str, task: F) -> Self {
        Self {
            name: name.to_string(),
            task: Some(task),
        }
    }

    /// Execute the shutdown task
    pub fn execute(&mut self) {
        if let Some(task) = self.task.take() {
            info!("⏳ Running shutdown task: {}", self.name);
            task();
            info!("✅ Completed shutdown task: {}", self.name);
        }
    }
}

/// Install signal handlers for graceful shutdown
/// Returns a future that completes when a shutdown signal is received
pub async fn wait_for_shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        let mut sigint = signal(SignalKind::interrupt()).expect("Failed to install SIGINT handler");
        let mut sigterm = signal(SignalKind::terminate()).expect("Failed to install SIGTERM handler");

        tokio::select! {
            _ = sigint.recv() => {
                info!("📥 Received SIGINT, initiating shutdown...");
            }
            _ = sigterm.recv() => {
                info!("📥 Received SIGTERM, initiating shutdown...");
            }
        }
    }

    #[cfg(windows)]
    {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
        info!("📥 Received Ctrl+C, initiating shutdown...");
    }
}

/// Perform graceful shutdown with callbacks
pub async fn graceful_shutdown<F>(
    handler: &ShutdownHandler,
    cleanup: F,
) where
    F: FnOnce(),
{
    handler.shutdown();

    info!("🔄 Running cleanup tasks...");
    cleanup();

    info!("✅ Graceful shutdown complete");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shutdown_handler_creation() {
        let handler = ShutdownHandler::new();
        assert!(!handler.is_shutting_down());
    }

    #[test]
    fn test_shutdown_flag() {
        let handler = ShutdownHandler::new();
        handler.shutdown();
        assert!(handler.is_shutting_down());
    }

    #[test]
    fn test_shutdown_task() {
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;

        let executed = Arc::new(AtomicBool::new(false));
        let executed_clone = executed.clone();

        let mut task = ShutdownTask::new("test_task", move || {
            executed_clone.store(true, Ordering::SeqCst);
        });

        assert!(!executed.load(Ordering::SeqCst));
        task.execute();
        assert!(executed.load(Ordering::SeqCst));
    }
}
