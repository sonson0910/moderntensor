"""
Axon Information Model

Represents Axon server endpoint information.
"""

from typing import Optional
from pydantic import BaseModel, Field


class AxonInfo(BaseModel):
    """Axon server endpoint information."""
    
    # Network Info
    ip: str = Field(..., description="IP address")
    port: int = Field(..., description="Port number", ge=1, le=65535)
    ip_type: int = Field(default=4, description="IP type (4 or 6)", ge=4, le=6)
    
    # Protocol
    protocol: int = Field(default=4, description="Protocol version", ge=4)
    
    # Identity
    hotkey: str = Field(..., description="Hotkey address")
    coldkey: str = Field(..., description="Coldkey address")
    
    # Version
    version: int = Field(default=0, description="Axon version", ge=0)
    
    # Placeholder
    placeholder1: int = Field(default=0, description="Placeholder field 1")
    placeholder2: int = Field(default=0, description="Placeholder field 2")
    
    class Config:
        json_schema_extra = {
            "example": {
                "ip": "192.168.1.100",
                "port": 8091,
                "ip_type": 4,
                "protocol": 4,
                "hotkey": "5C4hrfjw9DjXZTzV3MwzrrAr9P1MJhSrvWGWqi1eSuyUpnhM",
                "coldkey": "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
                "version": 1,
                "placeholder1": 0,
                "placeholder2": 0
            }
        }
    
    @property
    def endpoint(self) -> str:
        """Get full endpoint URL."""
        return f"http://{self.ip}:{self.port}"
    
    def __str__(self) -> str:
        return f"AxonInfo(ip={self.ip}, port={self.port})"
    
    def __repr__(self) -> str:
        return self.__str__()
