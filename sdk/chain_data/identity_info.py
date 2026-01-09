"""
Identity Information Model

Represents on-chain identity and metadata for accounts.
"""

from typing import Optional, Dict, List
from pydantic import BaseModel, Field, HttpUrl


class IdentityInfo(BaseModel):
    """
    On-chain identity information.
    
    Stores verified identity data for accounts, including
    display name, social media links, and verification status.
    """
    
    # Account
    account: str = Field(..., description="Account address")
    
    # Basic Info
    display_name: Optional[str] = Field(default=None, description="Display name")
    legal_name: Optional[str] = Field(default=None, description="Legal name")
    
    # Contact
    email: Optional[str] = Field(default=None, description="Email address")
    web: Optional[str] = Field(default=None, description="Website URL")
    
    # Social Media
    twitter: Optional[str] = Field(default=None, description="Twitter handle")
    riot: Optional[str] = Field(default=None, description="Matrix/Riot ID")
    github: Optional[str] = Field(default=None, description="GitHub username")
    discord: Optional[str] = Field(default=None, description="Discord username")
    
    # Verification
    verified: bool = Field(default=False, description="Whether identity is verified")
    verification_level: int = Field(
        default=0,
        description="Verification level (0=none, 1=basic, 2=full)",
        ge=0,
        le=2
    )
    
    # Additional Data
    additional_fields: Dict[str, str] = Field(
        default_factory=dict,
        description="Additional custom fields"
    )
    
    # Metadata
    created_block: int = Field(default=0, description="Block when identity was created", ge=0)
    updated_block: int = Field(default=0, description="Block when identity was last updated", ge=0)
    
    # Judgements (verification by registrars)
    judgements: List[Dict[str, str]] = Field(
        default_factory=list,
        description="Verification judgements from registrars"
    )
    
    class Config:
        json_schema_extra = {
            "example": {
                "account": "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
                "display_name": "ModernTensor Validator",
                "legal_name": None,
                "email": "validator@moderntensor.io",
                "web": "https://moderntensor.io",
                "twitter": "@moderntensor",
                "github": "moderntensor",
                "discord": "moderntensor#1234",
                "verified": True,
                "verification_level": 2,
                "additional_fields": {
                    "location": "Vietnam"
                },
                "created_block": 12345,
                "updated_block": 12500,
                "judgements": [
                    {
                        "registrar": "Official",
                        "judgement": "Reasonable"
                    }
                ]
            }
        }
    
    def __str__(self) -> str:
        name = self.display_name or self.account[:8] + "..."
        verified_str = " (verified)" if self.verified else ""
        return f"IdentityInfo({name}{verified_str})"
    
    def __repr__(self) -> str:
        return self.__str__()
