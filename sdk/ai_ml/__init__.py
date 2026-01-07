"""
ModernTensor AI/ML Layer

Clean, production-ready AI/ML infrastructure with proper separation of concerns.
"""

__version__ = "0.1.0"

# Core exports
from .core.protocol import SubnetProtocol, TaskContext, Task, Result, Score

__all__ = [
    "SubnetProtocol",
    "TaskContext",
    "Task",
    "Result", 
    "Score",
]
