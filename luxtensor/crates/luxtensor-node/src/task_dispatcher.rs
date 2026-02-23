//! Task Dispatcher Module - Dispatches AI tasks to miners via P2P
//!
//! This module handles:
//! - Broadcasting pending AI tasks to miners via gossipsub
//! - Stake-weighted miner selection
//! - Task assignment and result tracking
//! - Timeout and retry mechanisms

use luxtensor_storage::MetagraphDB;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

// ============================================================
// Types
// ============================================================

/// AI Task Status for the dispatcher
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Assigned,
    Completed,
    Failed,
    Timeout,
}

/// Task assignment info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAssignment {
    pub task_id: [u8; 32],
    pub assigned_to: [u8; 20],
    pub assigned_at: u64,
    pub deadline: u64,
    pub retry_count: u32,
}

/// Task result from miner
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: [u8; 32],
    pub worker: [u8; 20],
    pub result_hash: [u8; 32],
    pub execution_time_ms: u64,
    pub proof: Option<Vec<u8>>,
}

/// Pending task with full details for dispatch
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingTask {
    pub task_id: [u8; 32],
    pub model_hash: String,
    pub input_hash: [u8; 32],
    pub reward: u128,
    pub created_at: u64,
}

/// Miner info for task dispatch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinerInfo {
    pub address: [u8; 20],
    pub stake: u128,
    pub capacity: u32,
    pub current_tasks: u32,
    pub success_rate: f64,
    pub avg_execution_time: u64,
    pub last_seen: u64,
}

/// P2P messages for task dispatch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskDispatchMessage {
    /// Broadcast new task to miners
    NewTask {
        task_id: [u8; 32],
        model_hash: String,
        input_hash: [u8; 32],
        reward: u128,
        deadline: u64,
    },
    /// Miner claims a task
    ClaimTask { task_id: [u8; 32], miner: [u8; 20], signature: Vec<u8> },
    /// Task assigned to miner (from dispatcher)
    TaskAssigned { task_id: [u8; 32], miner: [u8; 20], deadline: u64 },
    /// Miner submits result
    SubmitResult {
        task_id: [u8; 32],
        result_hash: [u8; 32],
        execution_time_ms: u64,
        proof: Option<Vec<u8>>,
    },
    /// Task completed confirmation
    TaskCompleted { task_id: [u8; 32], worker: [u8; 20], reward: u128 },
}

// ============================================================
// Task Dispatcher Configuration
// ============================================================

/// Configuration for task dispatch
#[derive(Debug, Clone)]
pub struct DispatcherConfig {
    /// Maximum time for a task to be completed (seconds)
    pub task_timeout: u64,
    /// Maximum retry attempts for failed tasks
    pub max_retries: u32,
    /// Minimum stake required to be eligible for tasks
    pub min_miner_stake: u128,
    /// Time between dispatch rounds (milliseconds)
    pub dispatch_interval_ms: u64,
    /// Maximum concurrent tasks per miner
    pub max_tasks_per_miner: u32,
}

impl Default for DispatcherConfig {
    fn default() -> Self {
        Self {
            task_timeout: 300, // 5 minutes
            max_retries: 3,
            min_miner_stake: 1000_000_000_000_000_000, // 1 token in wei
            dispatch_interval_ms: 1000,
            max_tasks_per_miner: 5,
        }
    }
}

// ============================================================
// Task Dispatcher
// ============================================================

/// Task Dispatcher handles distributing AI tasks to miners
pub struct TaskDispatcher {
    config: DispatcherConfig,
    /// MetagraphDB for neuron reputation lookups during task routing
    db: Arc<MetagraphDB>,
    /// Cached reputation map: (last_refresh, hotkey → (trust, rank))
    /// Avoids re-querying MetagraphDB on every `get_available_miners()` call.
    reputation_cache: RwLock<(Instant, HashMap<[u8; 20], (u32, u32)>)>,
    /// Pending tasks waiting for assignment (stores full task info)
    pending_queue: Arc<RwLock<VecDeque<PendingTask>>>,
    /// Active task assignments
    assignments: Arc<RwLock<HashMap<[u8; 32], TaskAssignment>>>,
    /// Original task metadata preserved for re-queue on timeout
    task_metadata: Arc<RwLock<HashMap<[u8; 32], PendingTask>>>,
    /// Registered miners
    miners: Arc<RwLock<HashMap<[u8; 20], MinerInfo>>>,
    /// Completed results
    results: Arc<RwLock<HashMap<[u8; 32], TaskResult>>>,
}

impl TaskDispatcher {
    /// Create a new task dispatcher
    pub fn new(db: Arc<MetagraphDB>, config: DispatcherConfig) -> Self {
        Self {
            config,
            db,
            // Force first refresh by setting timestamp far in the past
            reputation_cache: RwLock::new((
                Instant::now() - Duration::from_secs(120),
                HashMap::new(),
            )),
            pending_queue: Arc::new(RwLock::new(VecDeque::new())),
            assignments: Arc::new(RwLock::new(HashMap::new())),
            task_metadata: Arc::new(RwLock::new(HashMap::new())),
            miners: Arc::new(RwLock::new(HashMap::new())),
            results: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Enqueue a new task for dispatch with full details
    pub fn enqueue_task(&self, task: PendingTask) {
        let task_id = task.task_id;
        let mut queue = self.pending_queue.write();
        queue.push_back(task);
        info!("Task queued: 0x{}", hex::encode(&task_id[..8]));
    }

    /// Convenience method to enqueue with just ID (creates minimal task info)
    pub fn enqueue_task_id(
        &self,
        task_id: [u8; 32],
        model_hash: String,
        input_hash: [u8; 32],
        reward: u128,
    ) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.enqueue_task(PendingTask { task_id, model_hash, input_hash, reward, created_at: now });
    }

    /// Register or update a miner
    pub fn register_miner(&self, miner: MinerInfo) {
        let address = miner.address;
        let mut miners = self.miners.write();
        miners.insert(address, miner);
        debug!("Miner registered: 0x{}", hex::encode(&address[..8]));
    }

    /// TTL for the reputation cache (30 seconds).
    const REPUTATION_CACHE_TTL: Duration = Duration::from_secs(30);

    /// Build a hotkey→trust lookup from MetagraphDB neurons (root subnet 0).
    ///
    /// Returns a map from neuron hotkey `[u8; 20]` to `(trust, rank)` where
    /// both values are fixed-point u32 (divide by 65535 to normalize to [0, 1]).
    /// Gracefully returns empty map if MetagraphDB is unavailable.
    ///
    /// 🔧 FIX: Uses a 30-second TTL cache to avoid re-querying MetagraphDB
    /// on every `get_available_miners()` call (previously a performance bottleneck).
    fn build_reputation_map(&self) -> HashMap<[u8; 20], (u32, u32)> {
        // Fast path: return cached data if still valid
        {
            let cache = self.reputation_cache.read();
            if cache.0.elapsed() < Self::REPUTATION_CACHE_TTL {
                return cache.1.clone();
            }
        }

        // Slow path: refresh from MetagraphDB
        let map = match self.db.get_neurons_by_subnet(0) {
            Ok(neurons) => neurons
                .into_iter()
                .filter(|n| n.active)
                .map(|n| (n.hotkey, (n.trust, n.rank)))
                .collect(),
            Err(e) => {
                warn!("MetagraphDB reputation lookup failed, using base scores: {}", e);
                HashMap::new()
            }
        };

        // Update cache
        let mut cache = self.reputation_cache.write();
        *cache = (Instant::now(), map.clone());
        debug!("Reputation cache refreshed ({} entries)", map.len());
        map
    }

    /// Get available miners sorted by suitability (reputation-boosted scoring).
    ///
    /// Scoring formula: `(stake × success_rate × reputation_boost) / avg_time`
    /// where `reputation_boost = 1.0 + trust_normalized` (trust from MetagraphDB).
    /// Miners with no neuron data get boost = 1.0 (no penalty, just no bonus).
    pub fn get_available_miners(&self) -> Vec<MinerInfo> {
        let miners = self.miners.read();
        let mut available: Vec<_> = miners
            .values()
            .filter(|m| {
                m.stake >= self.config.min_miner_stake
                    && m.current_tasks < self.config.max_tasks_per_miner
            })
            .cloned()
            .collect();

        // Look up neuron reputation from MetagraphDB
        let reputation = self.build_reputation_map();

        // Sort by: stake * success_rate * reputation_boost / avg_execution_time
        available.sort_by(|a, b| {
            let boost_a = reputation
                .get(&a.address)
                .map(|(trust, _)| 1.0 + (*trust as f64 / 65535.0))
                .unwrap_or(1.0);
            let boost_b = reputation
                .get(&b.address)
                .map(|(trust, _)| 1.0 + (*trust as f64 / 65535.0))
                .unwrap_or(1.0);
            let score_a =
                (a.stake as f64 * a.success_rate * boost_a) / (a.avg_execution_time.max(1) as f64);
            let score_b =
                (b.stake as f64 * b.success_rate * boost_b) / (b.avg_execution_time.max(1) as f64);
            score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
        });

        available
    }

    /// Select best miner for a task (stake-weighted)
    pub fn select_miner(&self) -> Option<MinerInfo> {
        self.get_available_miners().into_iter().next()
    }

    /// Handle task claim from miner
    pub fn handle_claim(
        &self,
        task_id: [u8; 32],
        miner: [u8; 20],
        _signature: Vec<u8>,
    ) -> Result<TaskAssignment, String> {
        // Check if task is still pending and preserve its metadata for potential re-queue
        {
            let mut queue = self.pending_queue.write();
            if let Some(pos) = queue.iter().position(|task| task.task_id == task_id) {
                let original_task = queue.remove(pos).unwrap();
                // 🔧 FIX: Store original task metadata so timeout re-queue preserves it
                self.task_metadata.write().insert(task_id, original_task);
            } else {
                return Err("Task not in pending queue".into());
            }
        }

        // Check miner eligibility
        {
            let miners = self.miners.read();
            if let Some(info) = miners.get(&miner) {
                if info.stake < self.config.min_miner_stake {
                    return Err("Insufficient stake".into());
                }
                if info.current_tasks >= self.config.max_tasks_per_miner {
                    return Err("Miner at capacity".into());
                }
            } else {
                return Err("Miner not registered".into());
            }
        }

        // Create assignment
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(std::time::Duration::ZERO)
            .as_secs();

        let assignment = TaskAssignment {
            task_id,
            assigned_to: miner,
            assigned_at: now,
            deadline: now + self.config.task_timeout,
            retry_count: 0,
        };

        // Update miner task count
        {
            let mut miners = self.miners.write();
            if let Some(info) = miners.get_mut(&miner) {
                info.current_tasks += 1;
            }
        }

        // Store assignment
        {
            let mut assignments = self.assignments.write();
            assignments.insert(task_id, assignment.clone());
        }

        info!("Task assigned: 0x{} -> 0x{}", hex::encode(&task_id[..8]), hex::encode(&miner[..8]));

        Ok(assignment)
    }

    /// Handle result submission from miner
    pub fn handle_result(
        &self,
        task_id: [u8; 32],
        result_hash: [u8; 32],
        execution_time_ms: u64,
        proof: Option<Vec<u8>>,
    ) -> Result<(), String> {
        let assignment = {
            let assignments = self.assignments.read();
            assignments.get(&task_id).cloned()
        };

        let assignment = assignment.ok_or("Task not found or not assigned")?;

        // Store result
        let result = TaskResult {
            task_id,
            worker: assignment.assigned_to,
            result_hash,
            execution_time_ms,
            proof,
        };

        {
            let mut results = self.results.write();
            results.insert(task_id, result);
        }

        // Update miner stats
        {
            let mut miners = self.miners.write();
            if let Some(info) = miners.get_mut(&assignment.assigned_to) {
                info.current_tasks = info.current_tasks.saturating_sub(1);
                // Update average execution time
                info.avg_execution_time = (info.avg_execution_time + execution_time_ms) / 2;
            }
        }

        // Remove from assignments and clean up metadata
        {
            let mut assignments = self.assignments.write();
            assignments.remove(&task_id);
        }
        {
            // 🔧 FIX: Clean up preserved metadata on successful completion
            self.task_metadata.write().remove(&task_id);
        }

        info!("Task completed: 0x{}", hex::encode(&task_id[..8]));

        Ok(())
    }

    /// Check for timed out tasks and handle them
    pub fn check_timeouts(&self) -> Vec<[u8; 32]> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(std::time::Duration::ZERO)
            .as_secs();

        let timed_out: Vec<_> = {
            let assignments = self.assignments.read();
            assignments.iter().filter(|(_, a)| a.deadline < now).map(|(id, _)| *id).collect()
        };

        for task_id in &timed_out {
            self.handle_timeout(*task_id);
        }

        timed_out
    }

    /// Handle task timeout
    fn handle_timeout(&self, task_id: [u8; 32]) {
        let assignment = {
            let mut assignments = self.assignments.write();
            assignments.remove(&task_id)
        };

        if let Some(a) = assignment {
            // Update miner task count
            {
                let mut miners = self.miners.write();
                if let Some(info) = miners.get_mut(&a.assigned_to) {
                    info.current_tasks = info.current_tasks.saturating_sub(1);
                    info.success_rate *= 0.95; // Penalize success rate
                }
            }

            // Re-queue if retries remaining, using preserved original metadata
            if a.retry_count < self.config.max_retries {
                // 🔧 FIX: Retrieve original task metadata instead of creating empty defaults
                let requeue_task = self
                    .task_metadata
                    .write()
                    .remove(&task_id)
                    .unwrap_or_else(|| {
                        // Fallback: should never happen if metadata was stored on claim
                        warn!("Task metadata missing for 0x{}, using defaults", hex::encode(&task_id[..8]));
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        PendingTask {
                            task_id,
                            model_hash: String::new(),
                            input_hash: [0u8; 32],
                            reward: 0,
                            created_at: now,
                        }
                    });
                let mut queue = self.pending_queue.write();
                queue.push_front(requeue_task);
                warn!("Task timed out, re-queued: 0x{}", hex::encode(&task_id[..8]));
            } else {
                warn!(
                    "Task failed after {} retries: 0x{}",
                    self.config.max_retries,
                    hex::encode(&task_id[..8])
                );
            }
        }
    }

    /// Get pending task count
    pub fn pending_count(&self) -> usize {
        self.pending_queue.read().len()
    }

    /// Get active assignment count
    pub fn active_count(&self) -> usize {
        self.assignments.read().len()
    }

    /// Get completed result count
    pub fn completed_count(&self) -> usize {
        self.results.read().len()
    }

    /// Get result for a task
    pub fn get_result(&self, task_id: &[u8; 32]) -> Option<TaskResult> {
        self.results.read().get(task_id).cloned()
    }

    /// Get all pending tasks with full details
    pub fn get_pending_tasks(&self) -> Vec<PendingTask> {
        self.pending_queue.read().iter().cloned().collect()
    }

    /// Create P2P broadcast message for a new task
    pub fn create_broadcast_message(
        &self,
        task_id: [u8; 32],
        model_hash: String,
        input_hash: [u8; 32],
        reward: u128,
    ) -> TaskDispatchMessage {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(std::time::Duration::ZERO)
            .as_secs();

        TaskDispatchMessage::NewTask {
            task_id,
            model_hash,
            input_hash,
            reward,
            deadline: now + self.config.task_timeout,
        }
    }
}

// ============================================================
// Dispatch Loop (Background Service)
// ============================================================

/// Runs the dispatch loop as a background service
pub struct DispatchService {
    dispatcher: Arc<TaskDispatcher>,
    running: Arc<RwLock<bool>>,
    /// Optional command sender for P2P broadcast integration
    command_tx: Option<tokio::sync::mpsc::Sender<luxtensor_network::SwarmCommand>>,
}

impl DispatchService {
    /// Create a new dispatch service without P2P integration
    pub fn new(dispatcher: Arc<TaskDispatcher>) -> Self {
        Self { dispatcher, running: Arc::new(RwLock::new(false)), command_tx: None }
    }

    /// Create a new dispatch service with P2P broadcast capability
    pub fn with_p2p(
        dispatcher: Arc<TaskDispatcher>,
        command_tx: tokio::sync::mpsc::Sender<luxtensor_network::SwarmCommand>,
    ) -> Self {
        Self { dispatcher, running: Arc::new(RwLock::new(false)), command_tx: Some(command_tx) }
    }

    /// Start the dispatch service
    ///
    /// This should be called with a command sender for P2P integration:
    /// ```ignore
    /// let service = DispatchService::new(dispatcher, Some(swarm_command_tx));
    /// service.start().await;
    /// ```
    pub async fn start(&self) {
        *self.running.write() = true;
        info!("Task dispatch service started");

        let interval = Duration::from_millis(self.dispatcher.config.dispatch_interval_ms);

        while *self.running.read() {
            // Check for timeouts
            let _timed_out = self.dispatcher.check_timeouts();

            // Broadcast pending tasks to miners via P2P
            if let Some(ref tx) = self.command_tx {
                let pending_tasks = self.dispatcher.get_pending_tasks();
                for task in pending_tasks.iter().take(5) {
                    // Limit broadcast batch
                    let deadline = task.created_at + self.dispatcher.config.task_timeout;
                    use luxtensor_network::SwarmCommand;
                    if let Err(e) = tx.try_send(SwarmCommand::BroadcastTaskDispatch {
                        task_id: task.task_id,
                        model_hash: task.model_hash.clone(),
                        input_hash: task.input_hash,
                        reward: task.reward,
                        deadline,
                    }) {
                        warn!(
                            "Failed to broadcast task 0x{}: {}",
                            hex::encode(&task.task_id[..8]),
                            e
                        );
                    } else {
                        debug!("📡 Broadcast task 0x{} to miners", hex::encode(&task.task_id[..8]));
                    }
                }
            }

            tokio::time::sleep(interval).await;
        }

        info!("Task dispatch service stopped");
    }

    /// Stop the dispatch service
    pub fn stop(&self) {
        *self.running.write() = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use luxtensor_storage::NeuronData;
    use tempfile::TempDir;

    fn create_test_dispatcher() -> TaskDispatcher {
        let temp_dir = TempDir::new().unwrap();
        let db = MetagraphDB::open(temp_dir.path()).unwrap();
        TaskDispatcher::new(Arc::new(db), DispatcherConfig::default())
    }

    /// Create dispatcher with access to the underlying MetagraphDB for seeding test data
    fn create_test_dispatcher_with_db() -> (TaskDispatcher, Arc<MetagraphDB>) {
        let temp_dir = TempDir::new().unwrap();
        let db = Arc::new(MetagraphDB::open(temp_dir.path()).unwrap());
        let dispatcher = TaskDispatcher::new(db.clone(), DispatcherConfig::default());
        (dispatcher, db)
    }

    #[test]
    fn test_enqueue_task() {
        let dispatcher = create_test_dispatcher();
        let task_id = [1u8; 32];

        dispatcher.enqueue_task_id(task_id, "test_model".to_string(), [0u8; 32], 1000);
        assert_eq!(dispatcher.pending_count(), 1);
    }

    #[test]
    fn test_register_miner() {
        let dispatcher = create_test_dispatcher();
        let miner = MinerInfo {
            address: [1u8; 20],
            stake: 10_000_000_000_000_000_000,
            capacity: 10,
            current_tasks: 0,
            success_rate: 0.95,
            avg_execution_time: 1000,
            last_seen: 0,
        };

        dispatcher.register_miner(miner.clone());
        let available = dispatcher.get_available_miners();
        assert_eq!(available.len(), 1);
    }

    #[test]
    fn test_task_claim() {
        let dispatcher = create_test_dispatcher();
        let task_id = [1u8; 32];
        let miner_addr = [2u8; 20];

        // Register miner
        dispatcher.register_miner(MinerInfo {
            address: miner_addr,
            stake: 10_000_000_000_000_000_000,
            capacity: 10,
            current_tasks: 0,
            success_rate: 0.95,
            avg_execution_time: 1000,
            last_seen: 0,
        });

        // Enqueue task
        dispatcher.enqueue_task_id(task_id, "test_model".to_string(), [0u8; 32], 1000);

        // Claim task
        let result = dispatcher.handle_claim(task_id, miner_addr, vec![]);
        assert!(result.is_ok());
        assert_eq!(dispatcher.pending_count(), 0);
        assert_eq!(dispatcher.active_count(), 1);
    }

    #[test]
    fn test_submit_result() {
        let dispatcher = create_test_dispatcher();
        let task_id = [1u8; 32];
        let miner_addr = [2u8; 20];

        // Setup
        dispatcher.register_miner(MinerInfo {
            address: miner_addr,
            stake: 10_000_000_000_000_000_000,
            capacity: 10,
            current_tasks: 0,
            success_rate: 0.95,
            avg_execution_time: 1000,
            last_seen: 0,
        });
        dispatcher.enqueue_task_id(task_id, "test_model".to_string(), [0u8; 32], 1000);
        dispatcher.handle_claim(task_id, miner_addr, vec![]).unwrap();

        // Submit result
        let result = dispatcher.handle_result(task_id, [3u8; 32], 500, None);
        assert!(result.is_ok());
        assert_eq!(dispatcher.active_count(), 0);
        assert_eq!(dispatcher.completed_count(), 1);
    }

    #[test]
    fn test_reputation_boosted_scoring() {
        let (dispatcher, db) = create_test_dispatcher_with_db();

        let miner_a = [0xAA; 20]; // high trust
        let miner_b = [0xBB; 20]; // no trust data

        // Seed MetagraphDB with neuron data for miner_a (high trust)
        let neuron = NeuronData {
            uid: 1,
            subnet_id: 0,
            hotkey: miner_a,
            coldkey: [0u8; 20],
            stake: 0,
            trust: 60000,    // ~0.915 normalized (near max)
            rank: 50000,
            incentive: 40000,
            dividends: 0,
            emission: 0,
            last_update: 0,
            active: true,
            endpoint: String::new(),
        };
        db.store_neuron(&neuron).unwrap();

        // Register both miners with IDENTICAL stats
        for addr in [miner_a, miner_b] {
            dispatcher.register_miner(MinerInfo {
                address: addr,
                stake: 10_000_000_000_000_000_000,
                capacity: 10,
                current_tasks: 0,
                success_rate: 0.90,
                avg_execution_time: 1000,
                last_seen: 0,
            });
        }

        let available = dispatcher.get_available_miners();
        assert_eq!(available.len(), 2);

        // Miner A (high trust) should rank FIRST because of reputation boost
        assert_eq!(available[0].address, miner_a, "Trusted miner should rank first");
        assert_eq!(available[1].address, miner_b, "Untrusted miner should rank second");
    }
}
