"""
Layer 1 Staking Service for ModernTensor blockchain.

Provides staking operations for the native Layer 1 blockchain.
"""
import logging
from typing import Optional, Dict

from .transaction import StakingTransaction
from .state import StateDB
from .crypto import KeyPair

logger = logging.getLogger(__name__)


class L1StakingService:
    """
    Manages staking operations for Layer 1 validators.
    
    This service handles:
    - Validator registration and staking
    - Unstaking requests
    - Reward claiming
    - Staking state queries
    """
    
    def __init__(self, state_db: StateDB):
        """
        Initialize staking service.
        
        Args:
            state_db: State database instance
        """
        self.state = state_db
        logger.info("L1StakingService initialized")
    
    def stake(
        self,
        from_address: bytes,
        validator_address: bytes,
        amount: int,
        public_key: bytes,
        private_key: bytes,
        nonce: int,
        gas_price: int = 1000,
        gas_limit: int = 100000
    ) -> Optional[StakingTransaction]:
        """
        Create and sign a stake transaction.
        
        Args:
            from_address: Staker address (20 bytes)
            validator_address: Validator address (20 bytes)
            amount: Amount to stake
            public_key: Validator public key (32 bytes)
            private_key: Staker private key for signing (32 bytes)
            nonce: Transaction nonce
            gas_price: Gas price
            gas_limit: Gas limit
            
        Returns:
            Optional[StakingTransaction]: Signed transaction or None if validation fails
        """
        # Validate inputs
        if len(from_address) != 20:
            logger.error(f"Invalid from_address length: {len(from_address)}, expected 20")
            return None
        if len(validator_address) != 20:
            logger.error(f"Invalid validator_address length: {len(validator_address)}, expected 20")
            return None
        if len(public_key) != 32:
            logger.error(f"Invalid public_key length: {len(public_key)}, expected 32")
            return None
        if amount <= 0:
            logger.error(f"Invalid stake amount: {amount}")
            return None
        
        # Check balance
        balance = self.state.get_balance(from_address)
        total_cost = amount + (gas_price * gas_limit)
        if balance < total_cost:
            logger.error(f"Insufficient balance: {balance} < {total_cost}")
            return None
        
        # Create staking transaction
        tx = StakingTransaction(
            tx_type='stake',
            nonce=nonce,
            from_address=from_address,
            validator_address=validator_address,
            amount=amount,
            gas_price=gas_price,
            gas_limit=gas_limit,
            public_key=public_key,
        )
        
        # Sign transaction
        tx.sign(private_key)
        
        logger.info(
            f"Created stake transaction: {amount} from {from_address.hex()[:8]}... "
            f"for validator {validator_address.hex()[:8]}..."
        )
        
        return tx
    
    def unstake(
        self,
        from_address: bytes,
        validator_address: bytes,
        amount: int,
        private_key: bytes,
        nonce: int,
        gas_price: int = 1000,
        gas_limit: int = 100000
    ) -> Optional[StakingTransaction]:
        """
        Create and sign an unstake transaction.
        
        Args:
            from_address: Staker address (20 bytes)
            validator_address: Validator address (20 bytes)
            amount: Amount to unstake
            private_key: Staker private key for signing (32 bytes)
            nonce: Transaction nonce
            gas_price: Gas price
            gas_limit: Gas limit
            
        Returns:
            Optional[StakingTransaction]: Signed transaction or None if validation fails
        """
        # Validate inputs
        if len(from_address) != 20 or len(validator_address) != 20:
            logger.error("Invalid address length")
            return None
        if amount <= 0:
            logger.error(f"Invalid unstake amount: {amount}")
            return None
        
        # Check staked amount
        staked = self.state.get_staked_amount(validator_address)
        if staked < amount:
            logger.error(f"Insufficient stake: {staked} < {amount}")
            return None
        
        # Check gas balance
        balance = self.state.get_balance(from_address)
        gas_cost = gas_price * gas_limit
        if balance < gas_cost:
            logger.error(f"Insufficient balance for gas: {balance} < {gas_cost}")
            return None
        
        # Create unstaking transaction
        tx = StakingTransaction(
            tx_type='unstake',
            nonce=nonce,
            from_address=from_address,
            validator_address=validator_address,
            amount=amount,
            gas_price=gas_price,
            gas_limit=gas_limit,
        )
        
        # Sign transaction
        tx.sign(private_key)
        
        logger.info(
            f"Created unstake transaction: {amount} from validator {validator_address.hex()[:8]}..."
        )
        
        return tx
    
    def claim_rewards(
        self,
        from_address: bytes,
        validator_address: bytes,
        private_key: bytes,
        nonce: int,
        gas_price: int = 1000,
        gas_limit: int = 50000
    ) -> Optional[StakingTransaction]:
        """
        Create and sign a claim rewards transaction.
        
        Args:
            from_address: Claimer address (20 bytes)
            validator_address: Validator address (20 bytes)
            private_key: Claimer private key for signing (32 bytes)
            nonce: Transaction nonce
            gas_price: Gas price
            gas_limit: Gas limit
            
        Returns:
            Optional[StakingTransaction]: Signed transaction or None if validation fails
        """
        # Validate inputs
        if len(from_address) != 20 or len(validator_address) != 20:
            logger.error("Invalid address length")
            return None
        
        # Check pending rewards
        pending_rewards = self.state.get_pending_rewards(validator_address)
        if pending_rewards == 0:
            logger.warning(f"No pending rewards for validator {validator_address.hex()[:8]}...")
            return None
        
        # Check gas balance
        balance = self.state.get_balance(from_address)
        gas_cost = gas_price * gas_limit
        if balance < gas_cost:
            logger.error(f"Insufficient balance for gas: {balance} < {gas_cost}")
            return None
        
        # Create claim rewards transaction
        tx = StakingTransaction(
            tx_type='claim_rewards',
            nonce=nonce,
            from_address=from_address,
            validator_address=validator_address,
            amount=0,  # No amount for claim
            gas_price=gas_price,
            gas_limit=gas_limit,
        )
        
        # Sign transaction
        tx.sign(private_key)
        
        logger.info(
            f"Created claim rewards transaction for validator {validator_address.hex()[:8]}... "
            f"(pending: {pending_rewards})"
        )
        
        return tx
    
    def execute_staking_tx(self, tx: StakingTransaction) -> bool:
        """
        Execute a staking transaction and update state.
        
        Args:
            tx: Staking transaction to execute
            
        Returns:
            bool: True if successful
        """
        # Verify signature
        if not tx.verify_signature():
            logger.error("Invalid transaction signature")
            return False
        
        # Calculate gas cost
        gas_used = tx.intrinsic_gas()
        gas_cost = tx.gas_price * gas_used
        
        # Check balance for gas
        if self.state.get_balance(tx.from_address) < gas_cost:
            logger.error("Insufficient balance for gas")
            return False
        
        # Execute based on transaction type
        try:
            if tx.tx_type == 'stake':
                return self._execute_stake(tx, gas_cost)
            elif tx.tx_type == 'unstake':
                return self._execute_unstake(tx, gas_cost)
            elif tx.tx_type == 'claim_rewards':
                return self._execute_claim_rewards(tx, gas_cost)
            else:
                logger.error(f"Unknown transaction type: {tx.tx_type}")
                return False
        except Exception as e:
            logger.exception(f"Error executing staking transaction: {e}")
            return False
    
    def _execute_stake(self, tx: StakingTransaction, gas_cost: int) -> bool:
        """Execute stake transaction."""
        total_cost = tx.amount + gas_cost
        
        # Deduct amount and gas from staker
        if not self.state.sub_balance(tx.from_address, total_cost):
            logger.error("Failed to deduct stake and gas")
            return False
        
        # Add to staked amount
        self.state.add_stake(tx.validator_address, tx.amount)
        
        # Register/update validator info
        self.state.set_validator_info(tx.validator_address, tx.public_key, active=True)
        
        logger.info(
            f"Executed stake: {tx.amount} for validator {tx.validator_address.hex()[:8]}..."
        )
        return True
    
    def _execute_unstake(self, tx: StakingTransaction, gas_cost: int) -> bool:
        """Execute unstake transaction."""
        # Deduct gas from staker
        if not self.state.sub_balance(tx.from_address, gas_cost):
            logger.error("Failed to deduct gas")
            return False
        
        # Subtract from staked amount
        if not self.state.sub_stake(tx.validator_address, tx.amount):
            logger.error("Failed to subtract stake")
            return False
        
        # Return staked amount to staker
        self.state.add_balance(tx.from_address, tx.amount)
        
        logger.info(
            f"Executed unstake: {tx.amount} from validator {tx.validator_address.hex()[:8]}..."
        )
        return True
    
    def _execute_claim_rewards(self, tx: StakingTransaction, gas_cost: int) -> bool:
        """Execute claim rewards transaction."""
        # Deduct gas
        if not self.state.sub_balance(tx.from_address, gas_cost):
            logger.error("Failed to deduct gas")
            return False
        
        # Claim rewards (transfers to validator balance)
        rewards_claimed = self.state.claim_rewards(tx.validator_address)
        
        logger.info(
            f"Executed claim rewards: {rewards_claimed} for validator {tx.validator_address.hex()[:8]}..."
        )
        return True
    
    def get_staking_info(self, validator_address: bytes) -> Dict:
        """
        Get staking information for a validator.
        
        Args:
            validator_address: Validator address
            
        Returns:
            Dict: Staking information
        """
        return {
            "address": validator_address.hex(),
            "staked_amount": self.state.get_staked_amount(validator_address),
            "pending_rewards": self.state.get_pending_rewards(validator_address),
            "validator_info": self.state.get_validator_info(validator_address),
        }
