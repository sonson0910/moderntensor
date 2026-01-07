"""
Example: Creating a Custom Subnet with ModernTensor AI/ML Layer

This example demonstrates how to create a production-ready subnet in just a few lines of code.
"""

import uuid
import random
from sdk.ai_ml.core.protocol import (
    SubnetProtocol,
    TaskContext,
    Task,
    Result,
    Score,
)
from sdk.ai_ml.subnets.base import BaseSubnet


# =====================================
# Example 1: Simple Custom Subnet
# =====================================

class SimpleTextSubnet(BaseSubnet):
    """
    A simple text generation subnet.
    
    This example shows the minimum code needed to create a working subnet.
    """
    
    def setup(self):
        """Initialize the subnet"""
        super().setup()
        print("SimpleTextSubnet initialized")
    
    def _create_task_impl(self, context: TaskContext) -> Task:
        """Create a text generation task"""
        prompts = [
            "Explain quantum computing",
            "Write a poem about AI",
            "Describe the future of blockchain",
        ]
        
        task_data = {
            "prompt": random.choice(prompts),
            "max_length": int(100 * context.difficulty),
            "temperature": 0.7,
        }
        
        task_id = f"task_{uuid.uuid4().hex[:8]}"
        return Task(
            task_id=task_id,
            task_data=task_data,
            context=context,
            timeout=60.0,
        )
    
    def _solve_task_impl(self, task: Task) -> Result:
        """Solve the task (mock implementation)"""
        prompt = task.task_data["prompt"]
        max_length = task.task_data["max_length"]
        
        # In production, this would call an actual LLM
        # For now, generate dummy text
        output_text = f"Response to '{prompt}': " + "Generated text " * max_length
        
        return Result(
            task_id=task.task_id,
            result_data={"text": output_text.strip()},
            miner_uid=task.context.miner_uid,
        )
    
    def _score_result_impl(self, task: Task, result: Result) -> Score:
        """Score the result"""
        # In production, this would use a reward model
        # For now, use simple heuristics
        output_text = result.result_data.get("text", "")
        expected_length = task.task_data.get("max_length", 100)
        actual_length = len(output_text.split())
        
        # Score based on length match
        ratio = min(actual_length, expected_length) / max(actual_length, expected_length, 1)
        
        return Score(
            value=ratio,
            confidence=0.9,
            metadata={"method": "length_ratio"},
        )


# =====================================
# Example 2: Advanced Custom Subnet
# =====================================

class AdvancedTextSubnet(BaseSubnet):
    """
    An advanced text generation subnet with more features.
    
    This example shows additional features like:
    - Custom validation
    - Better scoring
    - Metadata tracking
    """
    
    def setup(self):
        """Initialize with custom configuration"""
        super().setup()
        
        # Custom configuration
        self.min_prompt_length = self.config.get("min_prompt_length", 10)
        self.max_prompt_length = self.config.get("max_prompt_length", 200)
        self.supported_languages = self.config.get("supported_languages", ["en"])
        
        print(f"AdvancedTextSubnet initialized with config: {self.config}")
    
    def validate_task(self, task: Task) -> bool:
        """Custom validation logic"""
        if not super().validate_task(task):
            return False
        
        # Check required fields
        if "prompt" not in task.task_data:
            return False
        
        prompt = task.task_data["prompt"]
        
        # Check prompt length
        if not (self.min_prompt_length <= len(prompt) <= self.max_prompt_length):
            return False
        
        return True
    
    def _create_task_impl(self, context: TaskContext) -> Task:
        """Create task with difficulty scaling"""
        # Scale difficulty (0.0 to 1.0) to appropriate values
        difficulty = context.difficulty
        
        prompts = {
            "easy": ["What is AI?", "Explain Python"],
            "medium": ["Describe machine learning", "Explain neural networks"],
            "hard": ["Compare transformer architectures", "Discuss AGI safety"],
        }
        
        # Select prompt based on difficulty
        if difficulty < 0.3:
            prompt_list = prompts["easy"]
        elif difficulty < 0.7:
            prompt_list = prompts["medium"]
        else:
            prompt_list = prompts["hard"]
        
        task_data = {
            "prompt": random.choice(prompt_list),
            "max_length": int(50 + 200 * difficulty),  # 50-250 words
            "temperature": 0.5 + 0.4 * difficulty,  # 0.5-0.9
            "language": "en",
        }
        
        task_id = f"task_{context.subnet_uid}_{context.cycle}_{uuid.uuid4().hex[:6]}"
        
        return Task(
            task_id=task_id,
            task_data=task_data,
            context=context,
            timeout=30.0 + 60.0 * difficulty,  # 30-90 seconds
        )
    
    def _solve_task_impl(self, task: Task) -> Result:
        """Solve with metadata tracking"""
        import time
        
        start_time = time.time()
        
        prompt = task.task_data["prompt"]
        max_length = task.task_data["max_length"]
        
        # TODO: In production, call actual LLM here
        # Example: output_text = self.model.generate(prompt, max_length=max_length)
        
        # Mock generation
        output_text = f"[Mock Response] {prompt}: " + "AI generated content " * (max_length // 3)
        
        solve_time = time.time() - start_time
        
        return Result(
            task_id=task.task_id,
            result_data={
                "text": output_text.strip(),
                "length": len(output_text.split()),
            },
            miner_uid=task.context.miner_uid,
            metadata={
                "solve_time": solve_time,
                "model": "mock_model_v1",
                "temperature": task.task_data.get("temperature", 0.7),
            },
        )
    
    def _score_result_impl(self, task: Task, result: Result) -> Score:
        """Advanced scoring with multiple criteria"""
        output_text = result.result_data.get("text", "")
        prompt = task.task_data["prompt"]
        
        # Criterion 1: Length match
        expected_length = task.task_data.get("max_length", 100)
        actual_length = len(output_text.split())
        length_score = min(actual_length, expected_length) / max(actual_length, expected_length, 1)
        
        # Criterion 2: Content quality (mock - would use reward model in production)
        # Check if output contains relevant keywords
        keywords = prompt.lower().split()[:3]
        keyword_matches = sum(1 for kw in keywords if kw in output_text.lower())
        content_score = keyword_matches / max(len(keywords), 1)
        
        # Criterion 3: Formatting (mock)
        has_proper_format = len(output_text) > 0 and not output_text.endswith("...")
        format_score = 1.0 if has_proper_format else 0.5
        
        # Combine scores with weights
        final_score = (
            0.3 * length_score +
            0.5 * content_score +
            0.2 * format_score
        )
        
        # Calculate confidence based on result metadata
        solve_time = result.metadata.get("solve_time", 0)
        confidence = 0.95 if solve_time < 5.0 else 0.85
        
        return Score(
            value=max(0.0, min(1.0, final_score)),
            confidence=confidence,
            metadata={
                "length_score": length_score,
                "content_score": content_score,
                "format_score": format_score,
                "scoring_method": "weighted_multi_criteria",
            },
        )
    
    def get_metadata(self) -> dict:
        """Return subnet metadata"""
        metadata = super().get_metadata()
        metadata.update({
            "name": "AdvancedTextSubnet",
            "version": "1.0.0",
            "description": "Advanced text generation subnet with multi-criteria scoring",
            "supported_languages": self.supported_languages,
            "min_prompt_length": self.min_prompt_length,
            "max_prompt_length": self.max_prompt_length,
        })
        return metadata


# =====================================
# Usage Example
# =====================================

def main():
    """Example usage of the custom subnets"""
    
    print("=" * 60)
    print("ModernTensor AI/ML Layer - Custom Subnet Example")
    print("=" * 60)
    print()
    
    # ===========================
    # Example 1: Simple Subnet
    # ===========================
    
    print("1. Simple Subnet Example")
    print("-" * 60)
    
    # Create and initialize subnet
    simple_subnet = SimpleTextSubnet(config={
        "enable_cache": True,
        "cache_size": 100,
    })
    simple_subnet.setup()
    
    # Create task context
    context = TaskContext(
        miner_uid="miner_test_001",
        difficulty=0.5,
        subnet_uid=1,
        cycle=1,
    )
    
    # Create task
    task = simple_subnet.create_task(context)
    print(f"Created task: {task.task_id}")
    print(f"Task data: {task.task_data}")
    print()
    
    # Solve task
    result = simple_subnet.solve_task(task)
    print(f"Result: {result.result_data}")
    print(f"Execution time: {result.execution_time:.4f}s")
    print()
    
    # Score result
    score = simple_subnet.score_result(task, result)
    print(f"Score: {score.value:.3f} (confidence: {score.confidence:.3f})")
    print()
    
    # Get metrics
    metrics = simple_subnet.get_metrics()
    print(f"Metrics: {metrics}")
    print()
    
    # ===========================
    # Example 2: Advanced Subnet
    # ===========================
    
    print("\n2. Advanced Subnet Example")
    print("-" * 60)
    
    # Create subnet with custom config
    advanced_subnet = AdvancedTextSubnet(config={
        "enable_cache": True,
        "cache_size": 500,
        "min_prompt_length": 5,
        "max_prompt_length": 300,
        "supported_languages": ["en", "vi"],
    })
    advanced_subnet.setup()
    
    # Create multiple tasks with different difficulties
    for difficulty in [0.2, 0.5, 0.8]:
        print(f"\nDifficulty: {difficulty:.1f}")
        
        context = TaskContext(
            miner_uid="miner_test_002",
            difficulty=difficulty,
            subnet_uid=1,
            cycle=1,
        )
        
        task = advanced_subnet.create_task(context)
        print(f"  Task: {task.task_data['prompt']}")
        print(f"  Max length: {task.task_data['max_length']}")
        
        result = advanced_subnet.solve_task(task)
        print(f"  Result length: {result.result_data['length']} words")
        print(f"  Execution time: {result.execution_time:.4f}s")
        
        score = advanced_subnet.score_result(task, result)
        print(f"  Score: {score.value:.3f} (confidence: {score.confidence:.3f})")
        print(f"  Scoring details: {score.metadata}")
    
    print()
    
    # Display subnet metadata
    print("\nSubnet Metadata:")
    print("-" * 60)
    metadata = advanced_subnet.get_metadata()
    for key, value in metadata.items():
        print(f"  {key}: {value}")
    
    print()
    print("=" * 60)
    print("Example completed successfully!")
    print("=" * 60)


if __name__ == "__main__":
    main()
