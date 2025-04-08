from server import BaseMiner, BaseValidator, TaskModel
import threading
import time
import requests

# Custom Miner Example
class CustomMiner(BaseMiner):
    def process_task(self, task: TaskModel) -> dict:
        """
        Custom task processing logic.
        
        Args:
            task (TaskModel): Task to be processed.
        
        Returns:
            dict: Result of the task processing.
        """
        print(f"[CustomMiner] Processing task: {task.task_id} with custom logic")
        processing_time = 5  # Fixed processing time of 5 seconds
        time.sleep(processing_time)
        result = {
            "result_id": f"custom_result_{task.task_id}",
            "description": f"Custom processed result for {task.description}",
            "processing_time": processing_time,
            "miner_id": "custom_miner_001"
        }
        print(f"[CustomMiner] Task {task.task_id} completed in {processing_time}s")
        return result

# Custom Validator Example
class CustomValidator(BaseValidator):
    def send_task(self, task_counter: int):
        """
        Custom task sending logic.
        
        Args:
            task_counter (int): Task counter for generating task ID.
        """
        task = {
            "task_id": f"custom_task_{task_counter:03d}",
            "description": f"Custom task number {task_counter}",
            "deadline": "2025-01-01",
            "priority": 2  # Fixed priority
        }
        try:
            print(f"[CustomValidator] Sending custom task: {task['task_id']} - {task['description']}")
            response = requests.post(self.miner_url, json=task, timeout=5)
            print(f"[CustomValidator] Miner response: {response.json()}")
        except Exception as e:
            print(f"[CustomValidator] Error sending task: {e}")

# Running the example
if __name__ == "__main__":
    # Define URLs for communication
    validator_url = "http://localhost:8001/submit-result"
    miner_url = "http://localhost:8000/receive-task"

    # Create and start CustomMiner in a separate thread
    custom_miner = CustomMiner(validator_url=validator_url, host="localhost", port=8000)
    miner_thread = threading.Thread(target=custom_miner.run)
    miner_thread.start()

    # Create and start CustomValidator
    custom_validator = CustomValidator(miner_url=miner_url, host="localhost", port=8001)
    custom_validator.run()