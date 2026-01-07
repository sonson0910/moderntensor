"""
Miner AI Agent - Clean AI task handling for miners.

Separates AI/ML concerns from blockchain logic.
"""

import logging
from typing import Optional, Dict, Any

from ..core.protocol import Task, Result, TaskContext
from ..subnets.base import BaseSubnet

logger = logging.getLogger(__name__)


class MinerAIAgent:
    """
    AI agent for miners - handles AI/ML task solving.
    
    Clean separation of concerns:
    - AI task solving (this class)
    - Blockchain operations (separate MinerAgent)
    - Network communication (separate)
    
    Example:
        agent = MinerAIAgent(subnet=text_generation_subnet)
        agent.setup()
        
        # Solve AI task
        result = agent.solve_task(task)
    """
    
    def __init__(
        self,
        subnet: BaseSubnet,
        miner_uid: str,
        config: Optional[Dict[str, Any]] = None,
    ):
        """
        Initialize miner AI agent.
        
        Args:
            subnet: Subnet protocol implementation
            miner_uid: Unique miner identifier
            config: Optional configuration
        """
        self.subnet = subnet
        self.miner_uid = miner_uid
        self.config = config or {}
        self.is_ready = False
        
        logger.info(f"MinerAIAgent initialized for miner {miner_uid}")
    
    def setup(self) -> None:
        """Setup the agent and subnet"""
        logger.info("Setting up MinerAIAgent...")
        
        # Setup subnet
        self.subnet.setup()
        
        self.is_ready = True
        logger.info("MinerAIAgent ready")
    
    def solve_task(self, task: Task) -> Result:
        """
        Solve an AI task.
        
        Args:
            task: Task to solve
        
        Returns:
            Result from solving the task
        """
        if not self.is_ready:
            raise RuntimeError("Agent not setup. Call setup() first.")
        
        logger.debug(f"Solving task {task.task_id}")
        
        # Use subnet to solve task
        result = self.subnet.solve_task(task)
        
        logger.debug(f"Task {task.task_id} solved")
        
        return result
    
    def get_metrics(self) -> Dict[str, Any]:
        """Get agent performance metrics"""
        return {
            "miner_uid": self.miner_uid,
            "is_ready": self.is_ready,
            "subnet_metrics": self.subnet.get_metrics() if self.is_ready else {},
        }
    
    def teardown(self) -> None:
        """Cleanup agent resources"""
        if self.is_ready:
            self.subnet.teardown()
            self.is_ready = False
            logger.info("MinerAIAgent torn down")
