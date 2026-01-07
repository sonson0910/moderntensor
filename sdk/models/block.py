"""
Block Information Model

Represents blockchain block information.
"""

from typing import List, Optional
from pydantic import BaseModel, Field


class BlockInfo(BaseModel):
    """Blockchain block information."""
    
    # Block Identity
    block_number: int = Field(..., description="Block number", ge=0)
    block_hash: str = Field(..., description="Block hash")
    parent_hash: str = Field(..., description="Parent block hash")
    
    # Timestamp
    timestamp: int = Field(..., description="Block timestamp (Unix time)", ge=0)
    
    # Transactions
    transactions: List[str] = Field(default_factory=list, description="Transaction hashes in this block")
    transaction_count: int = Field(default=0, description="Number of transactions", ge=0)
    
    # State
    state_root: str = Field(..., description="State root hash")
    extrinsics_root: str = Field(..., description="Extrinsics root hash")
    
    # Validator
    author: Optional[str] = Field(default=None, description="Block author/validator address")
    
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
                "extrinsics_root": "0xextroot456",
                "author": "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"
            }
        }
        
    def __str__(self) -> str:
        return f"BlockInfo(number={self.block_number}, txs={self.transaction_count})"
    
    def __repr__(self) -> str:
        return self.__str__()
