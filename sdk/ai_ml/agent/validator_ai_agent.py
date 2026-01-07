"""
Validator AI Agent - Clean AI task validation for validators.

Separates AI/ML concerns from blockchain logic.
"""

import logging
from typing import Optional, Dict, Any, List

from ..core.protocol import Task, Result, Score, TaskContext
from ..subnets.base import BaseSubnet
from ..scoring import AdvancedScorer, ConsensusAggregator, ValidatorScore

logger = logging.getLogger(__name__)


class ValidatorAIAgent:
    """
    AI agent for validators - handles AI/ML task creation and scoring.
    
    Clean separation of concerns:
    - AI task creation and scoring (this class)
    - Blockchain operations (separate ValidatorAgent)
    - Consensus aggregation (separate)
    
    Example:
        agent = ValidatorAIAgent(
            subnet=text_generation_subnet,
            scorer=advanced_scorer,
        )
        agent.setup()
        
        # Create task
        task = agent.create_task(context)
        
        # Score result
        score = agent.score_result(task, result)
    """
    
    def __init__(
        self,
        subnet: BaseSubnet,
        validator_uid: str,
        scorer: Optional[AdvancedScorer] = None,
        config: Optional[Dict[str, Any]] = None,
    ):
        """
        Initialize validator AI agent.
        
        Args:
            subnet: Subnet protocol implementation
            validator_uid: Unique validator identifier
            scorer: Optional advanced scorer (uses subnet's default if None)
            config: Optional configuration
        """
        self.subnet = subnet
        self.validator_uid = validator_uid
        self.scorer = scorer
        self.config = config or {}
        self.is_ready = False
        
        logger.info(f"ValidatorAIAgent initialized for validator {validator_uid}")
    
    def setup(self) -> None:
        """Setup the agent and subnet"""
        logger.info("Setting up ValidatorAIAgent...")
        
        # Setup subnet
        self.subnet.setup()
        
        self.is_ready = True
        logger.info("ValidatorAIAgent ready")
    
    def create_task(self, context: TaskContext) -> Task:
        """
        Create an AI task.
        
        Args:
            context: Task context
        
        Returns:
            Created task
        """
        if not self.is_ready:
            raise RuntimeError("Agent not setup. Call setup() first.")
        
        logger.debug(f"Creating task for miner {context.miner_uid}")
        
        # Use subnet to create task
        task = self.subnet.create_task(context)
        
        logger.debug(f"Task {task.task_id} created")
        
        return task
    
    def score_result(self, task: Task, result: Result) -> Score:
        """
        Score a result.
        
        Args:
            task: Original task
            result: Result to score
        
        Returns:
            Score
        """
        if not self.is_ready:
            raise RuntimeError("Agent not setup. Call setup() first.")
        
        logger.debug(f"Scoring result for task {task.task_id}")
        
        # Use custom scorer if available, otherwise use subnet's scorer
        if self.scorer:
            score = self.scorer.score(task, result)
        else:
            score = self.subnet.score_result(task, result)
        
        # Add validator UID to score
        score.validator_uid = self.validator_uid
        
        logger.debug(f"Result scored: {score.value:.3f}")
        
        return score
    
    def aggregate_consensus(
        self,
        validator_scores: List[ValidatorScore],
        aggregator: ConsensusAggregator,
    ) -> Score:
        """
        Aggregate scores from multiple validators.
        
        Args:
            validator_scores: List of scores from validators
            aggregator: Consensus aggregator
        
        Returns:
            Consensus score
        """
        logger.debug(f"Aggregating consensus from {len(validator_scores)} validators")
        
        consensus = aggregator.aggregate(validator_scores)
        
        logger.debug(f"Consensus reached: {consensus.value:.3f}")
        
        return consensus
    
    def get_metrics(self) -> Dict[str, Any]:
        """Get agent performance metrics"""
        return {
            "validator_uid": self.validator_uid,
            "is_ready": self.is_ready,
            "has_custom_scorer": self.scorer is not None,
            "subnet_metrics": self.subnet.get_metrics() if self.is_ready else {},
        }
    
    def teardown(self) -> None:
        """Cleanup agent resources"""
        if self.is_ready:
            self.subnet.teardown()
            self.is_ready = False
            logger.info("ValidatorAIAgent torn down")
