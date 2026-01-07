"""
Transaction Information Model

Represents transaction information.
"""

from typing import Optional, Dict, Any
from pydantic import BaseModel, Field


class TransactionInfo(BaseModel):
    """Transaction information."""
    
    # Transaction Identity
    tx_hash: str = Field(..., description="Transaction hash")
    block_number: int = Field(..., description="Block number", ge=0)
    block_hash: str = Field(..., description="Block hash")
    
    # Transaction Details
    from_address: str = Field(..., description="Sender address")
    to_address: Optional[str] = Field(default=None, description="Recipient address (None for some tx types)")
    
    # Transaction Type
    method: str = Field(..., description="Transaction method/call")
    pallet: str = Field(..., description="Pallet name")
    
    # Status
    success: bool = Field(..., description="Whether transaction was successful")
    
    # Fees
    fee: float = Field(default=0.0, description="Transaction fee paid", ge=0)
    
    # Data
    args: Dict[str, Any] = Field(default_factory=dict, description="Transaction arguments")
    
    # Metadata
    nonce: int = Field(default=0, description="Sender nonce", ge=0)
    signature: Optional[str] = Field(default=None, description="Transaction signature")
    
    # Timestamp
    timestamp: int = Field(..., description="Transaction timestamp", ge=0)
    
    class Config:
        json_schema_extra = {
            "example": {
                "tx_hash": "0x1234567890abcdef",
                "block_number": 12345,
                "block_hash": "0xabcdef1234567890",
                "from_address": "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
                "to_address": "5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty",
                "method": "transfer",
                "pallet": "balances",
                "success": True,
                "fee": 0.01,
                "args": {"amount": 100.0},
                "nonce": 5,
                "timestamp": 1704643200
            }
        }
        
    def __str__(self) -> str:
        status = "✓" if self.success else "✗"
        return f"TransactionInfo({status} {self.pallet}.{self.method}, block={self.block_number})"
    
    def __repr__(self) -> str:
        return self.__str__()
