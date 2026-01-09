"""
Proxy Information Model

Represents proxy account relationships in the ModernTensor network.
"""

from typing import Optional, List
from pydantic import BaseModel, Field


class ProxyInfo(BaseModel):
    """
    Proxy account information.
    
    Proxies allow one account to perform operations on behalf of another,
    with specific permissions and restrictions.
    """
    
    # Identity
    proxy_account: str = Field(..., description="Proxy account address")
    delegator_account: str = Field(..., description="Delegator (owner) account address")
    
    # Permissions
    proxy_type: str = Field(
        default="Any",
        description="Type of proxy (Any, Staking, Transfer, Governance, etc.)"
    )
    
    # Restrictions
    delay_blocks: int = Field(
        default=0,
        description="Number of blocks to delay proxy actions",
        ge=0
    )
    
    # Status
    active: bool = Field(default=True, description="Whether proxy is active")
    
    # Metadata
    created_block: int = Field(default=0, description="Block when proxy was created", ge=0)
    expires_block: Optional[int] = Field(
        default=None,
        description="Block when proxy expires (None = never)",
    )
    
    class Config:
        json_schema_extra = {
            "example": {
                "proxy_account": "5C4hrfjw9DjXZTzV3MwzrrAr9P1MJhSrvWGWqi1eSuyUpnhM",
                "delegator_account": "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
                "proxy_type": "Staking",
                "delay_blocks": 0,
                "active": True,
                "created_block": 12345,
                "expires_block": None
            }
        }
    
    def __str__(self) -> str:
        return (
            f"ProxyInfo(proxy={self.proxy_account[:8]}..., "
            f"delegator={self.delegator_account[:8]}..., "
            f"type={self.proxy_type})"
        )
    
    def __repr__(self) -> str:
        return self.__str__()
