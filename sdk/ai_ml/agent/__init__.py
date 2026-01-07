"""
AI/ML Agent Module - Clean separation of AI concerns from blockchain.

Provides specialized agents for handling AI/ML tasks:
- MinerAIAgent: Handles AI task solving for miners
- ValidatorAIAgent: Handles AI task validation for validators
"""

from .miner_ai_agent import MinerAIAgent
from .validator_ai_agent import ValidatorAIAgent

__all__ = [
    "MinerAIAgent",
    "ValidatorAIAgent",
]
