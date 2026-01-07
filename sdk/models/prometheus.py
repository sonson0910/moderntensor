"""
Prometheus Information Model

Represents Prometheus metrics endpoint information.
"""

from pydantic import BaseModel, Field


class PrometheusInfo(BaseModel):
    """Prometheus metrics endpoint information."""
    
    # Network Info
    ip: str = Field(..., description="IP address")
    port: int = Field(..., description="Port number", ge=1, le=65535)
    ip_type: int = Field(default=4, description="IP type (4 or 6)", ge=4, le=6)
    
    # Version
    version: int = Field(default=0, description="Prometheus version", ge=0)
    
    # Block
    block: int = Field(default=0, description="Block number", ge=0)
    
    class Config:
        json_schema_extra = {
            "example": {
                "ip": "192.168.1.100",
                "port": 9090,
                "ip_type": 4,
                "version": 1,
                "block": 12345
            }
        }
    
    @property
    def endpoint(self) -> str:
        """Get full endpoint URL."""
        return f"http://{self.ip}:{self.port}/metrics"
    
    def __str__(self) -> str:
        return f"PrometheusInfo(ip={self.ip}, port={self.port})"
    
    def __repr__(self) -> str:
        return self.__str__()
