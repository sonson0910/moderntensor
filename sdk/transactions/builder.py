"""
Transaction Builder

Provides fluent API for building transactions.
"""

import logging
from typing import Optional, Dict, Any, List
from sdk.transactions.types import (
    TransactionType,
    TransferTransaction,
    StakeTransaction,
    UnstakeTransaction,
    RegisterTransaction,
    WeightTransaction,
    ProposalTransaction,
    VoteTransaction,
    DelegateTransaction,
    ServeAxonTransaction,
    SwapHotkeyTransaction,
)

logger = logging.getLogger(__name__)


class TransactionBuilder:
    """
    Fluent builder for constructing transactions.
    
    Example:
        ```python
        tx = TransactionBuilder()\\
            .transfer(from_addr="addr1", to_addr="addr2", amount=100.0)\\
            .with_fee(0.01)\\
            .with_memo("Payment for services")\\
            .build()
        ```
    """
    
    def __init__(self):
        self._transaction_data = {}
        self._transaction_type = None
        
    def transfer(
        self,
        from_address: str,
        to_address: str,
        amount: float
    ) -> "TransactionBuilder":
        """Build a transfer transaction."""
        self._transaction_type = TransferTransaction
        self._transaction_data = {
            "from_address": from_address,
            "to_address": to_address,
            "amount": amount,
        }
        return self
    
    def stake(
        self,
        from_address: str,
        hotkey: str,
        amount: float,
        subnet_id: Optional[int] = None
    ) -> "TransactionBuilder":
        """Build a stake transaction."""
        self._transaction_type = StakeTransaction
        self._transaction_data = {
            "from_address": from_address,
            "hotkey": hotkey,
            "amount": amount,
            "subnet_id": subnet_id,
        }
        return self
    
    def unstake(
        self,
        from_address: str,
        hotkey: str,
        amount: float,
        subnet_id: Optional[int] = None
    ) -> "TransactionBuilder":
        """Build an unstake transaction."""
        self._transaction_type = UnstakeTransaction
        self._transaction_data = {
            "from_address": from_address,
            "hotkey": hotkey,
            "amount": amount,
            "subnet_id": subnet_id,
        }
        return self
    
    def register(
        self,
        from_address: str,
        subnet_id: int,
        hotkey: str,
        metadata: Optional[Dict[str, Any]] = None
    ) -> "TransactionBuilder":
        """Build a registration transaction."""
        self._transaction_type = RegisterTransaction
        self._transaction_data = {
            "from_address": from_address,
            "subnet_id": subnet_id,
            "hotkey": hotkey,
            "metadata": metadata,
        }
        return self
    
    def set_weights(
        self,
        from_address: str,
        subnet_id: int,
        uids: List[int],
        weights: List[float],
        version_key: int
    ) -> "TransactionBuilder":
        """Build a weight setting transaction."""
        self._transaction_type = WeightTransaction
        self._transaction_data = {
            "from_address": from_address,
            "subnet_id": subnet_id,
            "uids": uids,
            "weights": weights,
            "version_key": version_key,
        }
        return self
    
    def propose(
        self,
        from_address: str,
        title: str,
        description: str,
        proposal_type: str,
        options: List[str],
        duration_blocks: int
    ) -> "TransactionBuilder":
        """Build a proposal transaction."""
        self._transaction_type = ProposalTransaction
        self._transaction_data = {
            "from_address": from_address,
            "title": title,
            "description": description,
            "proposal_type": proposal_type,
            "options": options,
            "duration_blocks": duration_blocks,
        }
        return self
    
    def vote(
        self,
        from_address: str,
        proposal_id: int,
        option: str,
        voting_power: Optional[float] = None
    ) -> "TransactionBuilder":
        """Build a vote transaction."""
        self._transaction_type = VoteTransaction
        self._transaction_data = {
            "from_address": from_address,
            "proposal_id": proposal_id,
            "option": option,
            "voting_power": voting_power,
        }
        return self
    
    def delegate(
        self,
        from_address: str,
        validator_hotkey: str,
        amount: float
    ) -> "TransactionBuilder":
        """Build a delegation transaction."""
        self._transaction_type = DelegateTransaction
        self._transaction_data = {
            "from_address": from_address,
            "validator_hotkey": validator_hotkey,
            "amount": amount,
        }
        return self
    
    def serve_axon(
        self,
        from_address: str,
        subnet_id: int,
        ip: str,
        port: int,
        protocol: str = "http",
        version: int = 1
    ) -> "TransactionBuilder":
        """Build a serve axon transaction."""
        self._transaction_type = ServeAxonTransaction
        self._transaction_data = {
            "from_address": from_address,
            "subnet_id": subnet_id,
            "ip": ip,
            "port": port,
            "protocol": protocol,
            "version": version,
        }
        return self
    
    def swap_hotkey(
        self,
        from_address: str,
        subnet_id: int,
        old_hotkey: str,
        new_hotkey: str
    ) -> "TransactionBuilder":
        """Build a hotkey swap transaction."""
        self._transaction_type = SwapHotkeyTransaction
        self._transaction_data = {
            "from_address": from_address,
            "subnet_id": subnet_id,
            "old_hotkey": old_hotkey,
            "new_hotkey": new_hotkey,
        }
        return self
    
    def with_nonce(self, nonce: int) -> "TransactionBuilder":
        """Set transaction nonce."""
        self._transaction_data["nonce"] = nonce
        return self
    
    def with_fee(self, fee: float) -> "TransactionBuilder":
        """Set transaction fee."""
        self._transaction_data["fee"] = fee
        return self
    
    def with_memo(self, memo: str) -> "TransactionBuilder":
        """Set transaction memo."""
        self._transaction_data["memo"] = memo
        return self
    
    def build(self):
        """
        Build and validate the transaction.
        
        Returns:
            Transaction: The constructed transaction object
            
        Raises:
            ValueError: If transaction type not set or validation fails
        """
        if self._transaction_type is None:
            raise ValueError("Transaction type not set. Call a transaction method first.")
        
        try:
            transaction = self._transaction_type(**self._transaction_data)
            logger.debug(f"Built transaction: {transaction.transaction_type}")
            return transaction
        except Exception as e:
            logger.error(f"Failed to build transaction: {e}")
            raise
    
    def reset(self) -> "TransactionBuilder":
        """Reset the builder to create a new transaction."""
        self._transaction_data = {}
        self._transaction_type = None
        return self
