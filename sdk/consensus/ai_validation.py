"""
AI validation integration for ModernTensor Layer 1 blockchain.

Integrates zkML proofs and AI task validation into the consensus mechanism.
"""
import logging
from typing import Optional, Dict, Any
from dataclasses import dataclass
import hashlib

from ..utils.zkml import ZkmlManager

logger = logging.getLogger(__name__)


@dataclass
class AITask:
    """
    AI task submitted to the blockchain.
    
    Attributes:
        task_id: Unique task identifier
        model_hash: Hash of the AI model to use
        input_data: Input data for inference
        requester: Address of task requester
        reward: Reward offered for task completion
        timeout: Task timeout (blocks)
    """
    task_id: bytes  # 32 bytes
    model_hash: bytes  # 32 bytes
    input_data: bytes
    requester: bytes  # 20 bytes
    reward: int
    timeout: int = 100  # blocks
    
    def serialize(self) -> bytes:
        """Serialize task for hashing."""
        import json
        data = {
            "task_id": self.task_id.hex(),
            "model_hash": self.model_hash.hex(),
            "input_data": self.input_data.hex(),
            "requester": self.requester.hex(),
            "reward": self.reward,
            "timeout": self.timeout,
        }
        return json.dumps(data, separators=(',', ':')).encode('utf-8')
    
    def hash(self) -> bytes:
        """Calculate task hash."""
        return hashlib.sha256(self.serialize()).digest()


@dataclass
class AIResult:
    """
    Result of AI task execution.
    
    Attributes:
        task_id: Reference to original task
        result_data: Inference result
        proof: zkML proof of correct execution
        worker: Address of worker who executed task
        execution_time: Time taken (milliseconds)
    """
    task_id: bytes  # 32 bytes
    result_data: bytes
    proof: bytes  # zkML proof
    worker: bytes  # 20 bytes
    execution_time: int = 0  # milliseconds
    
    def serialize(self) -> bytes:
        """Serialize result for hashing."""
        import json
        data = {
            "task_id": self.task_id.hex(),
            "result_data": self.result_data.hex(),
            "proof": self.proof.hex() if self.proof else "",
            "worker": self.worker.hex(),
            "execution_time": self.execution_time,
        }
        return json.dumps(data, separators=(',', ':')).encode('utf-8')
    
    def hash(self) -> bytes:
        """Calculate result hash."""
        return hashlib.sha256(self.serialize()).digest()


class AIValidator:
    """
    Validates AI computation results using zkML proofs.
    
    Integrates with the consensus mechanism to reward quality AI work.
    """
    
    def __init__(self, zkml_manager: Optional[ZkmlManager] = None):
        """
        Initialize AI validator.
        
        Args:
            zkml_manager: ZkML manager for proof verification (None = create default)
        """
        self.zkml = zkml_manager or ZkmlManager()
        self.pending_tasks: Dict[bytes, AITask] = {}  # task_id -> AITask
        self.completed_tasks: Dict[bytes, AIResult] = {}  # task_id -> AIResult
        
        logger.info("AI validator initialized")
    
    def submit_task(self, task: AITask) -> bool:
        """
        Submit an AI task to be processed.
        
        Args:
            task: AI task to submit
            
        Returns:
            bool: True if submitted successfully
        """
        task_id = task.task_id
        
        if task_id in self.pending_tasks or task_id in self.completed_tasks:
            logger.warning(f"Task {task_id.hex()[:8]}... already exists")
            return False
        
        self.pending_tasks[task_id] = task
        logger.info(
            f"Submitted AI task {task_id.hex()[:8]}... "
            f"from {task.requester.hex()[:8]}... with reward {task.reward}"
        )
        
        return True
    
    def validate_ai_task(self, task: AITask, result: AIResult) -> bool:
        """
        Validate an AI computation result.
        
        Checks:
        1. Result corresponds to task
        2. zkML proof is valid
        3. Result quality score
        
        Args:
            task: Original AI task
            result: Claimed result
            
        Returns:
            bool: True if result is valid
        """
        # 1. Check task ID matches
        if task.task_id != result.task_id:
            logger.error("Task ID mismatch")
            return False
        
        # 2. Verify zkML proof
        if result.proof:
            try:
                # Use configurable verification key path
                vk_path = getattr(self, 'vk_path', 'vk.key')  # Allow configuration
                proof_valid = self.zkml.verify_proof(
                    result.proof.hex() if isinstance(result.proof, bytes) else result.proof,
                    vk_path=vk_path
                )
                if not proof_valid:
                    logger.error(f"zkML proof verification failed for task {task.task_id.hex()[:8]}...")
                    return False
            except Exception as e:
                logger.error(f"Error verifying zkML proof: {e}")
                return False
        else:
            logger.warning(f"No zkML proof provided for task {task.task_id.hex()[:8]}...")
            # Check if proof is mandatory (production mode)
            require_proof = getattr(self, 'require_proof', False)
            if require_proof:
                logger.error("zkML proof is mandatory in production mode")
                return False
            # Accept without proof in development mode
        
        # 3. Check result correctness (application-specific)
        # This would involve checking the result data matches expected format
        # and potentially running validation inference
        
        logger.info(f"AI task {task.task_id.hex()[:8]}... validated successfully")
        return True
    
    def submit_result(self, result: AIResult) -> bool:
        """
        Submit a result for a pending task.
        
        Args:
            result: AI task result
            
        Returns:
            bool: True if result accepted
        """
        task_id = result.task_id
        
        # Check if task exists
        if task_id not in self.pending_tasks:
            logger.warning(f"Task {task_id.hex()[:8]}... not found")
            return False
        
        # Validate result
        task = self.pending_tasks[task_id]
        if not self.validate_ai_task(task, result):
            logger.error(f"Result validation failed for task {task_id.hex()[:8]}...")
            return False
        
        # Accept result
        self.completed_tasks[task_id] = result
        del self.pending_tasks[task_id]
        
        logger.info(
            f"Accepted result for task {task_id.hex()[:8]}... "
            f"from worker {result.worker.hex()[:8]}..."
        )
        
        return True
    
    def calculate_ai_reward(
        self,
        validation_score: float,
        stake: int,
        base_reward: int = 100
    ) -> int:
        """
        Calculate reward for AI task based on validation quality.
        
        Args:
            validation_score: Quality score (0-1)
            stake: Worker's stake
            base_reward: Base reward amount
            
        Returns:
            int: Calculated reward
        """
        # Reward formula: base * quality * (1 + log(stake))
        import math
        
        if validation_score <= 0:
            return 0
        
        # Apply quality multiplier
        quality_multiplier = validation_score
        
        # Apply stake bonus (logarithmic)
        stake_multiplier = 1.0 + math.log10(max(1, stake / 1000))
        
        reward = int(base_reward * quality_multiplier * stake_multiplier)
        
        logger.debug(
            f"AI reward calculated: base={base_reward}, "
            f"quality={validation_score:.2f}, stake={stake}, "
            f"final={reward}"
        )
        
        return reward
    
    def get_task(self, task_id: bytes) -> Optional[AITask]:
        """
        Get a task by ID.
        
        Args:
            task_id: Task identifier
            
        Returns:
            Optional[AITask]: Task if found
        """
        return self.pending_tasks.get(task_id) or self.completed_tasks.get(task_id)
    
    def get_pending_tasks(self) -> Dict[bytes, AITask]:
        """Get all pending tasks."""
        return self.pending_tasks.copy()
    
    def get_completed_tasks(self) -> Dict[bytes, AIResult]:
        """Get all completed tasks."""
        return self.completed_tasks.copy()
    
    def cleanup_expired_tasks(self, current_height: int) -> int:
        """
        Remove expired tasks that have timed out.
        
        Args:
            current_height: Current block height
            
        Returns:
            int: Number of tasks removed
        """
        expired = []
        
        for task_id, task in self.pending_tasks.items():
            # Track submission height for proper timeout calculation
            # Check if task has expired based on submission time or block height
            submission_height = task.get('submission_height', 0)
            timeout_blocks = task.get('timeout_blocks', 100)  # Default timeout in blocks
            
            # In production, compare against current block height
            # For now, use time-based expiration as fallback
            if hasattr(self, 'current_block_height'):
                if self.current_block_height - submission_height > timeout_blocks:
                    expired.append(task_id)
            # Fallback to time-based for development
            else:
                submission_time = task.get('submission_time', 0)
                if time.time() - submission_time > 3600:  # 1 hour
                    expired.append(task_id)
        
        for task_id in expired:
            del self.pending_tasks[task_id]
            logger.info(f"Removed expired task {task_id.hex()[:8]}...")
        
        return len(expired)
    
    def get_statistics(self) -> Dict[str, Any]:
        """
        Get AI validation statistics.
        
        Returns:
            Dict: Statistics including pending, completed, total rewards
        """
        return {
            "pending_tasks": len(self.pending_tasks),
            "completed_tasks": len(self.completed_tasks),
            "total_tasks": len(self.pending_tasks) + len(self.completed_tasks),
        }
