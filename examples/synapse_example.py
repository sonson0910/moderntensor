"""
Example usage of the Synapse protocol.

Demonstrates how to create and use Synapse messages for
communication between Axon and Dendrite.
"""

from sdk.synapse import (
    Synapse,
    SynapseRequest,
    SynapseResponse,
    ForwardRequest,
    ForwardResponse,
    SynapseSerializer,
)


def basic_example():
    """Basic Synapse usage example."""
    print("\n" + "="*60)
    print("Basic Synapse Protocol Example")
    print("="*60 + "\n")
    
    # Create a forward request
    forward_req = ForwardRequest(
        input="What is artificial intelligence?",
        model="gpt-example",
        temperature=0.7,
        max_tokens=100,
    )
    
    print("Forward Request:")
    print(f"  Input: {forward_req.input}")
    print(f"  Model: {forward_req.model}")
    print(f"  Temperature: {forward_req.temperature}")
    
    # Wrap in Synapse request
    synapse_req = Synapse.create_request(
        message_type="forward",
        payload=forward_req.model_dump(),
        sender_uid="validator_001",
        receiver_uid="miner_001",
        priority=5,
    )
    
    print(f"\nSynapse Request:")
    print(f"  ID: {synapse_req.request_id}")
    print(f"  Type: {synapse_req.message_type}")
    print(f"  Sender: {synapse_req.sender_uid}")
    print(f"  Receiver: {synapse_req.receiver_uid}")
    
    # Create a forward response
    forward_resp = ForwardResponse(
        output="Artificial intelligence is the simulation of human intelligence...",
        model="gpt-example",
        confidence=0.95,
        tokens_used=50,
        processing_time=0.123,
        success=True,
    )
    
    print(f"\nForward Response:")
    print(f"  Output: {forward_resp.output[:50]}...")
    print(f"  Confidence: {forward_resp.confidence}")
    print(f"  Tokens: {forward_resp.tokens_used}")
    print(f"  Time: {forward_resp.processing_time}s")
    
    # Wrap in Synapse response
    synapse_resp = Synapse.create_response(
        message_type="forward",
        payload=forward_resp.model_dump(),
        request_id=synapse_req.request_id,
        sender_uid="miner_001",
        success=True,
        status_code=200,
    )
    
    print(f"\nSynapse Response:")
    print(f"  ID: {synapse_resp.response_id}")
    print(f"  Request ID: {synapse_resp.request_id}")
    print(f"  Success: {synapse_resp.success}")
    print(f"  Status: {synapse_resp.status_code}")


def serialization_example():
    """Serialization/deserialization example."""
    print("\n" + "="*60)
    print("Serialization Example")
    print("="*60 + "\n")
    
    # Create a request
    forward_req = ForwardRequest(
        input="Test input",
        model="test-model",
    )
    
    synapse_req = Synapse.create_request(
        message_type="forward",
        payload=forward_req.model_dump(),
        sender_uid="validator_001",
    )
    
    # Serialize to JSON
    json_str = SynapseSerializer.serialize_request(synapse_req)
    print("Serialized request:")
    print(json_str[:200] + "...")
    
    # Deserialize from JSON
    deserialized_req = SynapseSerializer.deserialize_request(json_str)
    print(f"\nDeserialized request:")
    print(f"  ID: {deserialized_req.request_id}")
    print(f"  Type: {deserialized_req.message_type}")
    print(f"  Sender: {deserialized_req.sender_uid}")
    
    # Verify it matches
    assert deserialized_req.request_id == synapse_req.request_id
    print("\n✓ Serialization/deserialization successful!")


def typed_message_example():
    """Typed message serialization example."""
    print("\n" + "="*60)
    print("Typed Message Example")
    print("="*60 + "\n")
    
    # Create typed request
    forward_req = ForwardRequest(
        input="Explain machine learning",
        model="ml-model",
        temperature=0.8,
        max_tokens=200,
    )
    
    # Serialize typed request
    json_str = SynapseSerializer.serialize_typed_request("forward", forward_req)
    print("Typed request JSON:")
    print(json_str[:150] + "...")
    
    # Deserialize typed request
    deserialized = SynapseSerializer.deserialize_typed_request("forward", json_str)
    print(f"\nDeserialized typed request:")
    print(f"  Input: {deserialized.input}")
    print(f"  Model: {deserialized.model}")
    print(f"  Temperature: {deserialized.temperature}")
    
    assert isinstance(deserialized, ForwardRequest)
    print("\n✓ Typed message handling successful!")


def validation_example():
    """Request/response validation example."""
    print("\n" + "="*60)
    print("Validation Example")
    print("="*60 + "\n")
    
    # Create and validate request
    synapse_req = Synapse.create_request(
        message_type="forward",
        payload={"input": "test"},
        priority=5,
        timeout=30.0,
    )
    
    try:
        Synapse.validate_request(synapse_req)
        print("✓ Request validation passed")
    except ValueError as e:
        print(f"✗ Request validation failed: {e}")
    
    # Create and validate response
    synapse_resp = Synapse.create_response(
        message_type="forward",
        payload={"output": "result"},
        success=True,
        status_code=200,
    )
    
    try:
        Synapse.validate_response(synapse_resp)
        print("✓ Response validation passed")
    except ValueError as e:
        print(f"✗ Response validation failed: {e}")
    
    # Test invalid priority
    print("\nTesting invalid priority...")
    invalid_req = Synapse.create_request(
        message_type="forward",
        payload={"input": "test"},
        priority=15,  # Invalid: > 10
    )
    
    try:
        Synapse.validate_request(invalid_req)
        print("✗ Should have failed validation")
    except ValueError as e:
        print(f"✓ Correctly rejected: {e}")


def error_handling_example():
    """Error response example."""
    print("\n" + "="*60)
    print("Error Handling Example")
    print("="*60 + "\n")
    
    # Create error response
    error_resp = Synapse.create_response(
        message_type="forward",
        payload={},
        success=False,
        status_code=500,
        error="Model inference failed",
        error_code="INFERENCE_ERROR",
    )
    
    print("Error Response:")
    print(f"  Success: {error_resp.success}")
    print(f"  Status: {error_resp.status_code}")
    print(f"  Error: {error_resp.error}")
    print(f"  Error Code: {error_resp.error_code}")
    
    # Wrap error response
    forward_error = ForwardResponse(
        output=None,
        success=False,
        error="Timeout occurred",
        error_code="TIMEOUT",
    )
    
    print("\nForward Error Response:")
    print(f"  Success: {forward_error.success}")
    print(f"  Error: {forward_error.error}")


def main():
    """Run all examples."""
    print("\n" + "="*60)
    print("Synapse Protocol Examples")
    print("="*60)
    
    basic_example()
    serialization_example()
    typed_message_example()
    validation_example()
    error_handling_example()
    
    print("\n" + "="*60)
    print("All examples completed!")
    print("="*60 + "\n")


if __name__ == "__main__":
    main()
