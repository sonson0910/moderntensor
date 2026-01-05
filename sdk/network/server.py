from fastapi import FastAPI
from pydantic import BaseModel, Field
import uvicorn
import requests
import time
import threading
import json
from typing import Optional
import logging

# Get logger instance
logger = logging.getLogger(__name__)


# Define common data models
class TaskModel(BaseModel):
    """
    Pydantic model for task data sent from validator to miner.
    """

    task_id: str = Field(..., description="Unique ID of the task")
    description: str = Field(..., description="Detailed description of the task")
    deadline: Optional[str] = Field(
        None, description="Optional deadline for task completion"
    )
    priority: Optional[int] = Field(
        None, description="Optional priority of the task (1-5)"
    )
    validator_endpoint: Optional[str] = Field(
        None,
        description="API endpoint of the originating validator to send the result back to",
    )
    task_data: dict = Field(default_factory=dict, description="The actual task payload")


class ResultModel(BaseModel):
    """
    Pydantic model for result data sent from miner to validator.
    """

    task_id: str = Field(..., description="Unique ID of the task this result is for")
    miner_uid: str = Field(..., description="UID of the miner who processed the task")
    result_data: dict = Field(
        default_factory=dict, description="The actual result payload"
    )
    proof: Optional[str] = Field(
        None, description="Optional zkML proof (hex string) verifying the computation"
    )


# Base class for Miner
class BaseMiner:
    def __init__(
        self, validator_url, host="0.0.0.0", port=8000, miner_uid="miner_default_001"
    ):
        """
        Initialize BaseMiner.

        Args:
            validator_url (str): Default URL of the validator to send results to.
            host (str): Host address for the miner server.
            port (int): Port for the miner server.
            miner_uid (str): Unique identifier for this miner.
        """
        self.app = FastAPI()
        self.validator_url = validator_url
        self.host = host
        self.port = port
        self.miner_uid = miner_uid
        self.setup_routes()
        logger.info(
            f":robot: [Miner:{self.miner_uid}] Initialized. Default Validator target: [link={self.validator_url}]{self.validator_url}[/link]"
        )

    def setup_routes(self):
        """Set up routes for the miner server."""

        @self.app.post("/receive-task")
        async def receive_task(task: TaskModel):
            logger.info(
                f":inbox_tray: [Miner:{self.miner_uid}] Received task [yellow]{task.task_id}[/yellow] - Desc: '{task.description}' - Prio: {task.priority}"
            )
            threading.Thread(target=self.handle_task, args=(task,)).start()
            return {"message": f"Task {task.task_id} received and processing"}

    def process_task(self, task: TaskModel) -> dict:
        """
        Process the task data (can be overridden for customization).

        Args:
            task (TaskModel): Task containing task_id and task_data.

        Returns:
            dict: Result data dictionary.
        """
        logger.info(
            f":hourglass_flowing_sand: [Miner:{self.miner_uid}] Starting task [yellow]{task.task_id}[/yellow] (Priority: {task.priority}) - Data: {str(task.task_data)[:50]}..."
        )
        processing_time = 3 + (task.priority % 3) if task.priority else 4
        time.sleep(processing_time)
        result_payload = {
            "output": f"processed_output_for_{task.task_id}",
            "loss": round(1.0 / (processing_time + 1), 4),
            "processing_time_ms": int(processing_time * 1000),
        }
        logger.info(
            f":white_check_mark: [Miner:{self.miner_uid}] Completed task [yellow]{task.task_id}[/yellow] - Processing time: {processing_time:.2f}s"
        )
        return result_payload

    def handle_task(self, task: TaskModel):
        """Processes the task and sends the ResultModel back to the validator."""
        result_data_payload = self.process_task(task)

        result_to_send = ResultModel(
            task_id=task.task_id,
            miner_uid=self.miner_uid,
            result_data=result_data_payload,
        )

        target_validator_url = task.validator_endpoint or self.validator_url
        if not target_validator_url:
            logger.error(
                f":x: [Miner:{self.miner_uid}] No validator URL found for task [yellow]{task.task_id}[/yellow]. Cannot send result."
            )
            return

        result_submit_url = f"{target_validator_url.rstrip('/')}/v1/miner/submit_result"

        try:
            logger.debug(
                f":outbox_tray: [Miner:{self.miner_uid}] Sending result for task [yellow]{task.task_id}[/yellow] to [link={result_submit_url}]{result_submit_url}[/link]"
                + f" Payload: {str(result_to_send.dict())[:100]}..."
            )
            response = requests.post(
                result_submit_url, json=result_to_send.dict(), timeout=10
            )
            response.raise_for_status()
            try:
                response_data = response.json()
                logger.info(
                    f":mailbox_with_mail: [Miner:{self.miner_uid}] Result sent. Validator response for task [yellow]{task.task_id}[/yellow]: {response_data}"
                )
            except json.JSONDecodeError:
                logger.info(
                    f":mailbox_with_mail: [Miner:{self.miner_uid}] Result sent. Validator response for task [yellow]{task.task_id}[/yellow]: Status {response.status_code} (Non-JSON)"
                )

        except requests.exceptions.RequestException as e:
            logger.error(
                f":x: [Miner:{self.miner_uid}] Error sending result for task [yellow]{task.task_id}[/yellow] to {result_submit_url}: {e}"
            )
        except Exception as e:
            logger.exception(
                f":rotating_light: [Miner:{self.miner_uid}] Unexpected error handling task [yellow]{task.task_id}[/yellow]: {e}"
            )

    def run(self):
        """Start the miner server."""
        logger.info(
            f":rocket: [Miner:{self.miner_uid}] Starting server at [link=http://{self.host}:{self.port}]http://{self.host}:{self.port}[/link]"
        )
        uvicorn.run(self.app, host=self.host, port=self.port, log_config=None)


# Base class for Validator
class BaseValidator:
    def __init__(
        self, host="0.0.0.0", port=8001, validator_uid="validator_default_001"
    ):
        """
        Initialize BaseValidator.

        Args:
            host (str): Host address for the validator server.
            port (int): Port for the validator server.
            validator_uid (str): Unique identifier for this validator.
        """
        self.app = FastAPI()
        self.host = host
        self.port = port
        self.validator_uid = validator_uid
        self.miner_clients = {}
        self.setup_routes()
        logger.info(f":shield: [Validator:{self.validator_uid}] Initialized.")

    def setup_routes(self):
        """Set up routes for the validator server."""

        @self.app.post("/v1/miner/submit_result")
        async def submit_result(result: ResultModel):
            logger.info(
                f":inbox_tray: [Validator:{self.validator_uid}] Received result for task [yellow]{result.task_id}[/yellow] from Miner [cyan]{result.miner_uid}[/cyan] - Data: {str(result.result_data)[:50]}..."
            )
            return {"message": f"Result for task {result.task_id} received"}

    def send_task_to_miner(self, miner_endpoint: str, task_model: TaskModel):
        """
        Example function to send a single task to a miner.
        Note: Uses blocking requests, should be async or run in thread in real usage.
        """
        target_url = f"{miner_endpoint.rstrip('/')}/receive-task"
        try:
            logger.info(
                f":outbox_tray: [Validator:{self.validator_uid}] Sending task [yellow]{task_model.task_id}[/yellow] to Miner at [link={target_url}]{target_url}[/link]"
            )
            response = requests.post(target_url, json=task_model.dict(), timeout=5)
            response.raise_for_status()
            logger.info(
                f":mailbox_with_mail: [Validator:{self.validator_uid}] Miner response for task [yellow]{task_model.task_id}[/yellow]: {response.json()}"
            )
            return True
        except requests.exceptions.RequestException as e:
            logger.error(
                f":x: [Validator:{self.validator_uid}] Error sending task [yellow]{task_model.task_id}[/yellow] to {target_url}: {e}"
            )
            return False
        except Exception as e:
            logger.exception(
                f":rotating_light: [Validator:{self.validator_uid}] Unexpected error sending task [yellow]{task_model.task_id}[/yellow]: {e}"
            )
            return False

    def run_server_only(self):
        """Starts only the validator FastAPI server."""
        logger.info(
            f":rocket: [Validator:{self.validator_uid}] Starting server at [link=http://{self.host}:{self.port}]http://{self.host}:{self.port}[/link]"
        )
        uvicorn.run(self.app, host=self.host, port=self.port, log_config=None)


# Utility functions might not be needed if run from ValidatorNode/MinerAgent
# def run_miner(...)
# def run_validator(...)

# Remove example usage as this file is now likely imported as a module
# if __name__ == "__main__":
#    pass
