# ModernTensor AI/ML Layer - Implementation Guide

## Tổng Quan

Tài liệu này hướng dẫn cách sử dụng và triển khai AI/ML layer mới của ModernTensor.

## Kiến Trúc Mới

### Cấu Trúc Thư Mục

```
sdk/ai_ml/
├── core/              # Protocol và base classes
│   └── protocol.py    # SubnetProtocol, Task, Result, Score
├── subnets/           # Subnet implementations
│   └── base.py        # BaseSubnet với cache, retry, metrics
├── agent/             # Miner và validator agents (coming soon)
├── models/            # Model management (coming soon)
├── zkml/              # zkML proof generation (coming soon)
├── scoring/           # Scoring và consensus (coming soon)
└── processors/        # Task processors (coming soon)
```

## Hướng Dẫn Tạo Subnet Mới

### 1. Subnet Đơn Giản

```python
from sdk.ai_ml.subnets.base import BaseSubnet
from sdk.ai_ml.core.protocol import TaskContext, Task, Result, Score
import uuid

class MySimpleSubnet(BaseSubnet):
    """Subnet đơn giản nhất có thể"""
    
    def setup(self):
        """Khởi tạo subnet"""
        super().setup()
        # Load models hoặc resources tại đây
    
    def _create_task_impl(self, context: TaskContext) -> Task:
        """Tạo task"""
        task_data = {
            "input": "some_input",
            "difficulty": context.difficulty,
        }
        return Task(
            task_id=f"task_{uuid.uuid4().hex[:8]}",
            task_data=task_data,
            context=context,
        )
    
    def _solve_task_impl(self, task: Task) -> Result:
        """Giải task"""
        # Xử lý task tại đây
        output = process_task(task.task_data)
        
        return Result(
            task_id=task.task_id,
            result_data={"output": output},
            miner_uid=task.context.miner_uid,
        )
    
    def _score_result_impl(self, task: Task, result: Result) -> Score:
        """Chấm điểm"""
        score_value = evaluate(task, result)
        return Score(value=score_value, confidence=0.9)
```

### 2. Sử Dụng Subnet

```python
# Tạo và khởi tạo
subnet = MySimpleSubnet(config={
    "enable_cache": True,
    "cache_size": 100,
    "task_timeout": 60,
})
subnet.setup()

# Tạo task
context = TaskContext(
    miner_uid="miner_001",
    difficulty=0.5,
    subnet_uid=1,
    cycle=1,
)
task = subnet.create_task(context)

# Giải task
result = subnet.solve_task(task)

# Chấm điểm
score = subnet.score_result(task, result)

print(f"Score: {score.value:.3f}")
```

### 3. Features Tích Hợp Sẵn

BaseSubnet cung cấp sẵn các features:

#### Cache
```python
# Cache tự động được enable
subnet = MySubnet(config={"enable_cache": True, "cache_size": 1000})

# Kết quả giống nhau sẽ được cache
result1 = subnet.solve_task(task)  # Cache miss
result2 = subnet.solve_task(task)  # Cache hit - instant return
```

#### Retry Logic
```python
# Tự động retry khi task fail
subnet = MySubnet(config={"max_retries": 3})

# Nếu solve_task raise exception, sẽ retry 3 lần
result = subnet.solve_task(task)
```

#### Metrics
```python
# Metrics được track tự động
metrics = subnet.get_metrics()
print(metrics)
# Output:
# {
#   "task_creation_time_avg": 0.001,
#   "solve_time_avg": 0.5,
#   "score_time_avg": 0.1,
#   "cache": {"hits": 10, "misses": 5, "hit_rate": 0.67}
# }
```

#### Validation
```python
class MySubnet(BaseSubnet):
    def validate_task(self, task: Task) -> bool:
        """Custom validation"""
        if not super().validate_task(task):
            return False
        
        # Thêm validation rules
        if "required_field" not in task.task_data:
            return False
        
        return True
```

## Ví Dụ Thực Tế

### Text Generation Subnet

Xem file: `examples/ai_ml_subnet_example.py`

Có 2 ví dụ:
1. **SimpleTextSubnet** - Implementation đơn giản nhất
2. **AdvancedTextSubnet** - Implementation với features đầy đủ

Chạy ví dụ:
```bash
cd /home/runner/work/moderntensor/moderntensor
PYTHONPATH=. python examples/ai_ml_subnet_example.py
```

## Best Practices

### 1. Setup và Teardown

```python
def setup(self):
    """Luôn gọi super().setup() cuối cùng"""
    # Load models
    self.model = load_model()
    
    # Initialize resources
    self.tokenizer = load_tokenizer()
    
    # Must call super last
    super().setup()

def teardown(self):
    """Luôn gọi super().teardown() cuối cùng"""
    # Cleanup resources
    del self.model
    del self.tokenizer
    
    # Must call super last
    super().teardown()
```

### 2. Error Handling

```python
def _solve_task_impl(self, task: Task) -> Result:
    """BaseSubnet sẽ tự động retry nếu raise exception"""
    try:
        output = self.model.generate(task.task_data)
        return Result(...)
    except Exception as e:
        # Log error
        logger.error(f"Task failed: {e}")
        # Re-raise để trigger retry
        raise
```

### 3. Scoring với Confidence

```python
def _score_result_impl(self, task: Task, result: Result) -> Score:
    """Return score với confidence phù hợp"""
    score_value = calculate_score(task, result)
    
    # Higher confidence cho results tốt
    if score_value > 0.8:
        confidence = 0.95
    elif score_value > 0.5:
        confidence = 0.85
    else:
        confidence = 0.7
    
    return Score(
        value=score_value,
        confidence=confidence,
        metadata={"method": "reward_model"},
    )
```

### 4. Metadata cho Debugging

```python
def _solve_task_impl(self, task: Task) -> Result:
    """Thêm metadata hữu ích"""
    import time
    
    start_time = time.time()
    output = self.model.generate(task.task_data)
    solve_time = time.time() - start_time
    
    return Result(
        task_id=task.task_id,
        result_data={"output": output},
        miner_uid=task.context.miner_uid,
        metadata={
            "solve_time": solve_time,
            "model_version": "v1.0",
            "gpu_used": True,
        },
    )
```

## Configuration

### Subnet Config

```python
config = {
    # Cache settings
    "enable_cache": True,
    "cache_size": 1000,
    
    # Timeout settings
    "task_timeout": 60.0,  # seconds
    
    # Retry settings
    "max_retries": 3,
    
    # Custom settings
    "model_name": "gpt2",
    "batch_size": 8,
    "device": "cuda",
}

subnet = MySubnet(config=config)
```

## Testing

### Unit Tests

```python
import pytest
from sdk.ai_ml.core.protocol import TaskContext, Task, Result, Score

def test_subnet_initialization():
    subnet = MySubnet()
    assert not subnet.is_ready()
    
    subnet.setup()
    assert subnet.is_ready()

def test_create_task():
    subnet = MySubnet()
    subnet.setup()
    
    context = TaskContext(
        miner_uid="test_miner",
        difficulty=0.5,
        subnet_uid=1,
        cycle=1,
    )
    
    task = subnet.create_task(context)
    assert task.task_id is not None
    assert task.context == context

def test_solve_task():
    subnet = MySubnet()
    subnet.setup()
    
    # Create and solve
    task = subnet.create_task(context)
    result = subnet.solve_task(task)
    
    assert result.task_id == task.task_id
    assert result.execution_time is not None

def test_cache():
    subnet = MySubnet(config={"enable_cache": True})
    subnet.setup()
    
    task = subnet.create_task(context)
    
    # First solve - cache miss
    result1 = subnet.solve_task(task)
    metrics1 = subnet.get_metrics()
    assert metrics1["cache"]["misses"] == 1
    
    # Second solve - cache hit
    result2 = subnet.solve_task(task)
    metrics2 = subnet.get_metrics()
    assert metrics2["cache"]["hits"] == 1
```

## Migration từ Code Cũ

### Trước (Old Protocol)

```python
from sdk.subnets.protocol import SubnetProtocol

class OldSubnet(SubnetProtocol):
    def create_task(self, miner_uid: str, difficulty: float):
        # No structure
        return {"prompt": "test"}
    
    def score_result(self, task_data, result_data):
        # No error handling, no metrics
        return 0.8
    
    def solve_task(self, task_data):
        # No timeout, no retry
        return {"output": "test"}
```

### Sau (New AI/ML Layer)

```python
from sdk.ai_ml.subnets.base import BaseSubnet
from sdk.ai_ml.core.protocol import TaskContext, Task, Result, Score

class NewSubnet(BaseSubnet):
    def _create_task_impl(self, context: TaskContext) -> Task:
        # Structured Task object
        return Task(
            task_id=generate_id(),
            task_data={"prompt": "test"},
            context=context,
        )
    
    def _score_result_impl(self, task: Task, result: Result) -> Score:
        # Structured Score với confidence
        return Score(value=0.8, confidence=0.9)
    
    def _solve_task_impl(self, task: Task) -> Result:
        # Automatic timeout, retry, metrics
        return Result(
            task_id=task.task_id,
            result_data={"output": "test"},
            miner_uid=task.context.miner_uid,
        )
```

## Roadmap

### Phase 1: Foundation ✅ (Complete)
- [x] Enhanced SubnetProtocol
- [x] BaseSubnet với cache, retry, metrics
- [x] Documentation và examples

### Phase 2: Production Subnets (Week 3-4)
- [ ] TextGenerationSubnet với real LLM
- [ ] Model loading và management
- [ ] Reward model integration

### Phase 3: zkML Integration (Week 5-6)
- [ ] ProofGenerator
- [ ] EZKL integration
- [ ] Proof verification

### Phase 4: Agent Refactoring (Week 7-8)
- [ ] Refactor MinerAgent
- [ ] Separate concerns
- [ ] Remove deprecated code

## Support

Có câu hỏi? Tạo issue tại:
- GitHub: https://github.com/sonson0910/moderntensor/issues

## License

MIT License - See LICENSE file for details
