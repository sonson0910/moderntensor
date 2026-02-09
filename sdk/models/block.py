"""
Block Information Model

Represents blockchain block information.
"""

from typing import List, Optional
from pydantic import BaseModel, Field


class BlockInfo(BaseModel):
    """Blockchain block information (matches luxtensor-rpc RpcBlock)."""

    # Block Identity
    block_number: int = Field(..., description="Block number (height)", ge=0)
    block_hash: str = Field(..., description="Block hash")
    parent_hash: str = Field(..., description="Parent block hash")

    # Timestamp
    timestamp: int = Field(..., description="Block timestamp (Unix time)", ge=0)

    # Transactions
    transactions: List[str] = Field(default_factory=list, description="Transaction hashes in this block")
    transaction_count: int = Field(default=0, description="Number of transactions", ge=0)

    # State
    state_root: str = Field(default="0x" + "00" * 32, description="State root hash")

    # Gas
    gas_used: int = Field(default=0, description="Total gas used in block", ge=0)
    gas_limit: int = Field(default=10_000_000, description="Block gas limit", ge=0)

    # Optional fields (may be absent in the Rust response)
    version: Optional[int] = Field(default=None, description="Block version")
    author: Optional[str] = Field(default=None, description="Block author/validator address")

    @classmethod
    def from_rpc_response(cls, data: dict) -> "BlockInfo":
        """Create BlockInfo from LuxTensor RPC response (hex-encoded fields)."""
        def hex_to_int(val, default=0):
            if val is None:
                return default
            if isinstance(val, int):
                return val
            if isinstance(val, str) and val.startswith("0x"):
                return int(val, 16)
            return int(val)

        txs = data.get("transactions", [])
        return cls(
            block_number=hex_to_int(data.get("number")),
            block_hash=data.get("hash", ""),
            parent_hash=data.get("parent_hash", ""),
            timestamp=hex_to_int(data.get("timestamp")),
            transactions=txs,
            transaction_count=len(txs),
            state_root=data.get("state_root", "0x" + "00" * 32),
            gas_used=hex_to_int(data.get("gas_used")),
            gas_limit=hex_to_int(data.get("gas_limit")),
        )

    class Config:
        json_schema_extra = {
            "example": {
                "block_number": 12345,
                "block_hash": "0x1234567890abcdef",
                "parent_hash": "0xabcdef1234567890",
                "timestamp": 1704643200,
                "transactions": ["0xabc123", "0xdef456"],
                "transaction_count": 2,
                "state_root": "0xstateroot123",
                "gas_used": 42000,
                "gas_limit": 10000000
            }
        }

    def __str__(self) -> str:
        return f"BlockInfo(number={self.block_number}, txs={self.transaction_count})"

    def __repr__(self) -> str:
        return self.__str__()
