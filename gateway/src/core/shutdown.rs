//! Graceful Shutdown Coordinator
//!
//! Manages coordinated shutdown of all gateway components:
//! - Signal handling (SIGTERM, SIGINT)
//! - WebSocket connection draining
//! - Background task termination
//! - Resource cleanup

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::info;

/// Global shutdown flag shared across all components
pub struct ShutdownCoordinator {
    /// Atomic flag indicating shutdown has been initiated
    shutdown_requested: Arc<AtomicBool>,
    /// Timeout for graceful shutdown before forced termination
    shutdown_timeout: Duration,
}

impl ShutdownCoordinator {
    /// Create a new shutdown coordinator with default timeout (30 seconds)
    pub fn new() -> Self {
        Self {
            shutdown_requested: Arc::new(AtomicBool::new(false)),
            shutdown_timeout: Duration::from_secs(30),
        }
    }

    /// Create with custom timeout
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            shutdown_requested: Arc::new(AtomicBool::new(false)),
            shutdown_timeout: timeout,
        }
    }

    /// Get a cloned reference to the shutdown flag for use in background tasks
    pub fn flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.shutdown_requested)
    }

    /// Check if shutdown has been requested
    pub fn is_shutdown_requested(&self) -> bool {
        self.shutdown_requested.load(Ordering::SeqCst)
    }

    /// Request shutdown (safe to call multiple times)
    pub fn request_shutdown(&self) {
        self.shutdown_requested.store(true, Ordering::SeqCst);
    }

    /// Wait for shutdown signal (SIGINT or SIGTERM)
    pub async fn wait_for_signal(&self) {
        use tokio::signal;

        let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to setup SIGTERM handler");
        let mut sigint = signal::unix::signal(signal::unix::SignalKind::interrupt())
            .expect("Failed to setup SIGINT handler");

        tokio::select! {
            _ = sigterm.recv() => {
                info!("Received SIGTERM signal");
            }
            _ = sigint.recv() => {
                info!("Received SIGINT signal (Ctrl+C)");
            }
            _ = signal::ctrl_c() => {
                info!("Received Ctrl+C signal");
            }
        }

        self.request_shutdown();
    }

    /// Execute graceful shutdown sequence
    ///
    /// This coordinates:
    /// 1. Signal WebSocket manager to stop (via shutdown flag)
    /// 2. Wait briefly for WebSocket connections to drain
    /// 3. Stop the arbitrage scanner's next cycle
    /// 4. Stop the price indexer's next cycle
    /// 5. Drop storage backend (closes connections)
    pub async fn shutdown_gracefully(
        &self,
        _ws_manager: Option<Arc<crate::transport::websocket::WebSocketManager>>,
        arbitrage_scanner: Option<Arc<crate::core::arbitrage::ArbitrageScanner>>,
        price_index: Option<Arc<crate::core::price_index::PriceIndex>>,
        _storage: Option<Arc<dyn crate::core::storage::StorageBackend>>,
    ) {
        info!("Starting graceful shutdown sequence");

        // Phase 1: Signal shutdown — the shutdown flag is already set,
        // so the WebSocket manager will stop accepting new connections
        // on its next iteration.
        if _ws_manager.is_some() {
            info!("Shutdown flag set; WebSocket manager will stop accepting connections");
        }

        // Phase 2: Allow a brief drain period for WebSocket connections
        let drain_timeout = self.shutdown_timeout.div_f32(4.0);
        info!("Allowing {:?} for WebSocket connections to drain", drain_timeout);
        sleep(drain_timeout.min(Duration::from_secs(5))).await;

        // Phase 3: Stop arbitrage scanner
        if let Some(_scanner) = arbitrage_scanner {
            info!("Stopping arbitrage scanner");
            // Scanner will check shutdown flag in its main loop
            sleep(Duration::from_millis(500)).await;
        }

        // Phase 4: Stop price indexer
        if let Some(_indexer) = price_index {
            info!("Stopping price indexer");
            // Indexer will check shutdown flag in its main loop
            sleep(Duration::from_millis(500)).await;
        }

        // Phase 5: Storage backend cleanup — dropping the Arc will close
        // the connection when all references are released.
        if _storage.is_some() {
            info!("Storage backend will be cleaned up on drop");
        }

        info!("Graceful shutdown completed");
    }
}

impl Default for ShutdownCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function to create and run shutdown handler in background
pub fn start_shutdown_handler(coordinator: Arc<ShutdownCoordinator>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        coordinator.wait_for_signal().await;
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shutdown_flag_initialization() {
        let coordinator = ShutdownCoordinator::new();
        assert!(!coordinator.is_shutdown_requested());
    }

    #[test]
    fn test_shutdown_request() {
        let coordinator = ShutdownCoordinator::new();
        coordinator.request_shutdown();
        assert!(coordinator.is_shutdown_requested());
    }

    #[test]
    fn test_flag_cloning() {
        let coordinator = ShutdownCoordinator::new();
        let flag1 = coordinator.flag();
        let flag2 = coordinator.flag();

        flag1.store(true, Ordering::SeqCst);
        assert!(flag2.load(Ordering::SeqCst));
        assert!(coordinator.is_shutdown_requested());
    }

    #[test]
    fn test_custom_timeout() {
        let timeout = Duration::from_secs(60);
        let coordinator = ShutdownCoordinator::with_timeout(timeout);
        assert_eq!(coordinator.shutdown_timeout, timeout);
    }
}
