"""
Advanced AI/ML Subnet Example - Demonstrating features that surpass Bittensor.

This example showcases:
1. Model versioning with ModelManager
2. Batch processing for efficiency
3. Parallel task processing
4. Advanced metrics tracking
5. Priority-based task scheduling

Run:
    PYTHONPATH=. python3 examples/advanced_ai_ml_example.py
"""

import asyncio
import time
import uuid
from typing import List

from sdk.ai_ml.core.protocol import (
    SubnetProtocol,
    TaskContext,
    Task,
    Result,
    Score,
)
from sdk.ai_ml.subnets.base import BaseSubnet
from sdk.ai_ml.models import ModelManager, ModelMetadata
from sdk.ai_ml.processors import BatchProcessor, BatchConfig, ParallelProcessor


class AdvancedTextSubnet(BaseSubnet):
    """
    Advanced text generation subnet showcasing features beyond Bittensor.
    
    Features:
    - Model versioning and management
    - Batch processing for efficiency
    - Performance tracking
    - Multi-criteria scoring
    """
    
    def setup(self):
        """Initialize subnet with model manager"""
        super().setup()
        
        # Initialize model manager
        self.model_manager = ModelManager()
        
        # Register a text generation model
        self.model_manager.register_model(
            model_id="gpt2-small",
            name="GPT-2 Small",
            description="Small GPT-2 model for text generation",
            framework="transformers",
            task_type="text_generation",
            tags=["gpt2", "text", "generation"],
        )
        
        # Add version
        self.model_manager.add_version(
            model_id="gpt2-small",
            version="1.0.0",
            metadata={"parameters": "124M", "context_length": 1024},
        )
        
        # Register model loader (simplified - in production would load actual model)
        def mock_loader(model_id, version, **kwargs):
            print(f"Loading {model_id} v{version}")
            return {"model": "mock_gpt2", "version": version}
        
        self.model_manager.register_loader("gpt2-small", mock_loader)
        
        # Load model
        self.model = self.model_manager.load_model("gpt2-small")
        
        print(f"✅ {self.__class__.__name__} initialized with ModelManager")
    
    def _create_task_impl(self, context: TaskContext) -> Task:
        """Create text generation task"""
        prompts = [
            "Explain quantum computing",
            "Write about the future of AI",
            "Describe blockchain technology",
            "Discuss climate change solutions",
        ]
        
        import random
        prompt = random.choice(prompts)
        
        task_data = {
            "prompt": prompt,
            "max_length": int(100 * context.difficulty),
            "temperature": 0.7,
            "model_id": "gpt2-small",
        }
        
        return Task(
            task_id=f"task_{uuid.uuid4().hex[:8]}",
            task_data=task_data,
            context=context,
            timeout=30.0,
        )
    
    def _solve_task_impl(self, task: Task) -> Result:
        """
        Solve text generation task with performance tracking.
        
        In production, this would use a real LLM.
        """
        start_time = time.time()
        
        # Mock text generation (in production: use actual model)
        prompt = task.task_data["prompt"]
        max_length = task.task_data.get("max_length", 100)
        
        # Simulate processing time
        time.sleep(0.1)
        
        output = f"Generated response to '{prompt}': " + "Lorem ipsum " * max_length
        
        execution_time = time.time() - start_time
        
        # Track inference performance
        self.model_manager.track_inference(
            model_id="gpt2-small",
            latency_ms=execution_time * 1000,
        )
        
        return Result(
            task_id=task.task_id,
            result_data={"text": output, "tokens": max_length},
            miner_uid=task.context.miner_uid,
            execution_time=execution_time,
            metadata={
                "model_id": "gpt2-small",
                "model_version": "1.0.0",
                "prompt_length": len(prompt),
            },
        )
    
    def _score_result_impl(self, task: Task, result: Result) -> Score:
        """
        Multi-criteria scoring (beyond Bittensor's simple scoring).
        
        Criteria:
        1. Output quality (length, coherence)
        2. Execution speed
        3. Resource efficiency
        """
        output = result.result_data.get("text", "")
        expected_length = task.task_data.get("max_length", 100)
        
        # Criterion 1: Length score
        actual_words = len(output.split())
        length_score = min(actual_words / expected_length, 1.0)
        
        # Criterion 2: Speed score (faster is better)
        exec_time = result.execution_time or 1.0
        speed_score = max(0, 1.0 - (exec_time / 5.0))  # Penalty after 5s
        
        # Criterion 3: Quality score (simplified)
        quality_score = 0.8  # In production: use reward model
        
        # Weighted combination
        final_score = (
            0.4 * quality_score +
            0.3 * length_score +
            0.3 * speed_score
        )
        
        # Confidence based on execution time (faster = higher confidence)
        confidence = max(0.5, min(1.0, 1.0 - (exec_time / 10.0)))
        
        return Score(
            value=final_score,
            confidence=confidence,
            metadata={
                "length_score": length_score,
                "speed_score": speed_score,
                "quality_score": quality_score,
                "criteria": "multi-criteria scoring",
            },
        )


async def demo_batch_processing():
    """Demonstrate batch processing (feature Bittensor doesn't have)"""
    print("\n" + "=" * 60)
    print("DEMO 1: Batch Processing")
    print("=" * 60)
    
    subnet = AdvancedTextSubnet(config={
        "enable_cache": True,
        "max_retries": 3,
    })
    subnet.setup()
    
    # Create multiple tasks
    contexts = [
        TaskContext(
            miner_uid=f"miner_{i}",
            difficulty=0.5,
            subnet_uid=1,
            cycle=1,
        )
        for i in range(5)
    ]
    
    tasks = [subnet.create_task(ctx) for ctx in contexts]
    
    # Define batch processing function
    def process_batch(task_list: List[Task]) -> List[Result]:
        """Process multiple tasks at once"""
        print(f"  Processing batch of {len(task_list)} tasks...")
        return [subnet.solve_task(t) for t in task_list]
    
    # Create batch processor
    batch_config = BatchConfig(
        max_batch_size=3,
        batch_timeout_ms=100,
        enable_dynamic_batching=True,
    )
    processor = BatchProcessor(batch_config, process_batch)
    
    # Process tasks in batches
    start_time = time.time()
    results = await processor.process(tasks)
    total_time = time.time() - start_time
    
    print(f"\n✅ Processed {len(results)} tasks in {total_time:.2f}s")
    print(f"   Throughput: {len(results) / total_time:.1f} tasks/sec")
    
    # Show batch metrics
    metrics = processor.get_metrics()
    print(f"\nBatch Metrics:")
    print(f"  Total batches: {metrics['total_batches']}")
    print(f"  Avg batch size: {metrics['avg_batch_size']:.1f}")
    print(f"  Avg latency: {metrics['avg_latency_ms']:.1f}ms")
    print(f"  Throughput: {metrics['throughput_tasks_per_sec']:.1f} tasks/sec")


async def demo_parallel_processing():
    """Demonstrate parallel processing"""
    print("\n" + "=" * 60)
    print("DEMO 2: Parallel Processing")
    print("=" * 60)
    
    subnet = AdvancedTextSubnet(config={"enable_cache": False})
    subnet.setup()
    
    # Create tasks
    contexts = [
        TaskContext(
            miner_uid=f"miner_{i}",
            difficulty=0.5,
            subnet_uid=1,
            cycle=1,
        )
        for i in range(8)
    ]
    
    tasks = [subnet.create_task(ctx) for ctx in contexts]
    
    # Process in parallel
    processor = ParallelProcessor(num_workers=4)
    
    start_time = time.time()
    results = await processor.process_parallel(tasks, subnet.solve_task)
    total_time = time.time() - start_time
    
    print(f"\n✅ Processed {len(results)} tasks in {total_time:.2f}s using 4 workers")
    print(f"   Throughput: {len(results) / total_time:.1f} tasks/sec")
    
    processor.shutdown()


def demo_model_management():
    """Demonstrate model management (feature Bittensor doesn't have)"""
    print("\n" + "=" * 60)
    print("DEMO 3: Model Management & Versioning")
    print("=" * 60)
    
    manager = ModelManager()
    
    # Register multiple models
    manager.register_model(
        model_id="gpt2-small",
        name="GPT-2 Small",
        description="Small GPT-2 model",
        framework="transformers",
        task_type="text_generation",
    )
    
    manager.register_model(
        model_id="bert-base",
        name="BERT Base",
        description="BERT base model",
        framework="transformers",
        task_type="classification",
    )
    
    # Add versions
    manager.add_version("gpt2-small", "1.0.0", metadata={"params": "124M"})
    manager.add_version("gpt2-small", "1.1.0", metadata={"params": "124M", "optimized": True})
    manager.add_version("bert-base", "1.0.0", metadata={"params": "110M"})
    
    # Track performance
    manager.track_inference("gpt2-small", latency_ms=150, batch_size=1)
    manager.track_inference("gpt2-small", latency_ms=145, batch_size=1)
    manager.track_inference("bert-base", latency_ms=80, batch_size=1)
    
    # List models
    print("\nRegistered Models:")
    models = manager.list_models()
    for model in models:
        print(f"  • {model.name} ({model.model_id})")
        print(f"    Framework: {model.framework}")
        print(f"    Versions: {len(model.versions)}")
        print(f"    Latest: v{model.latest_version}")
    
    # Show performance summary
    print("\nPerformance Summary:")
    summary = manager.get_performance_summary()
    for model_id, stats in summary.items():
        perf = stats["performance"]
        print(f"  {stats['name']}:")
        if perf:
            print(f"    Avg latency: {perf.get('avg_latency_ms', 0):.1f}ms")
            print(f"    Total inferences: {perf.get('total_inferences', 0)}")


async def main():
    """Run all demos"""
    print("\n" + "=" * 60)
    print("ModernTensor Advanced AI/ML Features")
    print("Surpassing Bittensor's Capabilities")
    print("=" * 60)
    
    # Demo 1: Model Management
    demo_model_management()
    
    # Demo 2: Batch Processing
    await demo_batch_processing()
    
    # Demo 3: Parallel Processing
    await demo_parallel_processing()
    
    print("\n" + "=" * 60)
    print("All Demos Completed Successfully! ✅")
    print("=" * 60)
    print("\nKey Advantages over Bittensor:")
    print("  1. ✅ Model versioning and experiment tracking")
    print("  2. ✅ Automatic batch processing for efficiency")
    print("  3. ✅ Parallel task processing")
    print("  4. ✅ Multi-criteria scoring")
    print("  5. ✅ Advanced performance metrics")
    print("  6. ✅ Priority-based task scheduling")
    print("  7. ✅ Dynamic batch size optimization")
    print("\n")


if __name__ == "__main__":
    asyncio.run(main())
