"""
Tests for SubnetProtocol and core functionality
"""

import pytest
import time
from sdk.ai_ml.core.protocol import (
    SubnetProtocol,
    TaskContext,
    Task,
    Result,
    Score,
    TaskStatus,
)


class MockSubnet(SubnetProtocol):
    """Mock subnet for testing"""
    
    def setup(self):
        self._initialized = True
        self.setup_called = True
    
    def _create_task_impl(self, context: TaskContext) -> Task:
        return Task(
            task_id=f"task_{context.miner_uid}_{context.cycle}",
            task_data={"prompt": "test prompt", "difficulty": context.difficulty},
            context=context,
        )
    
    def _solve_task_impl(self, task: Task) -> Result:
        return Result(
            task_id=task.task_id,
            result_data={"output": "test output"},
            miner_uid=task.context.miner_uid,
        )
    
    def _score_result_impl(self, task: Task, result: Result) -> Score:
        return Score(value=0.8, confidence=0.95)


class TestTaskContext:
    """Tests for TaskContext"""
    
    def test_create_context(self):
        context = TaskContext(
            miner_uid="miner_1",
            difficulty=0.5,
            subnet_uid=1,
            cycle=10,
        )
        assert context.miner_uid == "miner_1"
        assert context.difficulty == 0.5
        assert context.subnet_uid == 1
        assert context.cycle == 10
        assert context.metadata == {}
    
    def test_context_with_metadata(self):
        context = TaskContext(
            miner_uid="miner_1",
            difficulty=0.5,
            subnet_uid=1,
            cycle=10,
            metadata={"key": "value"},
        )
        assert context.metadata == {"key": "value"}


class TestTask:
    """Tests for Task"""
    
    def test_create_task(self):
        context = TaskContext(
            miner_uid="miner_1",
            difficulty=0.5,
            subnet_uid=1,
            cycle=10,
        )
        task = Task(
            task_id="task_1",
            task_data={"prompt": "test"},
            context=context,
        )
        assert task.task_id == "task_1"
        assert task.task_data == {"prompt": "test"}
        assert task.context == context
        assert task.created_at > 0
        assert task.timeout is None
    
    def test_task_to_dict(self):
        context = TaskContext(
            miner_uid="miner_1",
            difficulty=0.5,
            subnet_uid=1,
            cycle=10,
        )
        task = Task(
            task_id="task_1",
            task_data={"prompt": "test"},
            context=context,
            timeout=60.0,
        )
        task_dict = task.to_dict()
        assert task_dict["task_id"] == "task_1"
        assert task_dict["task_data"] == {"prompt": "test"}
        assert task_dict["context"]["miner_uid"] == "miner_1"
        assert task_dict["timeout"] == 60.0


class TestResult:
    """Tests for Result"""
    
    def test_create_result(self):
        result = Result(
            task_id="task_1",
            result_data={"output": "test output"},
            miner_uid="miner_1",
        )
        assert result.task_id == "task_1"
        assert result.result_data == {"output": "test output"}
        assert result.miner_uid == "miner_1"
        assert result.completed_at > 0
        assert result.execution_time is None
        assert result.proof is None
    
    def test_result_to_dict(self):
        result = Result(
            task_id="task_1",
            result_data={"output": "test"},
            miner_uid="miner_1",
            execution_time=1.5,
        )
        result_dict = result.to_dict()
        assert result_dict["task_id"] == "task_1"
        assert result_dict["result_data"] == {"output": "test"}
        assert result_dict["miner_uid"] == "miner_1"
        assert result_dict["execution_time"] == 1.5
        assert result_dict["proof"] is None


class TestScore:
    """Tests for Score"""
    
    def test_create_score(self):
        score = Score(value=0.8, confidence=0.95)
        assert score.value == 0.8
        assert score.confidence == 0.95
        assert score.timestamp > 0
        assert score.validator_uid is None
    
    def test_score_validation(self):
        # Valid scores
        Score(value=0.0, confidence=1.0)
        Score(value=1.0, confidence=0.0)
        Score(value=0.5, confidence=0.5)
        
        # Invalid scores
        with pytest.raises(ValueError):
            Score(value=1.5, confidence=1.0)
        
        with pytest.raises(ValueError):
            Score(value=-0.1, confidence=1.0)


class TestSubnetProtocol:
    """Tests for SubnetProtocol"""
    
    def test_subnet_initialization(self):
        subnet = MockSubnet()
        assert not subnet.is_ready()
        
        subnet.setup()
        assert subnet.is_ready()
        assert subnet.setup_called
    
    def test_create_task(self):
        subnet = MockSubnet()
        subnet.setup()
        
        context = TaskContext(
            miner_uid="miner_1",
            difficulty=0.5,
            subnet_uid=1,
            cycle=10,
        )
        task = subnet.create_task(context)
        
        assert task.task_id == "task_miner_1_10"
        assert task.task_data["prompt"] == "test prompt"
    
    def test_solve_task(self):
        subnet = MockSubnet()
        subnet.setup()
        
        context = TaskContext(
            miner_uid="miner_1",
            difficulty=0.5,
            subnet_uid=1,
            cycle=10,
        )
        task = subnet.create_task(context)
        result = subnet.solve_task(task)
        
        assert result.task_id == task.task_id
        assert result.result_data["output"] == "test output"
        assert result.execution_time is not None
    
    def test_score_result(self):
        subnet = MockSubnet()
        subnet.setup()
        
        context = TaskContext(
            miner_uid="miner_1",
            difficulty=0.5,
            subnet_uid=1,
            cycle=10,
        )
        task = subnet.create_task(context)
        result = subnet.solve_task(task)
        score = subnet.score_result(task, result)
        
        assert score.value == 0.8
        assert score.confidence == 0.95
    
    def test_metrics(self):
        subnet = MockSubnet()
        subnet.setup()
        
        context = TaskContext(
            miner_uid="miner_1",
            difficulty=0.5,
            subnet_uid=1,
            cycle=10,
        )
        
        task = subnet.create_task(context)
        result = subnet.solve_task(task)
        score = subnet.score_result(task, result)
        
        metrics = subnet.get_metrics()
        assert "task_creation_time_avg" in metrics
        assert "solve_time_avg" in metrics
        assert "score_time_avg" in metrics


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
