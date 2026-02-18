"""
Transaction Information Model

Represents transaction information.
"""

from typing import Optional, Dict, Any
from pydantic import BaseModel, Field


class TransactionInfo(BaseModel):
    """Transaction information (matches luxtensor-core Transaction struct)."""

    # Transaction Identity
    tx_hash: str = Field(..., description="Transaction hash")
    block_number: int = Field(..., description="Block number", ge=0)
    block_hash: str = Field(..., description="Block hash")

    # Addresses
    from_address: str = Field(..., description="Sender address")
    to_address: Optional[str] = Field(default=None, description="Recipient address (None for contract creation)")

    # Value & Gas (EVM-style, matching Rust Transaction struct)
    value: int = Field(default=0, description="Value transferred in base units", ge=0)
    gas_price: int = Field(default=50, description="Gas price in base units", ge=0)
    gas_limit: int = Field(default=21000, description="Gas limit", ge=0)
    gas_used: int = Field(default=0, description="Actual gas used", ge=0)

    # Chain & Nonce
    chain_id: int = Field(default=8898, description="Chain ID (8898=mainnet, 9999=testnet, 8898=devnet)")
    nonce: int = Field(default=0, description="Sender nonce", ge=0)

    # Data
    data: str = Field(default="0x", description="Transaction data (hex)")

    # Status
    success: bool = Field(..., description="Whether transaction was successful")

    # Signature
    v: int = Field(default=0, description="Signature recovery id")
    r: Optional[str] = Field(default=None, description="Signature r component")
    s: Optional[str] = Field(default=None, description="Signature s component")

    # Timestamp
    timestamp: int = Field(..., description="Transaction timestamp", ge=0)

    class Config:
        json_schema_extra = {
            "example": {
                "tx_hash": "0x1234567890abcdef",
                "block_number": 12345,
                "block_hash": "0xabcdef1234567890",
                "from_address": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2",
                "to_address": "0x892d35Cc6634C0532925a3b844Bc9e7595f0cCd3",
                "value": 1000000000000000000,
                "gas_price": 50,
                "gas_limit": 21000,
                "gas_used": 21000,
                "chain_id": 8898,
                "nonce": 5,
                "data": "0x",
                "success": True,
                "v": 27,
                "r": "0xabc123",
                "s": "0xdef456",
                "timestamp": 1704643200
            }
        }

    def __str__(self) -> str:
        status = "✓" if self.success else "✗"
        to = self.to_address[:10] + "..." if self.to_address else "contract_create"
        return f"TransactionInfo({status} {self.from_address[:10]}...→{to}, value={self.value}, block={self.block_number})"

    def __repr__(self) -> str:
        return self.__str__()
