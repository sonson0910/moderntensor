"""
Model management for AI/ML subnets.

Provides advanced model lifecycle management that surpasses Bittensor:
- Model versioning and experiment tracking
- Model loading and caching
- Distributed model serving
- Model performance monitoring
"""

from .manager import ModelManager, ModelMetadata, ModelVersion

__all__ = [
    "ModelManager",
    "ModelMetadata",
    "ModelVersion",
]

