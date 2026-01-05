# sdk/consensus/weight_matrix.py
"""
Weight Matrix Manager with hybrid storage for Layer 1 blockchain.

This module implements a 3-layer storage strategy for weight matrices:
1. Layer 1 (On-Chain): Weight matrix hash (Merkle root) for verification
2. Layer 2 (Database): Full weight matrix for fast queries
3. Layer 3 (IPFS/Arweave): Historical weight matrices for audit trail

Key benefits:
- Fast verification using on-chain Merkle roots
- Quick queries using local database
- Permanent archive using decentralized storage
- Reduced on-chain storage costs
"""

import numpy as np
import hashlib
import json
from typing import List, Dict, Optional, Tuple
from dataclasses import dataclass, asdict
import scipy.sparse
from datetime import datetime


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


class WeightMatrixManager:
    """
    Manage weight matrices with hybrid storage.
    
    Storage layers:
    1. On-chain: Merkle root hash only (32 bytes)
    2. Database: Full matrix for fast access
    3. IPFS: Historical matrices for audit
    """
    
    def __init__(self, db=None, ipfs_client=None):
        """
        Initialize the weight matrix manager.
        
        Args:
            db: Database connection for Layer 2 storage (optional)
            ipfs_client: IPFS client for Layer 3 storage (optional)
        """
        self.db = db or {}  # Simple dict for testing, use real DB in production
        self.ipfs = ipfs_client
        
        # In-memory cache for recent matrices
        self.cache: Dict[Tuple[int, int], np.ndarray] = {}
        self.metadata_cache: Dict[Tuple[int, int], WeightMatrixMetadata] = {}
    
    async def store_weight_matrix(
        self,
        subnet_uid: int,
        epoch: int,
        weights: np.ndarray,
        upload_to_ipfs: bool = True
    ) -> Tuple[bytes, str]:
        """
        Store weight matrix with 3-layer approach:
        1. Calculate Merkle root
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
        
        # Calculate Merkle root
        merkle_root = self._calculate_merkle_root(weights)
        
        # Upload to IPFS (Layer 3)
        import logging
        ipfs_hash = ""
        if upload_to_ipfs and self.ipfs:
            try:
                ipfs_hash = await self._upload_to_ipfs(matrix_bytes, metadata={
                    'subnet_uid': subnet_uid,
                    'epoch': epoch,
                    'shape': list(weights.shape),
                    'is_sparse': is_sparse,
                    'timestamp': int(datetime.now().timestamp())
                })
            except (ConnectionError, TimeoutError) as e:
                logging.warning(f"Failed to upload to IPFS: {e}")
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
            return self.cache[key]
        
        # Try database
        if not from_ipfs:
            weights = await self._get_from_db(subnet_uid, epoch)
            if weights is not None:
                self.cache[key] = weights
                return weights
        
        # Try IPFS as fallback
        if self.ipfs:
            import logging
            metadata = self.metadata_cache.get(key)
            if metadata and metadata.ipfs_hash != "local_only":
                try:
                    weights = await self._download_from_ipfs(metadata.ipfs_hash)
                    if weights is not None:
                        self.cache[key] = weights
                        # Store in DB for future use
                        await self._store_in_db(subnet_uid, epoch, weights, metadata)
                        return weights
                except (ConnectionError, TimeoutError) as e:
                    logging.warning(f"Failed to download from IPFS: {e}")
        
        return None
    
    async def verify_weight_matrix(
        self,
        subnet_uid: int,
        epoch: int,
        weights: np.ndarray,
        merkle_root: bytes,
        merkle_proof: Optional[List[bytes]] = None
    ) -> bool:
        """
        Verify weights against on-chain Merkle root.
        
        Args:
            subnet_uid: Subnet identifier
            epoch: Epoch number
            weights: Weight matrix to verify
            merkle_root: On-chain Merkle root
            merkle_proof: Optional Merkle proof for specific entry
            
        Returns:
            True if verification passes
        """
        # Calculate Merkle root from provided weights
        calculated_root = self._calculate_merkle_root(weights)
        
        # Compare with on-chain root
        return calculated_root == merkle_root
    
    def _calculate_merkle_root(self, weights: np.ndarray) -> bytes:
        """
        Calculate Merkle root of weight matrix.
        
        IMPORTANT: This is a simplified implementation suitable for development
        and testing. For production use, implement a proper Merkle tree with:
        - Binary tree structure with left/right branches
        - Proof generation capability
        - Proper leaf node ordering
        
        Current approach: Hash each row individually, then hash all row hashes.
        This provides basic integrity checking but lacks proof generation.
        """
        import hashlib
        
        # Hash each row
        row_hashes = []
        for row in weights:
            row_bytes = row.tobytes()
            row_hash = hashlib.sha256(row_bytes).digest()
            row_hashes.append(row_hash)
        
        # Build simplified root (hash all row hashes together)
        # TODO: Replace with proper binary Merkle tree for production
        all_hashes = b''.join(row_hashes)
        merkle_root = hashlib.sha256(all_hashes).digest()
        
        return merkle_root
    
    def _calculate_merkle_proof(
        self,
        weights: np.ndarray,
        row_index: int
    ) -> List[bytes]:
        """
        Generate Merkle proof for a specific row.
        
        NOTE: This method is not yet implemented. A proper Merkle tree
        implementation with binary tree structure is needed for production use.
        """
        raise NotImplementedError(
            "Merkle proof generation not yet implemented. "
            "Use _calculate_merkle_root() for basic verification instead."
        )
    
    def _serialize_sparse_matrix(self, sparse_matrix: scipy.sparse.csr_matrix) -> bytes:
        """Serialize sparse matrix to bytes."""
        # Store matrix data in a compact format
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
    
    async def _upload_to_ipfs(self, data: bytes, metadata: dict) -> str:
        """
        Upload data to IPFS.
        
        Args:
            data: Binary data to upload
            metadata: Metadata dictionary
            
        Returns:
            IPFS hash (CID)
        """
        if not self.ipfs:
            return "ipfs_not_configured"
        
        # Create a JSON object with data and metadata
        upload_obj = {
            'data': data.hex(),
            'metadata': metadata
        }
        upload_bytes = json.dumps(upload_obj).encode('utf-8')
        
        # Upload to IPFS (mock implementation)
        # In production, use actual IPFS client
        # ipfs_hash = await self.ipfs.add(upload_bytes)
        
        # For now, return a mock IPFS hash
        ipfs_hash = f"Qm{hashlib.sha256(upload_bytes).hexdigest()[:44]}"
        
        return ipfs_hash
    
    async def _download_from_ipfs(self, ipfs_hash: str) -> Optional[np.ndarray]:
        """
        Download and deserialize weight matrix from IPFS.
        
        Args:
            ipfs_hash: IPFS CID
            
        Returns:
            Weight matrix or None
        """
        if not self.ipfs:
            return None
        
        # Download from IPFS (mock implementation)
        # In production, use actual IPFS client
        # data = await self.ipfs.cat(ipfs_hash)
        
        # For now, return None (not implemented)
        return None
    
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
        
        # Store in DB (simple dict for now)
        self.db[key] = {
            'weights': matrix_bytes,
            'metadata': asdict(metadata),
            'shape': list(weights.shape),
            'is_sparse': metadata.is_sparse
        }
    
    async def _get_from_db(
        self,
        subnet_uid: int,
        epoch: int
    ) -> Optional[np.ndarray]:
        """Retrieve weight matrix from database."""
        key = f"weights_{subnet_uid}_{epoch}"
        
        if key not in self.db:
            return None
        
        data = self.db[key]
        
        # Deserialize matrix
        if data['is_sparse']:
            sparse_matrix = self._deserialize_sparse_matrix(data['weights'])
            return sparse_matrix.toarray()
        else:
            shape = tuple(data['shape'])
            return np.frombuffer(data['weights'], dtype=np.float64).reshape(shape)
    
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
        if db_key in self.db:
            metadata_dict = self.db[db_key]['metadata']
            metadata = WeightMatrixMetadata(**metadata_dict)
            self.metadata_cache[key] = metadata
            return metadata
        
        return None
    
    def get_storage_stats(self) -> Dict:
        """Get statistics about stored weight matrices."""
        total_matrices = len(self.db)
        total_size = sum(len(v['weights']) for v in self.db.values())
        
        # Calculate average compression ratio
        compression_ratios = [
            v['metadata']['compression_ratio'] 
            for v in self.db.values()
        ]
        avg_compression = (
            sum(compression_ratios) / len(compression_ratios)
            if compression_ratios else 0.0
        )
        
        return {
            'total_matrices': total_matrices,
            'total_size_bytes': total_size,
            'cache_size': len(self.cache),
            'avg_compression_ratio': avg_compression
        }
    
    def clear_cache(self) -> None:
        """Clear the in-memory cache."""
        self.cache.clear()
        self.metadata_cache.clear()
    
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
        for key in list(self.db.keys()):
            if key.startswith(f"weights_{subnet_uid}_"):
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
            if key in self.db:
                del self.db[key]
                count += 1
            
            # Remove from cache too
            cache_key = (subnet_uid, epoch)
            if cache_key in self.cache:
                del self.cache[cache_key]
            if cache_key in self.metadata_cache:
                del self.metadata_cache[cache_key]
        
        return count
