"""
Subnet Templates

Provides template classes and scaffolding for creating new subnets.
"""

import logging
from typing import Optional, Dict, Any, List
from abc import ABC, abstractmethod


logger = logging.getLogger(__name__)


class SubnetTemplate(ABC):
    """
    Base template for creating ModernTensor subnets.
    
    This class provides a structured way to create new subnets with
    all necessary components: validators, miners, and scoring logic.
    
    Example:
        ```python
        from sdk.dev_framework import SubnetTemplate
        
        class MySubnet(SubnetTemplate):
            def __init__(self):
                super().__init__(
                    name="My AI Subnet",
                    version="1.0.0"
                )
            
            def validate(self, response):
                # Implement validation logic
                return score
            
            def score(self, responses):
                # Implement scoring logic
                return scores
        
        subnet = MySubnet()
        subnet.initialize()
        ```
    """
    
    def __init__(
        self,
        name: str,
        version: str = "1.0.0",
        description: str = "",
    ):
        """
        Initialize subnet template.
        
        Args:
            name: Subnet name
            version: Subnet version
            description: Subnet description
        """
        self.name = name
        self.version = version
        self.description = description
        
        self.config: Dict[str, Any] = {}
        self.validators: List[Any] = []
        self.miners: List[Any] = []
        
        logger.info(f"Initialized subnet template: {name} v{version}")
    
    @abstractmethod
    def validate(self, response: Any) -> float:
        """
        Validate a miner's response.
        
        Args:
            response: Response from miner
            
        Returns:
            Validation score (0-1)
        """
        pass
    
    @abstractmethod
    def score(self, responses: List[Any]) -> List[float]:
        """
        Score multiple miner responses.
        
        Args:
            responses: List of responses from miners
            
        Returns:
            List of scores (0-1)
        """
        pass
    
    def initialize(self):
        """Initialize the subnet."""
        logger.info(f"Initializing subnet: {self.name}")
        self._setup_validators()
        self._setup_miners()
        logger.info(f"Subnet {self.name} initialized successfully")
    
    def _setup_validators(self):
        """Setup validators (can be overridden)."""
        pass
    
    def _setup_miners(self):
        """Setup miners (can be overridden)."""
        pass
    
    def get_config(self) -> Dict[str, Any]:
        """Get subnet configuration."""
        return {
            "name": self.name,
            "version": self.version,
            "description": self.description,
            "validators": len(self.validators),
            "miners": len(self.miners),
            **self.config
        }
    
    def __str__(self) -> str:
        return f"SubnetTemplate({self.name} v{self.version})"
    
    def __repr__(self) -> str:
        return self.__str__()


class TextPromptingTemplate(SubnetTemplate):
    """
    Template for text prompting subnets.
    
    Pre-configured template for LLM-based text generation subnets.
    """
    
    def __init__(self):
        super().__init__(
            name="Text Prompting Subnet",
            version="1.0.0",
            description="Subnet for LLM text generation and prompting"
        )
    
    def validate(self, response: str) -> float:
        """Validate text response quality."""
        if not response:
            return 0.0
        
        # Basic validation: check length and content
        score = min(len(response) / 1000, 1.0)  # Prefer longer responses
        
        return score
    
    def score(self, responses: List[str]) -> List[float]:
        """Score multiple text responses."""
        return [self.validate(r) for r in responses]


class ImageGenerationTemplate(SubnetTemplate):
    """
    Template for image generation subnets.
    
    Pre-configured template for image generation (Stable Diffusion, etc.).
    """
    
    def __init__(self):
        super().__init__(
            name="Image Generation Subnet",
            version="1.0.0",
            description="Subnet for AI image generation"
        )
    
    def validate(self, response: bytes) -> float:
        """Validate image quality."""
        if not response:
            return 0.0
        
        # Basic validation: check size
        score = min(len(response) / 100000, 1.0)
        
        return score
    
    def score(self, responses: List[bytes]) -> List[float]:
        """Score multiple image responses."""
        return [self.validate(r) for r in responses]


__all__ = [
    "SubnetTemplate",
    "TextPromptingTemplate",
    "ImageGenerationTemplate",
]
