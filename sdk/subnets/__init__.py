"""
DEPRECATED: Old subnet module.

Use sdk.ai_ml instead for AI/ML subnet functionality.
This module is kept only for backward compatibility.
"""

# Redirect to new AI/ML module
from sdk.ai_ml.core.protocol import SubnetProtocol

__all__ = ["SubnetProtocol"]
