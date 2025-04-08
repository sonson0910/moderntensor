from fastapi import FastAPI
from pydantic import BaseModel, Field
import uvicorn
import requests
import time
import threading

# Define common data models
class TaskModel(BaseModel):
    """
    Pydantic model for task data sent from validator to miner.
    """
    task_id: str = Field(..., description="Unique ID of the task")
    description: str = Field(..., description="Detailed description of the task")
    deadline: str = Field(..., description="Deadline for task completion")
    priority: int = Field(..., description="Priority of the task (1-5)")

class ResultModel(BaseModel):
    """
    Pydantic model for result data sent from miner to validator.
    """
    result_id: str = Field(..., description="Unique ID of the result")
    description: str = Field(..., description="Detailed description of the result")
    processing_time: float = Field(..., description="Processing time in seconds")
    miner_id: str = Field(..., description="ID of the miner who processed the task")

# Base class for Miner
class BaseMiner:
    def __init__(self, validator_url, host="0.0.0.0", port=8000):
        """
        Initialize BaseMiner.
        
        Args:
            validator_url (str): URL of the validator to send results to.
            host (str): Host address for the miner server.
            port (int): Port for the miner server.
        """
        self.app = FastAPI()
        self.validator_url = validator_url
        self.host = host
        self.port = port
        self.setup_routes()

    def setup_routes(self):
        """Set up routes for the miner server."""
        @self.app.post("/receive-task")
        async def receive_task(task: TaskModel):
            print(f"[Miner] Received new task: {task.task_id} - {task.description} - Deadline: {task.deadline}")
            threading.Thread(target=self.handle_task, args=(task,)).start()
            return {"message": f"Task {task.task_id} received and processing"}

    def process_task(self, task: TaskModel) -> dict:
        """
        Process the task (can be overridden for customization).
        
        Args:
            task (TaskModel): Task to be processed.
        
        Returns:
            dict: Result of the task processing.
        """
        print(f"[Miner] Starting task: {task.task_id} - {task.description} (Priority: {task.priority})")
        processing_time = 3 + (task.priority % 3)  # Simulate processing time based on priority
        time.sleep(processing_time)
        result = {
            "result_id": f"result_{task.task_id}",
            "description": f"Result from task: {task.description}",
            "processing_time": processing_time,
            "miner_id": "miner_001"
        }
        print(f"[Miner] Completed task: {task.task_id} - Processing time: {processing_time}s")
        return result

    def handle_task(self, task: TaskModel):
        """Send the result back to the validator after processing."""
        result = self.process_task(task)
        try:
            print(f"[Miner] Sending result to validator: {result}")
            response = requests.post(self.validator_url, json=result, timeout=5)
            print(f"[Miner] Validator response: {response.json()}")
        except Exception as e:
            print(f"[Miner] Error sending result: {e}")

    def run(self):
        """Start the miner server."""
        print(f"[Miner] Starting server at http://{self.host}:{self.port}")
        uvicorn.run(self.app, host=self.host, port=self.port)

# Base class for Validator
class BaseValidator:
    def __init__(self, miner_url, host="0.0.0.0", port=8001):
        """
        Initialize BaseValidator.
        
        Args:
            miner_url (str): URL of the miner to send tasks to.
            host (str): Host address for the validator server.
            port (int): Port for the validator server.
        """
        self.app = FastAPI()
        self.miner_url = miner_url
        self.host = host
        self.port = port
        self.setup_routes()

    def setup_routes(self):
        """Set up routes for the validator server."""
        @self.app.post("/submit-result")
        async def submit_result(result: ResultModel):
            print(f"[Validator] Received result: {result.result_id} - {result.description} "
                  f"(Processing time: {result.processing_time}s, Miner: {result.miner_id})")
            return {"message": f"Result {result.result_id} received and processed"}

    def send_task(self, task_counter: int):
        """
        Send a task to the miner (can be overridden for customization).
        
        Args:
            task_counter (int): Task counter for generating task ID.
        """
        task = {
            "task_id": f"task_{task_counter:03d}",
            "description": f"Process data batch {task_counter}",
            "deadline": "2024-12-31",
            "priority": (task_counter % 5) + 1
        }
        try:
            print(f"[Validator] Sending task: {task['task_id']} - {task['description']} (Priority: {task['priority']})")
            response = requests.post(self.miner_url, json=task, timeout=5)
            print(f"[Validator] Miner response: {response.json()}")
        except Exception as e:
            print(f"[Validator] Error sending task: {e}")

    def run(self):
        """Start the validator server and continuously send tasks."""
        print(f"[Validator] Starting server at http://{self.host}:{self.port}")
        threading.Thread(target=lambda: uvicorn.run(self.app, host=self.host, port=self.port)).start()
        task_counter = 0
        while True:
            task_counter += 1
            self.send_task(task_counter)
            time.sleep(5)  # Send a task every 5 seconds

# Utility functions to run Miner and Validator from the SDK
def run_miner(validator_url: str, host: str = "0.0.0.0", port: int = 8000):
    """
    Run the miner server with custom configuration.
    
    Args:
        validator_url (str): URL of the validator.
        host (str): Host address of the miner.
        port (int): Port of the miner.
    """
    miner = BaseMiner(validator_url, host, port)
    miner.run()

def run_validator(miner_url: str, host: str = "0.0.0.0", port: int = 8001):
    """
    Run the validator server with custom configuration.
    
    Args:
        miner_url (str): URL of the miner.
        host (str): Host address of the validator.
        port (int): Port of the validator.
    """
    validator = BaseValidator(miner_url, host, port)
    validator.run()

# Example usage
if __name__ == "__main__":
    # Start miner
    # run_miner(validator_url="http://116.98.177.250:33001/submit-result", host="172.17.0.2", port=17812)

    # Start validator
    # run_validator(miner_url="http://172.17.0.2:17812/receive-task", host="172.17.0.3", port=33001)
    pass