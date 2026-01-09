"""
Schedule Information Model

Represents scheduled operations on the blockchain.
"""

from typing import Optional, Dict, Any
from pydantic import BaseModel, Field
from datetime import datetime


class ScheduleInfo(BaseModel):
    """
    Scheduled operation information.
    
    Represents operations that are scheduled to execute at a future block.
    Used for delayed transactions, governance actions, and timed events.
    """
    
    # Identity
    schedule_id: str = Field(..., description="Unique schedule identifier")
    
    # Scheduling
    scheduled_block: int = Field(..., description="Block when operation will execute", ge=0)
    created_block: int = Field(..., description="Block when schedule was created", ge=0)
    
    # Operation Details
    operation_type: str = Field(..., description="Type of scheduled operation")
    operation_data: Dict[str, Any] = Field(
        default_factory=dict,
        description="Operation-specific data"
    )
    
    # Executor
    executor: str = Field(..., description="Account that will execute the operation")
    
    # Status
    status: str = Field(
        default="Pending",
        description="Schedule status (Pending, Executed, Cancelled)"
    )
    executed_block: Optional[int] = Field(
        default=None,
        description="Block when operation was executed"
    )
    
    # Metadata
    priority: int = Field(default=0, description="Execution priority", ge=0)
    repeating: bool = Field(default=False, description="Whether operation repeats")
    repeat_interval: Optional[int] = Field(
        default=None,
        description="Repeat interval in blocks if repeating"
    )
    
    class Config:
        json_schema_extra = {
            "example": {
                "schedule_id": "sched_abc123",
                "scheduled_block": 150000,
                "created_block": 140000,
                "operation_type": "transfer",
                "operation_data": {
                    "to": "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
                    "amount": 1000.0
                },
                "executor": "5C4hrfjw9DjXZTzV3MwzrrAr9P1MJhSrvWGWqi1eSuyUpnhM",
                "status": "Pending",
                "priority": 1,
                "repeating": False
            }
        }
    
    def __str__(self) -> str:
        return (
            f"ScheduleInfo(id={self.schedule_id}, "
            f"block={self.scheduled_block}, "
            f"status={self.status})"
        )
    
    def __repr__(self) -> str:
        return self.__str__()
