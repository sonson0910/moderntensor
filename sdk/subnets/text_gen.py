from typing import Any, Dict
import random
from .protocol import SubnetProtocol


class TextGenerationSubnet(SubnetProtocol):
    """
    Example implementation of a Text Generation Subnet.
    """

    def create_task(self, miner_uid: str, difficulty: float) -> Dict[str, Any]:
        prompts = [
            "Explain the theory of relativity",
            "Write a poem about Cardano",
            "Describe the future of AI",
            "What is the meaning of life?",
            "Summarize the history of the internet",
        ]

        # Difficulty could affect max_length or complexity of prompt
        max_length = int(100 * difficulty)

        return {
            "prompt": random.choice(prompts),
            "max_length": max_length,
            "temperature": 0.7,
        }

    def score_result(self, task_data: Any, result_data: Any) -> float:
        # Mock scoring logic
        # In reality, this would use a reward model or similarity check
        output_text = result_data.get("text", "")
        if not output_text:
            return 0.0

        # Simple length check as a placeholder
        expected_len = task_data.get("max_length", 100)
        actual_len = len(output_text.split())

        # Score based on how close length is to expected (just an example)
        ratio = min(actual_len, expected_len) / max(actual_len, expected_len)
        return ratio

    def solve_task(self, task_data: Any) -> Any:
        """
        Mock implementation of solving a text generation task.
        """
        prompt = task_data.get("prompt", "")
        max_length = task_data.get("max_length", 100)

        # In a real scenario, this would call an LLM (e.g., GPT-2, Llama)
        # Here we just generate dummy text
        dummy_text = f"Response to '{prompt}': " + "blah " * max_length

        return {"text": dummy_text.strip()}

    def get_metadata(self) -> Dict[str, Any]:
        return {
            "name": "Text Generation Subnet",
            "version": "1.0.0",
            "description": "A subnet for generating high-quality text.",
        }
