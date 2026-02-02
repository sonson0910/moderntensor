# Building AI Subnets

ModernTensor's unique power lies in its **Subnets**—self-contained economic markets where Miners produce intelligence and Validators verify it.

## What is a Subnet?

A Subnet is a specific domain of competition (e.g., Image Generation, Text LLMs, Price Prediction). Each subnet has its own:

* **Incentive Mechanism**: How miners are scored.
* **Validators**: Nodes that run the verification logic.
* **Miners**: Nodes that perform the actual AI inference.

## 1. Creating a Subnet

Currently, Subnet creation is permissioned (Governance control) during the Beta phase. Once mainnet launches, anyone can burn **MDT** to create a subnet.

```python
from moderntensor.sdk import LuxtensorClient

client = LuxtensorClient("http://localhost:8545")

# (Future) Register a new subnet
# tx = client.subnets.register(
#     name="My Custom LLM Subnet",
#     metadata_url="ipfs://..."
# )
```

## 2. Participating as a Miner

Miners listen for tasks from the network and execute them.

### Step 1: Register

You must stake MDT to become a miner.

```python
tx = client.subnets.register_neuron(
    subnet_id=1,
    role="miner",
    stake=1000.0  # MDT
)
print(f"Miner Registered: {tx}")
```

### Step 2: Run the Miner Loop

```python
from moderntensor.sdk import Miner

miner = Miner(subnet_id=1, keypair=my_keypair)

@miner.on_request
def handle_request(data):
    # Your Custom AI Logic Here
    result = my_custom_model.predict(data)
    return result

miner.run()
```

## 3. Participating as a Validator

Validators generate tasks and score the miners' responses.

### Step 1: Register

Validators require higher stake than miners.

```python
tx = client.subnets.register_neuron(
    subnet_id=1,
    role="validator",
    stake=10000.0  # MDT
)
```

### Step 2: Run Validator Logic

```python
from moderntensor.sdk import Validator

validator = Validator(subnet_id=1, keypair=my_keypair)

def validation_logic():
    # 1. Send challenge to miners
    responses = validator.query_miners(data="Test Prompt")

    # 2. Score responses
    scores = []
    for resp in responses:
        score = evaluate(resp) # Your custom evaluation
        scores.append(score)

    # 3. Submit weights to blockchain
    validator.set_weights(scores)

validator.run(step_function=validation_logic)
```
