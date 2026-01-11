// Metagraph storage module for persistent subnet/neuron/weight storage
// Uses RocksDB column families for efficient storage

use crate::{Result, StorageError};
use rocksdb::{ColumnFamilyDescriptor, Options, WriteBatch, DB};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

/// Column family names for metagraph data
const CF_SUBNETS: &str = "subnets";
const CF_NEURONS: &str = "neurons";
const CF_WEIGHTS: &str = "weights";
const CF_AI_TASKS: &str = "ai_tasks";
const CF_METADATA: &str = "metagraph_meta";

/// Subnet information (stored)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubnetData {
    pub id: u64,
    pub name: String,
    pub owner: [u8; 20],
    pub emission_rate: u128,
    pub created_at: u64,
    pub tempo: u16,
    pub max_neurons: u16,
    pub min_stake: u128,
    pub active: bool,
}

/// Neuron information (stored)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuronData {
    pub uid: u64,
    pub subnet_id: u64,
    pub hotkey: [u8; 20],
    pub coldkey: [u8; 20],
    pub stake: u128,
    pub trust: u32,    // Fixed point: value / 65535
    pub rank: u32,     // Fixed point
    pub incentive: u32,
    pub dividends: u32,
    pub emission: u128,
    pub last_update: u64,
    pub active: bool,
    pub endpoint: String,
}

/// Weight entry (stored)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightData {
    pub from_uid: u64,
    pub to_uid: u64,
    pub weight: u16,
    pub updated_at: u64,
}

/// AI Task (stored)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AITaskData {
    pub id: [u8; 32],
    pub model_hash: String,
    pub input_hash: [u8; 32],
    pub requester: [u8; 20],
    pub reward: u128,
    pub status: u8, // 0=Pending, 1=Processing, 2=Completed, 3=Failed
    pub worker: Option<[u8; 20]>,
    pub result_hash: Option<[u8; 32]>,
    pub created_at: u64,
    pub completed_at: Option<u64>,
}

/// Metagraph database for persistent storage
pub struct MetagraphDB {
    db: Arc<DB>,
}

impl MetagraphDB {
    /// Open a metagraph database at the given path
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        let cf_opts = Options::default();
        let cfs = vec![
            ColumnFamilyDescriptor::new(CF_SUBNETS, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_NEURONS, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_WEIGHTS, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_AI_TASKS, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_METADATA, cf_opts),
        ];

        let db = DB::open_cf_descriptors(&opts, path, cfs)
            .map_err(|e| StorageError::DatabaseError(e.to_string()))?;

        Ok(Self { db: Arc::new(db) })
    }

    /// Get inner DB reference
    pub fn inner_db(&self) -> Arc<DB> {
        self.db.clone()
    }

    // ==================== SUBNET OPERATIONS ====================

    /// Store a subnet
    pub fn store_subnet(&self, subnet: &SubnetData) -> Result<()> {
        let cf = self.db.cf_handle(CF_SUBNETS)
            .ok_or_else(|| StorageError::DatabaseError("Missing subnets CF".into()))?;

        let key = subnet.id.to_be_bytes();
        let value = bincode::serialize(subnet)
            .map_err(|e| StorageError::SerializationError(e.to_string()))?;

        self.db.put_cf(&cf, key, value)
            .map_err(|e| StorageError::DatabaseError(e.to_string()))
    }

    /// Get a subnet by ID
    pub fn get_subnet(&self, subnet_id: u64) -> Result<Option<SubnetData>> {
        let cf = self.db.cf_handle(CF_SUBNETS)
            .ok_or_else(|| StorageError::DatabaseError("Missing subnets CF".into()))?;

        let key = subnet_id.to_be_bytes();
        match self.db.get_cf(&cf, key) {
            Ok(Some(data)) => {
                let subnet: SubnetData = bincode::deserialize(&data)
                    .map_err(|e| StorageError::SerializationError(e.to_string()))?;
                Ok(Some(subnet))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(StorageError::DatabaseError(e.to_string())),
        }
    }

    /// Get all subnets
    pub fn get_all_subnets(&self) -> Result<Vec<SubnetData>> {
        let cf = self.db.cf_handle(CF_SUBNETS)
            .ok_or_else(|| StorageError::DatabaseError("Missing subnets CF".into()))?;

        let mut subnets = Vec::new();
        let iter = self.db.iterator_cf(&cf, rocksdb::IteratorMode::Start);

        for item in iter {
            let (_, value) = item.map_err(|e| StorageError::DatabaseError(e.to_string()))?;
            let subnet: SubnetData = bincode::deserialize(&value)
                .map_err(|e| StorageError::SerializationError(e.to_string()))?;
            subnets.push(subnet);
        }

        Ok(subnets)
    }

    /// Delete a subnet
    pub fn delete_subnet(&self, subnet_id: u64) -> Result<()> {
        let cf = self.db.cf_handle(CF_SUBNETS)
            .ok_or_else(|| StorageError::DatabaseError("Missing subnets CF".into()))?;

        let key = subnet_id.to_be_bytes();
        self.db.delete_cf(&cf, key)
            .map_err(|e| StorageError::DatabaseError(e.to_string()))
    }

    // ==================== NEURON OPERATIONS ====================

    /// Create neuron key: subnet_id (8 bytes) + uid (8 bytes)
    fn neuron_key(subnet_id: u64, uid: u64) -> [u8; 16] {
        let mut key = [0u8; 16];
        key[..8].copy_from_slice(&subnet_id.to_be_bytes());
        key[8..].copy_from_slice(&uid.to_be_bytes());
        key
    }

    /// Store a neuron
    pub fn store_neuron(&self, neuron: &NeuronData) -> Result<()> {
        let cf = self.db.cf_handle(CF_NEURONS)
            .ok_or_else(|| StorageError::DatabaseError("Missing neurons CF".into()))?;

        let key = Self::neuron_key(neuron.subnet_id, neuron.uid);
        let value = bincode::serialize(neuron)
            .map_err(|e| StorageError::SerializationError(e.to_string()))?;

        self.db.put_cf(&cf, key, value)
            .map_err(|e| StorageError::DatabaseError(e.to_string()))
    }

    /// Get a neuron by subnet_id and uid
    pub fn get_neuron(&self, subnet_id: u64, uid: u64) -> Result<Option<NeuronData>> {
        let cf = self.db.cf_handle(CF_NEURONS)
            .ok_or_else(|| StorageError::DatabaseError("Missing neurons CF".into()))?;

        let key = Self::neuron_key(subnet_id, uid);
        match self.db.get_cf(&cf, key) {
            Ok(Some(data)) => {
                let neuron: NeuronData = bincode::deserialize(&data)
                    .map_err(|e| StorageError::SerializationError(e.to_string()))?;
                Ok(Some(neuron))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(StorageError::DatabaseError(e.to_string())),
        }
    }

    /// Get all neurons in a subnet
    pub fn get_neurons_by_subnet(&self, subnet_id: u64) -> Result<Vec<NeuronData>> {
        let cf = self.db.cf_handle(CF_NEURONS)
            .ok_or_else(|| StorageError::DatabaseError("Missing neurons CF".into()))?;

        let prefix = subnet_id.to_be_bytes();
        let mut neurons = Vec::new();

        let iter = self.db.prefix_iterator_cf(&cf, prefix);
        for item in iter {
            let (key, value) = item.map_err(|e| StorageError::DatabaseError(e.to_string()))?;

            // Check if key still has our prefix
            if key.len() >= 8 && &key[..8] == &prefix[..] {
                let neuron: NeuronData = bincode::deserialize(&value)
                    .map_err(|e| StorageError::SerializationError(e.to_string()))?;
                neurons.push(neuron);
            } else {
                break;
            }
        }

        Ok(neurons)
    }

    /// Get neuron count in a subnet
    pub fn get_neuron_count(&self, subnet_id: u64) -> Result<usize> {
        Ok(self.get_neurons_by_subnet(subnet_id)?.len())
    }

    // ==================== WEIGHT OPERATIONS ====================

    /// Create weight key: subnet_id (8) + from_uid (8) + to_uid (8)
    fn weight_key(subnet_id: u64, from_uid: u64, to_uid: u64) -> [u8; 24] {
        let mut key = [0u8; 24];
        key[..8].copy_from_slice(&subnet_id.to_be_bytes());
        key[8..16].copy_from_slice(&from_uid.to_be_bytes());
        key[16..].copy_from_slice(&to_uid.to_be_bytes());
        key
    }

    /// Store weights batch
    pub fn store_weights(&self, subnet_id: u64, from_uid: u64, weights: &[(u64, u16)]) -> Result<()> {
        let cf = self.db.cf_handle(CF_WEIGHTS)
            .ok_or_else(|| StorageError::DatabaseError("Missing weights CF".into()))?;

        let mut batch = WriteBatch::default();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        for (to_uid, weight) in weights {
            let key = Self::weight_key(subnet_id, from_uid, *to_uid);
            let data = WeightData {
                from_uid,
                to_uid: *to_uid,
                weight: *weight,
                updated_at: now,
            };
            let value = bincode::serialize(&data)
                .map_err(|e| StorageError::SerializationError(e.to_string()))?;
            batch.put_cf(&cf, key, value);
        }

        self.db.write(batch)
            .map_err(|e| StorageError::DatabaseError(e.to_string()))
    }

    /// Get weights from a neuron
    pub fn get_weights(&self, subnet_id: u64, from_uid: u64) -> Result<Vec<WeightData>> {
        let cf = self.db.cf_handle(CF_WEIGHTS)
            .ok_or_else(|| StorageError::DatabaseError("Missing weights CF".into()))?;

        let mut prefix = [0u8; 16];
        prefix[..8].copy_from_slice(&subnet_id.to_be_bytes());
        prefix[8..].copy_from_slice(&from_uid.to_be_bytes());

        let mut weights = Vec::new();
        let iter = self.db.prefix_iterator_cf(&cf, prefix);

        for item in iter {
            let (key, value) = item.map_err(|e| StorageError::DatabaseError(e.to_string()))?;

            if key.len() >= 16 && &key[..16] == &prefix[..] {
                let weight: WeightData = bincode::deserialize(&value)
                    .map_err(|e| StorageError::SerializationError(e.to_string()))?;
                weights.push(weight);
            } else {
                break;
            }
        }

        Ok(weights)
    }

    // ==================== AI TASK OPERATIONS ====================

    /// Store an AI task
    pub fn store_ai_task(&self, task: &AITaskData) -> Result<()> {
        let cf = self.db.cf_handle(CF_AI_TASKS)
            .ok_or_else(|| StorageError::DatabaseError("Missing ai_tasks CF".into()))?;

        let value = bincode::serialize(task)
            .map_err(|e| StorageError::SerializationError(e.to_string()))?;

        self.db.put_cf(&cf, task.id, value)
            .map_err(|e| StorageError::DatabaseError(e.to_string()))
    }

    /// Get an AI task by ID
    pub fn get_ai_task(&self, task_id: &[u8; 32]) -> Result<Option<AITaskData>> {
        let cf = self.db.cf_handle(CF_AI_TASKS)
            .ok_or_else(|| StorageError::DatabaseError("Missing ai_tasks CF".into()))?;

        match self.db.get_cf(&cf, task_id) {
            Ok(Some(data)) => {
                let task: AITaskData = bincode::deserialize(&data)
                    .map_err(|e| StorageError::SerializationError(e.to_string()))?;
                Ok(Some(task))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(StorageError::DatabaseError(e.to_string())),
        }
    }

    /// Get pending AI tasks
    pub fn get_pending_ai_tasks(&self) -> Result<Vec<AITaskData>> {
        let cf = self.db.cf_handle(CF_AI_TASKS)
            .ok_or_else(|| StorageError::DatabaseError("Missing ai_tasks CF".into()))?;

        let mut tasks = Vec::new();
        let iter = self.db.iterator_cf(&cf, rocksdb::IteratorMode::Start);

        for item in iter {
            let (_, value) = item.map_err(|e| StorageError::DatabaseError(e.to_string()))?;
            let task: AITaskData = bincode::deserialize(&value)
                .map_err(|e| StorageError::SerializationError(e.to_string()))?;

            if task.status == 0 { // Pending
                tasks.push(task);
            }
        }

        Ok(tasks)
    }

    // ==================== METADATA OPERATIONS ====================

    /// Store subnet count
    pub fn set_subnet_count(&self, count: u64) -> Result<()> {
        let cf = self.db.cf_handle(CF_METADATA)
            .ok_or_else(|| StorageError::DatabaseError("Missing metadata CF".into()))?;

        self.db.put_cf(&cf, b"subnet_count", count.to_be_bytes())
            .map_err(|e| StorageError::DatabaseError(e.to_string()))
    }

    /// Get subnet count
    pub fn get_subnet_count(&self) -> Result<u64> {
        let cf = self.db.cf_handle(CF_METADATA)
            .ok_or_else(|| StorageError::DatabaseError("Missing metadata CF".into()))?;

        match self.db.get_cf(&cf, b"subnet_count") {
            Ok(Some(data)) => {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(&data);
                Ok(u64::from_be_bytes(bytes))
            }
            Ok(None) => Ok(0),
            Err(e) => Err(StorageError::DatabaseError(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_subnet(id: u64) -> SubnetData {
        SubnetData {
            id,
            name: format!("Subnet {}", id),
            owner: [0u8; 20],
            emission_rate: 1_000_000,
            created_at: 123456,
            tempo: 100,
            max_neurons: 256,
            min_stake: 1000,
            active: true,
        }
    }

    fn create_test_neuron(subnet_id: u64, uid: u64) -> NeuronData {
        NeuronData {
            uid,
            subnet_id,
            hotkey: [uid as u8; 20],
            coldkey: [0u8; 20],
            stake: 1000 * uid as u128,
            trust: 50000,
            rank: 30000,
            incentive: 40000,
            dividends: 20000,
            emission: 100,
            last_update: 123456,
            active: true,
            endpoint: format!("http://neuron-{}.example.com", uid),
        }
    }

    #[test]
    fn test_subnet_storage() {
        let temp_dir = TempDir::new().unwrap();
        let db = MetagraphDB::open(temp_dir.path()).unwrap();

        let subnet = create_test_subnet(1);
        db.store_subnet(&subnet).unwrap();

        let retrieved = db.get_subnet(1).unwrap().unwrap();
        assert_eq!(retrieved.id, 1);
        assert_eq!(retrieved.name, "Subnet 1");
    }

    #[test]
    fn test_neuron_storage() {
        let temp_dir = TempDir::new().unwrap();
        let db = MetagraphDB::open(temp_dir.path()).unwrap();

        let neuron = create_test_neuron(1, 5);
        db.store_neuron(&neuron).unwrap();

        let retrieved = db.get_neuron(1, 5).unwrap().unwrap();
        assert_eq!(retrieved.uid, 5);
        assert_eq!(retrieved.subnet_id, 1);
    }

    #[test]
    fn test_get_neurons_by_subnet() {
        let temp_dir = TempDir::new().unwrap();
        let db = MetagraphDB::open(temp_dir.path()).unwrap();

        // Add neurons to subnet 1
        for uid in 0..5 {
            db.store_neuron(&create_test_neuron(1, uid)).unwrap();
        }

        // Add neurons to subnet 2
        for uid in 0..3 {
            db.store_neuron(&create_test_neuron(2, uid)).unwrap();
        }

        let subnet1_neurons = db.get_neurons_by_subnet(1).unwrap();
        let subnet2_neurons = db.get_neurons_by_subnet(2).unwrap();

        assert_eq!(subnet1_neurons.len(), 5);
        assert_eq!(subnet2_neurons.len(), 3);
    }

    #[test]
    fn test_weight_storage() {
        let temp_dir = TempDir::new().unwrap();
        let db = MetagraphDB::open(temp_dir.path()).unwrap();

        let weights = vec![(1u64, 100u16), (2, 200), (3, 150)];
        db.store_weights(1, 0, &weights).unwrap();

        let retrieved = db.get_weights(1, 0).unwrap();
        assert_eq!(retrieved.len(), 3);
    }
}
