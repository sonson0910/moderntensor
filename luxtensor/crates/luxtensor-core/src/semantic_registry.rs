//! World Semantic Index - Global Shared Vector Registry
//!
//! Provides a shared semantic knowledge base at the consensus level.
//! Features:
//! - Domain-specific namespacing (finance, gaming, social, etc.)
//! - Cross-contract vector sharing
//! - Semantic staking for storage allocation
//!
//! This is Phase 2 of LuxTensor's AI differentiation strategy.

use crate::hnsw::{HnswVectorStore, HnswError};
use std::collections::HashMap;
// SECURITY: Use parking_lot::RwLock (non-poisoning) for consistency with the rest of the codebase.
// std::sync::RwLock would brick the registry permanently if any write-holding thread panics.
use std::sync::Arc;
use parking_lot::RwLock;

/// Domain categories for semantic namespacing
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SemanticDomain {
    /// General purpose vectors (default)
    General = 0,
    /// DeFi: risk scores, asset embeddings, portfolio vectors
    Finance = 1,
    /// Gaming: player profiles, item embeddings, strategy vectors
    Gaming = 2,
    /// Social: content embeddings, user profiles, reputation
    Social = 3,
    /// Identity: biometric hashes, credential embeddings
    Identity = 4,
    /// Supply Chain: product embeddings, logistics vectors
    SupplyChain = 5,
    /// Healthcare: anonymized health embeddings
    Healthcare = 6,
    /// Custom domain (user-defined)
    Custom = 255,
}

impl From<u8> for SemanticDomain {
    fn from(value: u8) -> Self {
        match value {
            0 => SemanticDomain::General,
            1 => SemanticDomain::Finance,
            2 => SemanticDomain::Gaming,
            3 => SemanticDomain::Social,
            4 => SemanticDomain::Identity,
            5 => SemanticDomain::SupplyChain,
            6 => SemanticDomain::Healthcare,
            _ => SemanticDomain::Custom,
        }
    }
}

/// Metadata for a registered vector
#[derive(Clone, Debug)]
pub struct VectorMetadata {
    /// Original owner (contract address)
    pub owner: [u8; 20],
    /// Domain category
    pub domain: SemanticDomain,
    /// Registration timestamp (block number)
    pub registered_at: u64,
    /// Optional expiry (0 = permanent)
    pub expires_at: u64,
    /// Tags for filtering (hashed)
    pub tags: Vec<[u8; 32]>,
}

/// Entry in the global registry
#[derive(Clone, Debug)]
pub struct RegistryEntry {
    /// Vector ID in the underlying store
    pub vector_id: u64,
    /// Metadata
    pub metadata: VectorMetadata,
}

/// World Semantic Index - Global shared vector registry
///
/// Provides cross-contract composability for semantic vectors.
/// Each domain has its own HNSW index for efficient sharded search.
pub struct SemanticRegistry {
    /// Domain-specific vector stores (sharding by domain)
    stores: HashMap<SemanticDomain, Arc<RwLock<HnswVectorStore>>>,
    /// Global registry mapping: global_id -> (domain, vector_id)
    registry: Arc<RwLock<HashMap<u64, RegistryEntry>>>,
    /// Next global ID counter
    next_id: Arc<RwLock<u64>>,
    /// Default vector dimension
    dimension: usize,
    /// Storage quota per address (in vectors)
    quota_per_address: usize,
    /// Usage tracking: address -> count
    usage: Arc<RwLock<HashMap<[u8; 20], usize>>>,
}

impl SemanticRegistry {
    /// Create a new registry with default dimension
    pub fn new(dimension: usize) -> Self {
        let mut stores = HashMap::new();

        // Initialize stores for common domains
        for domain in [
            SemanticDomain::General,
            SemanticDomain::Finance,
            SemanticDomain::Gaming,
            SemanticDomain::Social,
        ] {
            stores.insert(domain, Arc::new(RwLock::new(HnswVectorStore::new(dimension))));
        }

        Self {
            stores,
            registry: Arc::new(RwLock::new(HashMap::new())),
            next_id: Arc::new(RwLock::new(1)),
            dimension,
            quota_per_address: 1000, // Default: 1000 vectors per address
            usage: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a vector in the global index
    ///
    /// # Arguments
    /// * `owner` - Contract/EOA address registering the vector
    /// * `domain` - Semantic domain for sharding
    /// * `vector` - The embedding vector
    /// * `tags` - Optional tags for filtering
    /// * `block_number` - Current block for timestamp
    /// * `ttl_blocks` - Time-to-live in blocks (0 = permanent)
    ///
    /// # Returns
    /// * Global ID for the registered vector
    pub fn register(
        &mut self,
        owner: [u8; 20],
        domain: SemanticDomain,
        vector: Vec<f32>,
        tags: Vec<[u8; 32]>,
        block_number: u64,
        ttl_blocks: u64,
    ) -> Result<u64, RegistryError> {
        // Check dimension
        if vector.len() != self.dimension {
            return Err(RegistryError::DimensionMismatch {
                expected: self.dimension,
                got: vector.len(),
            });
        }

        // Check quota
        {
            let usage = self.usage.read();
            let current = usage.get(&owner).copied().unwrap_or(0);
            if current >= self.quota_per_address {
                return Err(RegistryError::QuotaExceeded);
            }
        }

        // Get or create domain store
        let store = self.get_or_create_store(domain)?;

        // Generate global ID
        let global_id = {
            let mut counter = self.next_id.write();
            let id = *counter;
            *counter += 1;
            id
        };

        // Insert into domain store
        {
            let mut store_guard = store.write();
            store_guard.insert(global_id, vector)
                .map_err(|e| RegistryError::HnswError(e))?;
        }

        // Create metadata
        let metadata = VectorMetadata {
            owner,
            domain,
            registered_at: block_number,
            expires_at: if ttl_blocks > 0 { block_number + ttl_blocks } else { 0 },
            tags,
        };

        // Register in global index
        {
            let mut registry = self.registry.write();
            registry.insert(global_id, RegistryEntry {
                vector_id: global_id,
                metadata,
            });
        }

        // Update usage
        {
            let mut usage = self.usage.write();
            *usage.entry(owner).or_insert(0) += 1;
        }

        Ok(global_id)
    }

    /// Lookup a vector by global ID
    pub fn lookup(&self, global_id: u64) -> Result<Option<(Vec<f32>, VectorMetadata)>, RegistryError> {
        // Get registry entry
        let entry = {
            let registry = self.registry.read();
            registry.get(&global_id).cloned()
        };

        let entry = match entry {
            Some(e) => e,
            None => return Ok(None),
        };

        // Get vector from domain store
        let store = self.stores.get(&entry.metadata.domain)
            .ok_or(RegistryError::DomainNotFound)?;

        let store_guard = store.read();

        let vector = store_guard.get_vector(global_id);

        match vector {
            Some(v) => Ok(Some((v, entry.metadata))),
            None => Ok(None),
        }
    }

    /// Search within a specific domain
    pub fn search_domain(
        &self,
        domain: SemanticDomain,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<(u64, f32)>, RegistryError> {
        let store = self.stores.get(&domain)
            .ok_or(RegistryError::DomainNotFound)?;

        let store_guard = store.read();

        store_guard.search(query, k)
            .map_err(|e| RegistryError::HnswError(e))
    }

    /// Cross-domain search (searches all domains)
    pub fn search_global(
        &self,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<(u64, f32, SemanticDomain)>, RegistryError> {
        let mut all_results: Vec<(u64, f32, SemanticDomain)> = Vec::new();

        for (domain, store) in &self.stores {
            let store_guard = store.read();

            let results = store_guard.search(query, k)
                .map_err(|e| RegistryError::HnswError(e))?;

            for (id, score) in results {
                all_results.push((id, score, *domain));
            }
        }

        // Sort by score (ascending = closer)
        all_results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        all_results.truncate(k);

        Ok(all_results)
    }

    /// Get metadata for a vector
    pub fn get_metadata(&self, global_id: u64) -> Result<Option<VectorMetadata>, RegistryError> {
        let registry = self.registry.read();

        Ok(registry.get(&global_id).map(|e| e.metadata.clone()))
    }

    /// Check if a vector is expired
    pub fn is_expired(&self, global_id: u64, current_block: u64) -> Result<bool, RegistryError> {
        let registry = self.registry.read();

        match registry.get(&global_id) {
            Some(entry) => {
                if entry.metadata.expires_at == 0 {
                    Ok(false) // Permanent
                } else {
                    Ok(current_block > entry.metadata.expires_at)
                }
            }
            None => Ok(true), // Not found = expired
        }
    }

    /// Get storage usage for an address
    pub fn get_usage(&self, owner: &[u8; 20]) -> Result<usize, RegistryError> {
        let usage = self.usage.read();
        Ok(usage.get(owner).copied().unwrap_or(0))
    }

    /// Get remaining quota for an address
    pub fn get_remaining_quota(&self, owner: &[u8; 20]) -> Result<usize, RegistryError> {
        let used = self.get_usage(owner)?;
        Ok(self.quota_per_address.saturating_sub(used))
    }

    /// Get total vector count across all domains
    pub fn total_vectors(&self) -> Result<usize, RegistryError> {
        let registry = self.registry.read();
        Ok(registry.len())
    }

    /// Get vector count for a specific domain
    pub fn domain_vector_count(&self, domain: SemanticDomain) -> Result<usize, RegistryError> {
        match self.stores.get(&domain) {
            Some(store) => {
                let store_guard = store.read();
                Ok(store_guard.len())
            }
            None => Ok(0),
        }
    }

    /// Helper: Get or create store for a domain
    fn get_or_create_store(&mut self, domain: SemanticDomain) -> Result<Arc<RwLock<HnswVectorStore>>, RegistryError> {
        if let Some(store) = self.stores.get(&domain) {
            return Ok(store.clone());
        }

        // Create new store for this domain
        let new_store = Arc::new(RwLock::new(HnswVectorStore::new(self.dimension)));
        self.stores.insert(domain, new_store.clone());
        Ok(new_store)
    }
}

impl Default for SemanticRegistry {
    fn default() -> Self {
        Self::new(768) // Default to BERT dimension
    }
}

/// Registry errors
#[derive(Debug, Clone)]
pub enum RegistryError {
    /// Vector dimension doesn't match registry
    DimensionMismatch { expected: usize, got: usize },
    /// Address has exceeded storage quota
    QuotaExceeded,
    /// Domain not found
    DomainNotFound,
    /// Underlying HNSW error
    HnswError(HnswError),
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryError::DimensionMismatch { expected, got } => {
                write!(f, "Dimension mismatch: expected {}, got {}", expected, got)
            }
            RegistryError::QuotaExceeded => write!(f, "Storage quota exceeded"),
            RegistryError::DomainNotFound => write!(f, "Domain not found"),
            RegistryError::HnswError(e) => write!(f, "HNSW error: {}", e),
        }
    }
}

impl std::error::Error for RegistryError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_lookup() {
        let mut registry = SemanticRegistry::new(4);
        let owner = [1u8; 20];
        let vector = vec![1.0, 0.0, 0.0, 0.0];

        let id = registry.register(
            owner,
            SemanticDomain::Finance,
            vector.clone(),
            vec![],
            100,
            0, // Permanent
        ).unwrap();

        let (retrieved, metadata) = registry.lookup(id).unwrap().unwrap();
        assert_eq!(retrieved, vector);
        assert_eq!(metadata.domain, SemanticDomain::Finance);
        assert_eq!(metadata.owner, owner);
    }

    #[test]
    fn test_domain_search() {
        let mut registry = SemanticRegistry::new(4);
        let owner = [1u8; 20];

        // Register vectors in Finance domain
        registry.register(owner, SemanticDomain::Finance, vec![1.0, 0.0, 0.0, 0.0], vec![], 100, 0).unwrap();
        registry.register(owner, SemanticDomain::Finance, vec![0.9, 0.1, 0.0, 0.0], vec![], 100, 0).unwrap();

        // Register in Gaming domain
        registry.register(owner, SemanticDomain::Gaming, vec![0.0, 1.0, 0.0, 0.0], vec![], 100, 0).unwrap();

        // Search Finance domain
        let results = registry.search_domain(SemanticDomain::Finance, &[1.0, 0.0, 0.0, 0.0], 2).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_quota_enforcement() {
        let mut registry = SemanticRegistry::new(4);
        registry.quota_per_address = 2; // Low quota for testing

        let owner = [1u8; 20];

        // First two should succeed
        registry.register(owner, SemanticDomain::General, vec![1.0, 0.0, 0.0, 0.0], vec![], 100, 0).unwrap();
        registry.register(owner, SemanticDomain::General, vec![0.0, 1.0, 0.0, 0.0], vec![], 100, 0).unwrap();

        // Third should fail
        let result = registry.register(owner, SemanticDomain::General, vec![0.0, 0.0, 1.0, 0.0], vec![], 100, 0);
        assert!(matches!(result, Err(RegistryError::QuotaExceeded)));
    }

    #[test]
    fn test_expiry() {
        let mut registry = SemanticRegistry::new(4);
        let owner = [1u8; 20];

        // TTL of 100 blocks
        let id = registry.register(
            owner,
            SemanticDomain::General,
            vec![1.0, 0.0, 0.0, 0.0],
            vec![],
            100, // registered at block 100
            50,  // TTL 50 blocks
        ).unwrap();

        // Not expired at block 140
        assert!(!registry.is_expired(id, 140).unwrap());

        // Expired at block 160
        assert!(registry.is_expired(id, 160).unwrap());
    }

    #[test]
    fn test_global_search() {
        let mut registry = SemanticRegistry::new(4);
        let owner = [1u8; 20];

        registry.register(owner, SemanticDomain::Finance, vec![1.0, 0.0, 0.0, 0.0], vec![], 100, 0).unwrap();
        registry.register(owner, SemanticDomain::Gaming, vec![0.9, 0.1, 0.0, 0.0], vec![], 100, 0).unwrap();
        registry.register(owner, SemanticDomain::Social, vec![0.0, 1.0, 0.0, 0.0], vec![], 100, 0).unwrap();

        // Global search should find vectors across all domains
        let results = registry.search_global(&[1.0, 0.0, 0.0, 0.0], 3).unwrap();
        assert_eq!(results.len(), 3);

        // First result should be from Finance (exact match)
        assert_eq!(results[0].2, SemanticDomain::Finance);
    }
}
