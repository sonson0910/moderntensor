"""
Storage optimization module.

Provides:
- Database indexing improvements
- State pruning
- Archive vs full node support
"""
from typing import Dict, Any, List, Optional
import time


class StorageOptimizer:
    """
    Optimizes storage layer for better performance.
    
    Features:
    1. Database Indexing - Create optimal indices
    2. State Pruning - Remove old state data
    3. Archive/Full Node - Support different node types
    """
    
    def __init__(self, node_type: str = 'full'):
        """
        Initialize storage optimizer.
        
        Args:
            node_type: 'archive' or 'full' or 'light'
        """
        self.node_type = node_type
        self.pruning_enabled = (node_type != 'archive')
        self.stats = {
            'states_pruned': 0,
            'bytes_freed': 0,
            'indices_created': 0,
        }
    
    def create_indices(self, db: Any) -> None:
        """
        Create optimal database indices for common queries.
        
        Indices to create:
        - Block height -> block hash
        - Transaction hash -> block hash
        - Address -> transactions
        - Address -> balance (derived)
        """
        # This is a placeholder - actual implementation depends on DB type
        indices = [
            ('blocks', ['height']),
            ('blocks', ['timestamp']),
            ('transactions', ['from_address']),
            ('transactions', ['to_address']),
            ('transactions', ['block_height']),
        ]
        
        for table, columns in indices:
            self._create_index(db, table, columns)
            self.stats['indices_created'] += 1
    
    def _create_index(self, db: Any, table: str, columns: List[str]) -> None:
        """Create a single index (placeholder)."""
        pass  # Implementation depends on DB type
    
    def prune_old_state(self, db: Any, keep_blocks: int = 128) -> int:
        """
        Prune old state data to save disk space.
        
        Keeps recent state and removes old state that's no longer needed.
        Archive nodes don't prune.
        
        Args:
            db: Database instance
            keep_blocks: Number of recent blocks to keep full state
            
        Returns:
            int: Number of bytes freed
        """
        if not self.pruning_enabled:
            return 0
        
        # Get current height
        # Remove state for blocks older than (current_height - keep_blocks)
        # Keep only state roots (32 bytes) for old blocks
        
        bytes_freed = 0  # Placeholder
        self.stats['states_pruned'] += keep_blocks
        self.stats['bytes_freed'] += bytes_freed
        
        return bytes_freed
    
    def compact_database(self, db: Any) -> int:
        """
        Compact database to reclaim space.
        
        Args:
            db: Database instance
            
        Returns:
            int: Bytes freed by compaction
        """
        # LevelDB/RocksDB compaction
        # This would call db.compact_range() or similar
        bytes_freed = 0
        return bytes_freed
    
    def suggest_node_type(self, available_disk_gb: int, use_case: str) -> str:
        """
        Suggest optimal node type based on resources and use case.
        
        Args:
            available_disk_gb: Available disk space in GB
            use_case: 'validator', 'explorer', 'wallet', or 'developer'
            
        Returns:
            str: Suggested node type
        """
        if use_case == 'explorer':
            return 'archive'  # Needs full history
        elif use_case == 'validator':
            return 'full'  # Needs recent state
        elif available_disk_gb < 100:
            return 'light'  # Limited disk
        elif available_disk_gb > 500:
            return 'archive'  # Plenty of space
        else:
            return 'full'  # Standard
    
    def get_stats(self) -> Dict[str, Any]:
        """Get storage optimizer statistics."""
        return {
            **self.stats,
            'node_type': self.node_type,
            'pruning_enabled': self.pruning_enabled,
        }
