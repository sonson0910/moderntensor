"""
ModernTensor AI/ML Layer

Clean, production-ready AI/ML infrastructure with proper separation of concerns.

Sub-modules:
    - ``core``       — SubnetProtocol, TaskContext, Task, Result, Score
    - ``scoring``    — Weight setting, reward scoring utilities
    - ``models``     — Model registry and persistence
    - ``processors`` — Input/output data processing pipelines
    - ``zkml``       — Zero-knowledge ML proof integration
    - ``agent``      — Autonomous AI agent primitives
    - ``subnets``    — Subnet lifecycle management

Usage::

    from sdk.ai_ml import SubnetProtocol, TaskContext

    class MyProtocol(SubnetProtocol):
        async def forward(self, ctx: TaskContext) -> Result:
            ...
"""

__version__ = "0.1.0"

# ── Core protocol ──────────────────────────────────────────────
from .core.protocol import SubnetProtocol, TaskContext, Task, Result, Score

# ── Scoring ────────────────────────────────────────────────────
from .scoring import AdvancedScorer, ScoringMethod, ConsensusAggregator

# ── Models ─────────────────────────────────────────────────────
from .models import ModelManager, ModelMetadata, ModelVersion

__all__ = [
    # Core
    "SubnetProtocol",
    "TaskContext",
    "Task",
    "Result",
    "Score",
    # Scoring
    "AdvancedScorer",
    "ScoringMethod",
    "ConsensusAggregator",
    # Models
    "ModelManager",
    "ModelMetadata",
    "ModelVersion",
]
