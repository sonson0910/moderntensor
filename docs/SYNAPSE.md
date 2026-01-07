# Synapse Protocol Documentation

## Overview

The Synapse protocol defines the message format and communication standard for Axon-Dendrite interactions in ModernTensor. It provides type-safe request/response structures with versioning, serialization, and validation.

## Features

### Core Features
- **Type-Safe Messages**: Pydantic models for all message types
- **Protocol Versioning**: Version negotiation and compatibility checking
- **JSON Serialization**: Efficient serialization/deserialization
- **Request/Response Pattern**: Structured communication flow
- **Metadata Support**: Rich metadata for routing and tracking

### Message Types
- **Forward**: AI/ML inference requests and responses
- **Backward**: Gradient/feedback for model updates
- **Ping**: Availability checking
- **Status**: Miner information and metrics

### Protocol Features
- **Version Negotiation**: Automatic version compatibility
- **Type Validation**: Runtime type checking with Pydantic
- **Error Handling**: Structured error responses
- **Backward Compatibility**: Support for multiple protocol versions

## Quick Start

### Basic Usage

```python
from sdk.synapse import (
    Synapse,
    ForwardRequest,
    ForwardResponse,
    SynapseSerializer,
)

# Create a forward request
forward_req = ForwardRequest(
    input="What is AI?",
    model="gpt-example",
    temperature=0.7,
)

# Wrap in Synapse request
synapse_req = Synapse.create_request(
    message_type="forward",
    payload=forward_req.model_dump(),
    sender_uid="validator_001",
    receiver_uid="miner_001",
)

# Serialize for transmission
json_data = SynapseSerializer.serialize_request(synapse_req)

# On receiver side: deserialize
received_req = SynapseSerializer.deserialize_request(json_data)

# Create response
forward_resp = ForwardResponse(
    output="AI is...",
    confidence=0.95,
    success=True,
)

synapse_resp = Synapse.create_response(
    message_type="forward",
    payload=forward_resp.model_dump(),
    request_id=synapse_req.request_id,
)
```

## API Reference

### Protocol Version

#### `ProtocolVersion`
Enum of supported protocol versions.

```python
from sdk.synapse import ProtocolVersion

# Current version
CURRENT_VERSION = ProtocolVersion.V1_0

# Available versions
V1_0 = "1.0"
V1_1 = "1.1"
V2_0 = "2.0"
```

#### `version_compatible(client_version, server_version) -> bool`
Check if two versions are compatible.

```python
from sdk.synapse import version_compatible

# Same major version = compatible
compatible = version_compatible("1.0", "1.1")  # True
compatible = version_compatible("1.0", "2.0")  # False
```

#### `negotiate_version(client_versions, server_versions) -> str`
Negotiate highest common version.

```python
from sdk.synapse import negotiate_version

client = ["1.0", "1.1", "2.0"]
server = ["1.0", "1.1"]
version = negotiate_version(client, server)  # Returns "1.1"
```

### Message Types

#### ForwardRequest
Request for AI/ML inference.

```python
from sdk.synapse import ForwardRequest

request = ForwardRequest(
    input="Text to process",
    model="gpt-example",
    temperature=0.7,
    max_tokens=100,
    top_p=0.9,
    request_id="req_123",
    priority=5,
)
```

**Fields:**
- `input` (Any, required): Input data
- `model` (str, optional): Model name
- `temperature` (float, 0-2): Sampling temperature
- `max_tokens` (int): Maximum tokens
- `top_p` (float, 0-1): Top-p sampling
- `request_id` (str): Request identifier
- `priority` (int, 0-10): Request priority

#### ForwardResponse
Response with inference result.

```python
from sdk.synapse import ForwardResponse

response = ForwardResponse(
    output="Generated text",
    model="gpt-example",
    confidence=0.95,
    tokens_used=50,
    processing_time=0.123,
    success=True,
)
```

**Fields:**
- `output` (Any, required): Model output
- `model` (str): Model used
- `confidence` (float, 0-1): Confidence score
- `tokens_used` (int): Tokens consumed
- `processing_time` (float): Processing duration
- `success` (bool): Success status
- `error` (str): Error message if failed

#### BackwardRequest
Gradient/feedback request.

```python
from sdk.synapse import BackwardRequest

request = BackwardRequest(
    loss=0.234,
    loss_type="cross_entropy",
    reward=0.85,
    request_id="req_123",
)
```

**Fields:**
- `gradients` (dict): Gradient values
- `loss` (float): Loss value
- `loss_type` (str): Loss function type
- `reward` (float): Reward signal
- `target` (Any): Target output

#### BackwardResponse
Confirmation of gradient receipt.

```python
from sdk.synapse import BackwardResponse

response = BackwardResponse(
    success=True,
    applied=True,
    update_count=1000,
)
```

### Synapse Protocol

#### `Synapse.create_request(...) -> SynapseRequest`
Create a protocol request.

```python
from sdk.synapse import Synapse

request = Synapse.create_request(
    message_type="forward",
    payload={"input": "data"},
    sender_uid="validator_001",
    receiver_uid="miner_001",
    priority=5,
    timeout=30.0,
)
```

**Parameters:**
- `message_type` (str): Message type (forward, backward, ping, status)
- `payload` (dict): Message payload
- `sender_uid` (str): Sender identifier
- `receiver_uid` (str): Receiver identifier
- `priority` (int, 0-10): Request priority
- `timeout` (float): Timeout in seconds

#### `Synapse.create_response(...) -> SynapseResponse`
Create a protocol response.

```python
response = Synapse.create_response(
    message_type="forward",
    payload={"output": "result"},
    request_id="req_123",
    sender_uid="miner_001",
    success=True,
    status_code=200,
)
```

**Parameters:**
- `message_type` (str): Message type
- `payload` (dict): Response payload
- `request_id` (str): Original request ID
- `sender_uid` (str): Sender identifier
- `success` (bool): Success status
- `status_code` (int): HTTP-style status code

#### `Synapse.validate_request(request) -> bool`
Validate a request.

```python
try:
    Synapse.validate_request(request)
    print("Valid request")
except ValueError as e:
    print(f"Invalid: {e}")
```

#### `Synapse.validate_response(response) -> bool`
Validate a response.

```python
try:
    Synapse.validate_response(response)
    print("Valid response")
except ValueError as e:
    print(f"Invalid: {e}")
```

### Serialization

#### `SynapseSerializer.serialize_request(request) -> str`
Serialize request to JSON.

```python
from sdk.synapse import SynapseSerializer

json_str = SynapseSerializer.serialize_request(request)
```

#### `SynapseSerializer.deserialize_request(data) -> SynapseRequest`
Deserialize JSON to request.

```python
request = SynapseSerializer.deserialize_request(json_str)
```

#### `SynapseSerializer.serialize_typed_request(type, request) -> str`
Serialize typed request.

```python
forward_req = ForwardRequest(input="test")
json_str = SynapseSerializer.serialize_typed_request("forward", forward_req)
```

#### `SynapseSerializer.deserialize_typed_request(type, data) -> BaseModel`
Deserialize to typed request.

```python
forward_req = SynapseSerializer.deserialize_typed_request("forward", json_str)
```

## Message Flow

### Forward Request Flow

```
Validator (Dendrite)                    Miner (Axon)
       |                                      |
       | 1. Create ForwardRequest             |
       |------------------------------------→ |
       |                                      |
       |                                 2. Process
       |                                      |
       | ←----------------------------------- | 3. Return ForwardResponse
       |                                      |
```

### Backward Request Flow

```
Validator (Dendrite)                    Miner (Axon)
       |                                      |
       | 1. Create BackwardRequest            |
       |    (with gradients)                  |
       |------------------------------------→ |
       |                                      |
       |                             2. Apply gradients
       |                                      |
       | ←----------------------------------- | 3. Return BackwardResponse
       |                                      |
```

## Protocol Structure

### SynapseRequest Structure

```json
{
  "protocol_version": "1.0",
  "request_id": "req_abc123",
  "sender_uid": "validator_001",
  "receiver_uid": "miner_001",
  "message_type": "forward",
  "payload": {
    "input": "What is AI?",
    "model": "gpt-example",
    "temperature": 0.7
  },
  "timestamp": "2026-01-07T14:00:00Z",
  "priority": 5,
  "timeout": 30.0,
  "metadata": {}
}
```

### SynapseResponse Structure

```json
{
  "protocol_version": "1.0",
  "response_id": "resp_xyz789",
  "request_id": "req_abc123",
  "sender_uid": "miner_001",
  "message_type": "forward",
  "payload": {
    "output": "AI is...",
    "confidence": 0.95,
    "tokens_used": 50,
    "processing_time": 0.123
  },
  "success": true,
  "status_code": 200,
  "timestamp": "2026-01-07T14:00:01Z",
  "metadata": {}
}
```

## Error Handling

### Error Response

```python
# Create error response
error_resp = Synapse.create_response(
    message_type="forward",
    payload={},
    success=False,
    status_code=500,
    error="Model inference failed",
    error_code="INFERENCE_ERROR",
)
```

### Status Codes

- `200`: Success
- `400`: Bad request (invalid input)
- `401`: Unauthorized (authentication failed)
- `404`: Not found (model not available)
- `408`: Timeout
- `429`: Rate limit exceeded
- `500`: Internal server error
- `503`: Service unavailable

## Version Negotiation

### Example

```python
from sdk.synapse import negotiate_version

# Client supports
client_versions = ["1.0", "1.1", "2.0"]

# Server supports
server_versions = ["1.0", "1.1"]

# Negotiate
try:
    version = negotiate_version(client_versions, server_versions)
    print(f"Using protocol version: {version}")  # "1.1"
except ValueError:
    print("No compatible version found")
```

## Integration with Axon/Dendrite

### Axon (Miner) Side

```python
from sdk.axon import Axon
from sdk.synapse import (
    SynapseSerializer,
    ForwardRequest,
    ForwardResponse,
)

axon = Axon()

@axon.attach("/forward", methods=["POST"])
async def forward_handler(request):
    # Deserialize Synapse request
    synapse_req = SynapseSerializer.deserialize_request(
        await request.json()
    )
    
    # Extract typed request
    forward_req = SynapseSerializer.deserialize_typed_request(
        "forward",
        synapse_req.payload
    )
    
    # Process
    output = process_model(forward_req.input)
    
    # Create typed response
    forward_resp = ForwardResponse(
        output=output,
        success=True,
    )
    
    # Wrap in Synapse response
    synapse_resp = Synapse.create_response(
        message_type="forward",
        payload=forward_resp.model_dump(),
        request_id=synapse_req.request_id,
    )
    
    return synapse_resp.model_dump()
```

### Dendrite (Validator) Side

```python
from sdk.dendrite import Dendrite
from sdk.synapse import (
    Synapse,
    ForwardRequest,
    SynapseSerializer,
)

dendrite = Dendrite()

# Create forward request
forward_req = ForwardRequest(
    input="What is AI?",
    model="gpt-example",
)

# Wrap in Synapse request
synapse_req = Synapse.create_request(
    message_type="forward",
    payload=forward_req.model_dump(),
    sender_uid="validator_001",
)

# Send to miner
synapse_resp = await dendrite.query_single(
    endpoint="http://miner:8091/forward",
    data=synapse_req.model_dump(),
)

# Parse response
forward_resp = SynapseSerializer.deserialize_typed_response(
    "forward",
    synapse_resp["payload"]
)

print(f"Output: {forward_resp.output}")
```

## Best Practices

### 1. Always Use Protocol Wrappers

```python
# Good: Use Synapse wrappers
synapse_req = Synapse.create_request(
    message_type="forward",
    payload=forward_req.model_dump(),
)

# Avoid: Raw dictionaries
raw_dict = {"type": "forward", "data": {...}}
```

### 2. Validate Messages

```python
# Validate before sending
Synapse.validate_request(synapse_req)

# Validate on receipt
Synapse.validate_response(synapse_resp)
```

### 3. Handle Protocol Versions

```python
# Check compatibility
if not version_compatible(client_ver, server_ver):
    # Negotiate or fail gracefully
    pass
```

### 4. Use Typed Messages

```python
# Good: Typed messages
forward_req = ForwardRequest(input="test")

# Avoid: Generic dictionaries
payload = {"input": "test"}
```

### 5. Include Request IDs

```python
# Always set request_id for tracking
request = Synapse.create_request(
    message_type="forward",
    payload={...},
    # request_id auto-generated if not provided
)
```

## See Also

- [Axon Server Documentation](./AXON.md) (Phase 3)
- [Dendrite Client Documentation](./DENDRITE.md) (Phase 4)
- [ModernTensor SDK Roadmap](../SDK_REDESIGN_ROADMAP.md)

## Support

For issues or questions:
- GitHub Issues: https://github.com/sonson0910/moderntensor/issues
- Documentation: See roadmap documents in repository root
