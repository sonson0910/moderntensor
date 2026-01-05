"""
Bridge adapter to sync validators from Cardano to Layer 1 blockchain.

This module provides compatibility between the old Cardano-based validator system
and the new Layer 1 PoS consensus mechanism.
"""
import logging
from typing import Dict, Optional, List

logger = logging.getLogger(__name__)

try:
    from ..core.datatypes import ValidatorInfo, MinerInfo
    from ..consensus.pos import ProofOfStake, ValidatorSet
    from ..blockchain.state import StateDB
    HAS_DEPENDENCIES = True
except ImportError:
    HAS_DEPENDENCIES = False
    logger.warning("Could not import all dependencies for ValidatorBridge")


class ValidatorBridge:
    """
    Bridge between Cardano validator system and L1 PoS consensus.
    
    Synchronizes validator state from Cardano metagraph to L1 consensus engine.
    """
    
    def __init__(self, pos_consensus: Optional['ProofOfStake'] = None):
        """
        Initialize validator bridge.
        
        Args:
            pos_consensus: PoS consensus instance (creates new if None)
        """
        if not HAS_DEPENDENCIES:
            raise RuntimeError("Missing dependencies for ValidatorBridge")
        
        self.pos = pos_consensus
        self.sync_count = 0
        
        logger.info("ValidatorBridge initialized")
    
    def sync_from_cardano(
        self,
        validators_info: Dict[str, 'ValidatorInfo']
    ) -> int:
        """
        Sync validators from Cardano system to L1 PoS.
        
        Args:
            validators_info: Dictionary of validator info from Cardano
            
        Returns:
            int: Number of validators synchronized
        """
        if not self.pos:
            logger.warning("No PoS consensus instance, creating default")
            from ..blockchain.state import StateDB
            from ..consensus.pos import ProofOfStake, ConsensusConfig
            state = StateDB()
            self.pos = ProofOfStake(state, ConsensusConfig())
        
        count = self.pos.sync_validators_from_state(validators_info)
        self.sync_count += count
        
        logger.info(f"Synced {count} validators from Cardano to L1")
        return count
    
    def get_validator_mapping(self) -> Dict[str, bytes]:
        """
        Get mapping from Cardano UID to L1 address.
        
        Returns:
            Dict mapping Cardano UID (hex) to L1 address (bytes)
        """
        mapping = {}
        
        if self.pos and self.pos.validator_set:
            for address, validator in self.pos.validator_set.validators.items():
                # Map address back to UID if possible
                uid_hex = address.hex()
                mapping[uid_hex] = address
        
        return mapping
    
    def migrate_validator(
        self,
        cardano_uid: str,
        validator_info: 'ValidatorInfo'
    ) -> bool:
        """
        Migrate a single validator from Cardano to L1.
        
        Args:
            cardano_uid: Cardano validator UID
            validator_info: Validator information
            
        Returns:
            bool: True if migration successful
        """
        if not self.pos:
            logger.error("No PoS consensus instance available")
            return False
        
        success = self.pos.register_validator_from_info(validator_info)
        
        if success:
            logger.info(f"Migrated validator {cardano_uid[:8]}... to L1")
        else:
            logger.error(f"Failed to migrate validator {cardano_uid[:8]}...")
        
        return success
    
    def get_statistics(self) -> Dict[str, int]:
        """
        Get bridge statistics.
        
        Returns:
            Dict with sync statistics
        """
        stats = {
            "total_synced": self.sync_count,
            "active_validators": 0,
        }
        
        if self.pos and self.pos.validator_set:
            stats["active_validators"] = len(
                self.pos.validator_set.get_active_validators()
            )
        
        return stats


class CardanoAdapter:
    """
    Adapter for Cardano blockchain interactions.
    
    Provides a compatibility layer for operations that still need Cardano.
    """
    
    def __init__(self):
        """Initialize Cardano adapter."""
        self.enabled = False
        logger.info("CardanoAdapter initialized (compatibility mode)")
    
    def is_available(self) -> bool:
        """Check if Cardano integration is available."""
        try:
            import pycardano
            self.enabled = True
            return True
        except ImportError:
            logger.warning("pycardano not available, Cardano features disabled")
            return False
    
    def query_validators(self) -> List[Dict]:
        """
        Query validators from Cardano.
        
        Returns:
            List of validator data dicts
        """
        if not self.is_available():
            logger.warning("Cardano not available, returning empty list")
            return []
        
        # TODO: Implement actual Cardano query
        logger.info("Querying validators from Cardano (not implemented)")
        return []
    
    def submit_transaction(self, tx_data: Dict) -> Optional[str]:
        """
        Submit transaction to Cardano.
        
        Args:
            tx_data: Transaction data
            
        Returns:
            Transaction ID or None
        """
        if not self.is_available():
            logger.error("Cannot submit to Cardano - not available")
            return None
        
        # TODO: Implement actual transaction submission
        logger.info("Submitting transaction to Cardano (not implemented)")
        return None
