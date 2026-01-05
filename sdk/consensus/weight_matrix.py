# sdk/consensus/weight_matrix.py
"""
Production-ready Weight Matrix Manager with hybrid storage for Layer 1 blockchain.

This module implements a 3-layer storage strategy for weight matrices:
1. Layer 1 (On-Chain): Weight matrix hash (Merkle root) for verification
2. Layer 2 (Database): Full weight matrix for fast queries using LevelDB
3. Layer 3 (IPFS): Historical weight matrices for audit trail

Key benefits:
- Fast verification using on-chain Merkle roots with binary Merkle tree
- Quick queries using LevelDB
- Permanent archive using IPFS
- Reduced on-chain storage costs
"""

import numpy as np
import hashlib
import json
import logging
from typing import List, Dict, Optional, Tuple
from dataclasses import dataclass, asdict
import scipy.sparse
from datetime import datetime
from pathlib import Path

from sdk.utils.merkle_tree import MerkleTree, MerkleProof
from sdk.utils.ipfs_client import IPFSClient, IPFSConfig, get_ipfs_client
from sdk.storage.blockchain_db import LevelDBWrapper

logger = logging.getLogger(__name__)


@dataclass
class WeightMatrixMetadata:
    """Metadata for a weight matrix."""
    subnet_uid: int
    epoch: int
    num_validators: int
    num_miners: int
    timestamp: int
    merkle_root: bytes
    ipfs_hash: str
    is_sparse: bool
    compression_ratio: float
    
    def to_dict(self) -> dict:
        """Convert to dictionary for JSON serialization."""
        return {
            'subnet_uid': self.subnet_uid,
            'epoch': self.epoch,
            'num_validators': self.num_validators,
            'num_miners': self.num_miners,
            'timestamp': self.timestamp,
            'merkle_root': self.merkle_root.hex(),
            'ipfs_hash': self.ipfs_hash,
            'is_sparse': self.is_sparse,
            'compression_ratio': self.compression_ratio
        }
    
    @classmethod
    def from_dict(cls, data: dict) -> 'WeightMatrixMetadata':
        """Create from dictionary."""
        data_copy = data.copy()
        if isinstance(data_copy['merkle_root'], str):
            data_copy['merkle_root'] = bytes.fromhex(data_copy['merkle_root'])
        return cls(**data_copy)


class WeightMatrixManager:
    """
    Production-ready weight matrix manager with hybrid storage.
    
    Storage layers:
    1. On-chain: Merkle root hash only (32 bytes)
    2. Database: Full matrix for fast access (LevelDB)
    3. IPFS: Historical matrices for audit
    """
    
    def __init__(
        self,
        db_path: Optional[str] = None,
        ipfs_config: Optional[IPFSConfig] = None,
        enable_ipfs: bool = True
    ):
        """
        Initialize the weight matrix manager.
        
        Args:
            db_path: Path to LevelDB database (creates if not exists)
            ipfs_config: IPFS configuration
            enable_ipfs: Whether to enable IPFS storage
        """
        # Initialize LevelDB for Layer 2 storage
        if db_path is None:
            db_path = str(Path.home() / ".moderntensor" / "weight_matrices")
        
        try:
            self.db = LevelDBWrapper(db_path, create_if_missing=True)
            logger.info(f"Weight matrix database initialized at {db_path}")
        except ImportError:
            logger.warning("LevelDB not available, using in-memory storage")
            self.db = None
            self._mem_db: Dict[str, dict] = {}
        
        # Initialize IPFS client for Layer 3 storage
        self.enable_ipfs = enable_ipfs
        if enable_ipfs:
            self.ipfs = get_ipfs_client(ipfs_config)
        else:
            self.ipfs = None
        
        # In-memory cache for recent matrices
        self.cache: Dict[Tuple[int, int], np.ndarray] = {}
        self.metadata_cache: Dict[Tuple[int, int], WeightMatrixMetadata] = {}
        
        logger.info("WeightMatrixManager initialized (production-ready)")
    
    async def store_weight_matrix(
        self,
        subnet_uid: int,
        epoch: int,
        weights: np.ndarray,
        upload_to_ipfs: bool = True
    ) -> Tuple[bytes, str]:
        """
        Store weight matrix with 3-layer approach:
        1. Calculate Merkle root using binary tree
        2. Upload full matrix to IPFS (optional)
        3. Store in local DB for fast query
        4. Return root hash for on-chain storage
        
        Args:
            subnet_uid: Subnet identifier
            epoch: Epoch number
            weights: Weight matrix (N validators x M miners)
            upload_to_ipfs: Whether to upload to IPFS (default True)
            
        Returns:
            Tuple of (merkle_root, ipfs_hash)
        """
        # Validate input
        if weights.ndim != 2:
            raise ValueError("Weight matrix must be 2-dimensional")
        
        num_validators, num_miners = weights.shape
        
        # Determine if matrix is sparse
        sparsity = 1.0 - (np.count_nonzero(weights) / weights.size)
        is_sparse = sparsity > 0.5
        
        # Compress matrix if sparse
        if is_sparse:
            compressed = scipy.sparse.csr_matrix(weights)
            matrix_bytes = self._serialize_sparse_matrix(compressed)
            compression_ratio = len(matrix_bytes) / weights.nbytes
        else:
            matrix_bytes = weights.tobytes()
            compression_ratio = 1.0
        
        # Calculate Merkle root using production binary tree
        merkle_root = self._calculate_merkle_root(weights)
        
        # Upload to IPFS (Layer 3)
        ipfs_hash = ""
        if upload_to_ipfs and self.enable_ipfs and self.ipfs:
            try:
                metadata = {
                    'subnet_uid': subnet_uid,
                    'epoch': epoch,
                    'shape': list(weights.shape),
                    'is_sparse': is_sparse,
                    'timestamp': int(datetime.now().timestamp())
                }
                
                ipfs_hash = await self.ipfs.add(matrix_bytes, metadata)
                logger.info(f"Uploaded weight matrix to IPFS: {ipfs_hash}")
                
                # Pin the content
                await self.ipfs.pin(ipfs_hash)
            
            except (ConnectionError, TimeoutError) as e:
                logger.warning(f"Failed to upload to IPFS: {e}")
                ipfs_hash = "local_only"
        else:
            ipfs_hash = "local_only"
        
        # Create metadata
        metadata = WeightMatrixMetadata(
            subnet_uid=subnet_uid,
            epoch=epoch,
            num_validators=num_validators,
            num_miners=num_miners,
            timestamp=int(datetime.now().timestamp()),
            merkle_root=merkle_root,
            ipfs_hash=ipfs_hash,
            is_sparse=is_sparse,
            compression_ratio=compression_ratio
        )
        
        # Store in DB (Layer 2)
        await self._store_in_db(subnet_uid, epoch, weights, metadata)
        
        # Cache for quick access
        self.cache[(subnet_uid, epoch)] = weights
        self.metadata_cache[(subnet_uid, epoch)] = metadata
        
        logger.info(
            f"Stored weight matrix: subnet={subnet_uid}, epoch={epoch}, "
            f"shape={weights.shape}, sparse={is_sparse}"
        )
        
        return merkle_root, ipfs_hash
    
    async def get_weight_matrix(
        self,
        subnet_uid: int,
        epoch: int,
        from_ipfs: bool = False
    ) -> Optional[np.ndarray]:
        """
        Retrieve weight matrix.
        
        Args:
            subnet_uid: Subnet identifier
            epoch: Epoch number
            from_ipfs: Force retrieval from IPFS (default False)
            
        Returns:
            Weight matrix or None if not found
        """
        # Check cache first
        key = (subnet_uid, epoch)
        if key in self.cache and not from_ipfs:
            logger.debug(f"Weight matrix cache hit: {key}")
            return self.cache[key]
        
        # Try database
        if not from_ipfs:
            weights = await self._get_from_db(subnet_uid, epoch)
            if weights is not None:
                self.cache[key] = weights
                logger.debug(f"Weight matrix retrieved from DB: {key}")
                return weights
        
        # Try IPFS as fallback
        if self.enable_ipfs and self.ipfs:
            metadata = self.metadata_cache.get(key)
            if metadata and metadata.ipfs_hash != "local_only":
                try:
                    data, _ = await self.ipfs.get_with_metadata(metadata.ipfs_hash)
                    
                    # Deserialize
                    if metadata.is_sparse:
                        weights = self._deserialize_sparse_matrix(data).toarray()
                    else:
                        shape = (metadata.num_validators, metadata.num_miners)
                        weights = np.frombuffer(data, dtype=np.float64).reshape(shape)
                    
                    if weights is not None:
                        self.cache[key] = weights
                        # Store in DB for future use
                        await self._store_in_db(subnet_uid, epoch, weights, metadata)
                        logger.info(f"Weight matrix retrieved from IPFS: {key}")
                        return weights
                
                except (ConnectionError, TimeoutError, FileNotFoundError) as e:
                    logger.warning(f"Failed to download from IPFS: {e}")
        
        return None
    
    async def verify_weight_matrix(
        self,
        subnet_uid: int,
        epoch: int,
        weights: np.ndarray,
        merkle_root: bytes
    ) -> bool:
        """
        Verify weights against on-chain Merkle root.
        
        Args:
            subnet_uid: Subnet identifier
            epoch: Epoch number
            weights: Weight matrix to verify
            merkle_root: On-chain Merkle root
            
        Returns:
            True if verification passes
        """
        # Calculate Merkle root from provided weights
        calculated_root = self._calculate_merkle_root(weights)
        
        # Compare with on-chain root
        is_valid = calculated_root == merkle_root
        
        if is_valid:
            logger.info(f"Weight matrix verification passed: subnet={subnet_uid}, epoch={epoch}")
        else:
            logger.warning(f"Weight matrix verification FAILED: subnet={subnet_uid}, epoch={epoch}")
        
        return is_valid
    
    def _calculate_merkle_root(self, weights: np.ndarray) -> bytes:
        """
        Calculate Merkle root of weight matrix using production binary tree.
        
        Args:
            weights: Weight matrix
            
        Returns:
            Merkle root (32 bytes)
        """
        # Create leaf hashes from each row
        leaves = []
        for row in weights:
            row_bytes = row.tobytes()
            leaf_hash = hashlib.sha256(row_bytes).digest()
            leaves.append(leaf_hash)
        
        # Build Merkle tree
        tree = MerkleTree(leaves)
        
        return tree.get_root()
    
    def generate_merkle_proof(
        self,
        weights: np.ndarray,
        row_index: int
    ) -> MerkleProof:
        """
        Generate Merkle proof for a specific row.
        
        Args:
            weights: Weight matrix
            row_index: Row index to generate proof for
            
        Returns:
            MerkleProof object
        """
        # Create leaf hashes
        leaves = []
        for row in weights:
            row_bytes = row.tobytes()
            leaf_hash = hashlib.sha256(row_bytes).digest()
            leaves.append(leaf_hash)
        
        # Build tree and get proof
        tree = MerkleTree(leaves)
        proof = tree.get_proof(row_index)
        
        return proof
    
    def _serialize_sparse_matrix(self, sparse_matrix: scipy.sparse.csr_matrix) -> bytes:
        """Serialize sparse matrix to bytes."""
        data = {
            'data': sparse_matrix.data.tolist(),
            'indices': sparse_matrix.indices.tolist(),
            'indptr': sparse_matrix.indptr.tolist(),
            'shape': sparse_matrix.shape
        }
        return json.dumps(data).encode('utf-8')
    
    def _deserialize_sparse_matrix(self, data: bytes) -> scipy.sparse.csr_matrix:
        """Deserialize sparse matrix from bytes."""
        obj = json.loads(data.decode('utf-8'))
        return scipy.sparse.csr_matrix(
            (obj['data'], obj['indices'], obj['indptr']),
            shape=obj['shape']
        )
    
    async def _store_in_db(
        self,
        subnet_uid: int,
        epoch: int,
        weights: np.ndarray,
        metadata: WeightMatrixMetadata
    ) -> None:
        """Store weight matrix in database."""
        key = f"weights_{subnet_uid}_{epoch}"
        
        # Serialize matrix
        if metadata.is_sparse:
            sparse_matrix = scipy.sparse.csr_matrix(weights)
            matrix_bytes = self._serialize_sparse_matrix(sparse_matrix)
        else:
            matrix_bytes = weights.tobytes()
        
        # Prepare data
        data = {
            'weights': matrix_bytes.hex() if self.db else matrix_bytes,
            'metadata': metadata.to_dict(),
            'shape': list(weights.shape),
            'is_sparse': metadata.is_sparse
        }
        
        # Store in DB
        if self.db:
            try:
                value = json.dumps(data).encode('utf-8')
                self.db.put(key.encode('utf-8'), value)
            except Exception as e:
                logger.error(f"Failed to store in DB: {e}")
        else:
            self._mem_db[key] = data
    
    async def _get_from_db(
        self,
        subnet_uid: int,
        epoch: int
    ) -> Optional[np.ndarray]:
        """Retrieve weight matrix from database."""
        key = f"weights_{subnet_uid}_{epoch}"
        
        # Get from DB
        if self.db:
            try:
                value = self.db.get(key.encode('utf-8'))
                if value is None:
                    return None
                data = json.loads(value.decode('utf-8'))
            except Exception as e:
                logger.error(f"Failed to get from DB: {e}")
                return None
        else:
            data = self._mem_db.get(key)
            if data is None:
                return None
        
        # Deserialize matrix
        try:
            if isinstance(data['weights'], str):
                matrix_bytes = bytes.fromhex(data['weights'])
            else:
                matrix_bytes = data['weights']
            
            if data['is_sparse']:
                sparse_matrix = self._deserialize_sparse_matrix(matrix_bytes)
                return sparse_matrix.toarray()
            else:
                shape = tuple(data['shape'])
                return np.frombuffer(matrix_bytes, dtype=np.float64).reshape(shape)
        except Exception as e:
            logger.error(f"Failed to deserialize weight matrix: {e}")
            return None
    
    async def get_metadata(
        self,
        subnet_uid: int,
        epoch: int
    ) -> Optional[WeightMatrixMetadata]:
        """Get metadata for a weight matrix."""
        # Check cache
        key = (subnet_uid, epoch)
        if key in self.metadata_cache:
            return self.metadata_cache[key]
        
        # Try database
        db_key = f"weights_{subnet_uid}_{epoch}"
        
        if self.db:
            try:
                value = self.db.get(db_key.encode('utf-8'))
                if value is None:
                    return None
                data = json.loads(value.decode('utf-8'))
            except Exception:
                return None
        else:
            data = self._mem_db.get(db_key)
            if data is None:
                return None
        
        metadata = WeightMatrixMetadata.from_dict(data['metadata'])
        self.metadata_cache[key] = metadata
        return metadata
    
    def get_storage_stats(self) -> Dict:
        """Get statistics about stored weight matrices."""
        if self.db:
            # Count entries in LevelDB
            count = 0
            total_size = 0
            try:
                for key, value in self.db.iterator(prefix=b'weights_'):
                    count += 1
                    total_size += len(value)
            except Exception:
                pass
            
            total_matrices = count
            total_size_bytes = total_size
        else:
            total_matrices = len(self._mem_db)
            total_size_bytes = sum(
                len(v['weights']) if isinstance(v['weights'], bytes) 
                else len(v['weights'].encode())
                for v in self._mem_db.values()
            )
        
        # Calculate average compression ratio
        compression_ratios = []
        for metadata in self.metadata_cache.values():
            compression_ratios.append(metadata.compression_ratio)
        
        avg_compression = (
            sum(compression_ratios) / len(compression_ratios)
            if compression_ratios else 0.0
        )
        
        return {
            'total_matrices': total_matrices,
            'total_size_bytes': total_size_bytes,
            'cache_size': len(self.cache),
            'avg_compression_ratio': avg_compression,
            'ipfs_enabled': self.enable_ipfs
        }
    
    def clear_cache(self) -> None:
        """Clear the in-memory cache."""
        self.cache.clear()
        self.metadata_cache.clear()
        logger.info("Weight matrix cache cleared")
    
    async def prune_old_matrices(
        self,
        subnet_uid: int,
        keep_recent: int = 100
    ) -> int:
        """
        Prune old weight matrices from database, keeping only recent ones.
        
        Args:
            subnet_uid: Subnet to prune
            keep_recent: Number of recent epochs to keep
            
        Returns:
            Number of matrices pruned
        """
        # Find all epochs for this subnet
        epochs = []
        prefix = f"weights_{subnet_uid}_"
        
        if self.db:
            try:
                for key, _ in self.db.iterator(prefix=prefix.encode('utf-8')):
                    key_str = key.decode('utf-8')
                    epoch_str = key_str.split('_')[-1]
                    try:
                        epochs.append(int(epoch_str))
                    except ValueError:
                        continue
            except Exception:
                pass
        else:
            for key in list(self._mem_db.keys()):
                if key.startswith(prefix):
                    epoch_str = key.split('_')[-1]
                    try:
                        epochs.append(int(epoch_str))
                    except ValueError:
                        continue
        
        # Sort and identify old epochs
        epochs.sort(reverse=True)
        epochs_to_remove = epochs[keep_recent:]
        
        # Remove old matrices
        count = 0
        for epoch in epochs_to_remove:
            key = f"weights_{subnet_uid}_{epoch}"
            
            if self.db:
                try:
                    self.db.delete(key.encode('utf-8'))
                    count += 1
                except Exception:
                    pass
            else:
                if key in self._mem_db:
                    del self._mem_db[key]
                    count += 1
            
            # Remove from cache too
            cache_key = (subnet_uid, epoch)
            if cache_key in self.cache:
                del self.cache[cache_key]
            if cache_key in self.metadata_cache:
                del self.metadata_cache[cache_key]
        
        logger.info(f"Pruned {count} old weight matrices for subnet {subnet_uid}")
        return count
    
    def close(self):
        """Close database connection."""
        if self.db and hasattr(self.db, 'db'):
            try:
                self.db.db.close()
                logger.info("Weight matrix database closed")
            except Exception as e:
                logger.error(f"Error closing database: {e}")
