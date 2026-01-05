from abc import ABC, abstractmethod
from typing import Any, Dict, List


class SubnetProtocol(ABC):
    """
    Abstract Base Class defining the logic for a specific Subnet.
    Developers must implement this class to define their subnet's behavior.
    """

    @abstractmethod
    def create_task(self, miner_uid: str, difficulty: float) -> Dict[str, Any]:
        """
        Generates a task for a specific miner.

        Args:
            miner_uid: The UID of the target miner.
            difficulty: The current network difficulty.

        Returns:
            A dictionary containing the task data (e.g., prompt, params).
        """
        pass

    @abstractmethod
    def score_result(self, task_data: Any, result_data: Any) -> float:
        """
        Scores the result returned by a miner.

        Args:
            task_data: The original task data sent to the miner.
            result_data: The result data returned by the miner.

        Returns:
            A float score between 0.0 and 1.0.
        """
        pass

    @abstractmethod
    def solve_task(self, task_data: Any) -> Any:
        """
        Executes the task logic (Miner side).

        Args:
            task_data: The task data received from the validator.

        Returns:
            The result data to be sent back to the validator.
        """
        pass

    def get_metadata(self) -> Dict[str, Any]:
        """
        Returns metadata about this subnet logic.
        """
        return {
            "name": "Base Protocol",
            "version": "0.0.1",
            "description": "Abstract base protocol",
        }
