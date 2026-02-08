"""
Block Information Model

Represents blockchain block information.
"""

from typing import List, Optional
from pydantic import BaseModel, Field


class BlockInfo(BaseModel):
    """Blockchain block information (matches luxtensor-core BlockHeader)."""

    # Block Identity
    block_number: int = Field(..., description="Block number (height)", ge=0)
    block_hash: str = Field(..., description="Block hash")
    parent_hash: str = Field(..., description="Parent block hash")
    version: int = Field(default=1, description="Block version")

    # Timestamp
    timestamp: int = Field(..., description="Block timestamp (Unix time)", ge=0)

    # Transactions
    transactions: List[str] = Field(default_factory=list, description="Transaction hashes in this block")
    transaction_count: int = Field(default=0, description="Number of transactions", ge=0)

    # State roots
    state_root: str = Field(..., description="State root hash")
    txs_root: str = Field(default="0x" + "00" * 32, description="Transactions root hash")
    receipts_root: str = Field(default="0x" + "00" * 32, description="Receipts root hash")

    # Validator
    author: Optional[str] = Field(default=None, description="Block author/validator address")
    signature: Optional[str] = Field(default=None, description="Block signature")

    # Gas
    gas_used: int = Field(default=0, description="Total gas used in block", ge=0)
    gas_limit: int = Field(default=10_000_000, description="Block gas limit", ge=0)

    # Extra
    extra_data: Optional[str] = Field(default=None, description="Extra data (hex)")

    class Config:
        json_schema_extra = {
            "example": {
                "block_number": 12345,
                "block_hash": "0x1234567890abcdef",
                "parent_hash": "0xabcdef1234567890",
                "version": 1,
                "timestamp": 1704643200,
                "transactions": ["0xabc123", "0xdef456"],
                "transaction_count": 2,
                "state_root": "0xstateroot123",
                "txs_root": "0xtxsroot456",
                "receipts_root": "0xreceiptsroot789",
                "author": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2",
                "gas_used": 42000,
                "gas_limit": 10000000
            }
        }

    def __str__(self) -> str:
        return f"BlockInfo(number={self.block_number}, txs={self.transaction_count})"

    def __repr__(self) -> str:
        return self.__str__()
