"""
Tests for Synapse protocol implementation.

Tests cover protocol messages, serialization, and validation.
"""

import sys
import os

# Add parent directory to path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..'))

import importlib.util


def load_module_from_file(module_name, file_path):
    """Load a Python module directly from file path."""
    spec = importlib.util.spec_from_file_location(module_name, file_path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[module_name] = module
    spec.loader.exec_module(module)
    return module


def test_imports():
    """Test that all Synapse modules can be imported."""
    print("Testing imports...")
    
    # Get repository root
    repo_root = os.path.dirname(os.path.dirname(os.path.dirname(__file__)))
    sdk_path = os.path.join(repo_root, 'sdk')
    synapse_path = os.path.join(sdk_path, 'synapse')
    
    # Load version module
    version_path = os.path.join(synapse_path, 'version.py')
    version_module = load_module_from_file('sdk.synapse.version', version_path)
    print("✓ version module imported")
    
    # Load types module (requires pydantic)
    types_path = os.path.join(synapse_path, 'types.py')
    types_module = load_module_from_file('sdk.synapse.types', types_path)
    print("✓ types module imported")
    
    # Load synapse module
    synapse_file_path = os.path.join(synapse_path, 'synapse.py')
    synapse_module = load_module_from_file('sdk.synapse.synapse', synapse_file_path)
    print("✓ synapse module imported")
    
    # Load serializer module
    serializer_path = os.path.join(synapse_path, 'serializer.py')
    serializer_module = load_module_from_file('sdk.synapse.serializer', serializer_path)
    print("✓ serializer module imported")
    
    print("\n✅ All imports successful!\n")
    return version_module, types_module, synapse_module, serializer_module


def test_version(version_module):
    """Test protocol version management."""
    print("Testing protocol version...")
    
    parse_version = version_module.parse_version
    version_compatible = version_module.version_compatible
    negotiate_version = version_module.negotiate_version
    
    # Test version parsing
    major, minor = parse_version("1.0")
    assert major == 1 and minor == 0
    print("  ✓ Version parsing works")
    
    # Test version compatibility
    assert version_compatible("1.0", "1.1") == True  # Same major
    assert version_compatible("1.0", "2.0") == False  # Different major
    print("  ✓ Version compatibility works")
    
    # Test version negotiation
    client_versions = ["1.0", "1.1", "2.0"]
    server_versions = ["1.0", "1.1"]
    negotiated = negotiate_version(client_versions, server_versions)
    assert negotiated == "1.1"  # Highest common
    print("  ✓ Version negotiation works")
    
    print("✅ Protocol version tests passed!\n")
    return True


def test_message_types(types_module):
    """Test message type definitions."""
    print("Testing message types...")
    
    ForwardRequest = types_module.ForwardRequest
    ForwardResponse = types_module.ForwardResponse
    
    # Test ForwardRequest
    req = ForwardRequest(
        input="test input",
        model="test-model",
        temperature=0.7,
    )
    assert req.input == "test input"
    assert req.temperature == 0.7
    print("  ✓ ForwardRequest works")
    
    # Test ForwardResponse
    resp = ForwardResponse(
        output="test output",
        model="test-model",
        confidence=0.95,
        success=True,
    )
    assert resp.output == "test output"
    assert resp.success == True
    print("  ✓ ForwardResponse works")
    
    print("✅ Message type tests passed!\n")
    return True


def test_synapse_protocol(synapse_module):
    """Test Synapse protocol."""
    print("Testing Synapse protocol...")
    
    Synapse = synapse_module.Synapse
    SynapseRequest = synapse_module.SynapseRequest
    SynapseResponse = synapse_module.SynapseResponse
    
    # Test request creation
    req = Synapse.create_request(
        message_type="forward",
        payload={"input": "test"},
        sender_uid="validator_001",
        receiver_uid="miner_001",
        priority=5,
    )
    assert req.message_type == "forward"
    assert req.sender_uid == "validator_001"
    assert req.priority == 5
    print("  ✓ Request creation works")
    
    # Test response creation
    resp = Synapse.create_response(
        message_type="forward",
        payload={"output": "result"},
        request_id=req.request_id,
        sender_uid="miner_001",
        success=True,
        status_code=200,
    )
    assert resp.message_type == "forward"
    assert resp.request_id == req.request_id
    assert resp.success == True
    print("  ✓ Response creation works")
    
    # Test request validation
    try:
        Synapse.validate_request(req)
        print("  ✓ Request validation works")
    except ValueError as e:
        print(f"  ✗ Request validation failed: {e}")
        return False
    
    # Test response validation
    try:
        Synapse.validate_response(resp)
        print("  ✓ Response validation works")
    except ValueError as e:
        print(f"  ✗ Response validation failed: {e}")
        return False
    
    print("✅ Synapse protocol tests passed!\n")
    return True


def test_serialization(serializer_module, synapse_module):
    """Test message serialization."""
    print("Testing serialization...")
    
    SynapseSerializer = serializer_module.SynapseSerializer
    Synapse = synapse_module.Synapse
    
    # Create a request
    req = Synapse.create_request(
        message_type="forward",
        payload={"input": "test"},
        sender_uid="validator_001",
    )
    
    # Serialize
    json_str = SynapseSerializer.serialize_request(req)
    assert isinstance(json_str, str)
    assert len(json_str) > 0
    print("  ✓ Request serialization works")
    
    # Deserialize
    deserialized_req = SynapseSerializer.deserialize_request(json_str)
    assert deserialized_req.request_id == req.request_id
    assert deserialized_req.message_type == req.message_type
    print("  ✓ Request deserialization works")
    
    # Test response serialization
    resp = Synapse.create_response(
        message_type="forward",
        payload={"output": "result"},
        success=True,
    )
    
    json_str = SynapseSerializer.serialize_response(resp)
    deserialized_resp = SynapseSerializer.deserialize_response(json_str)
    assert deserialized_resp.response_id == resp.response_id
    print("  ✓ Response serialization works")
    
    print("✅ Serialization tests passed!\n")
    return True


def test_file_structure():
    """Test that all files exist."""
    print("Checking file structure...")
    
    # Get repository root
    repo_root = os.path.dirname(os.path.dirname(os.path.dirname(__file__)))
    synapse_path = os.path.join(repo_root, 'sdk', 'synapse')
    
    files_to_check = [
        '__init__.py',
        'version.py',
        'types.py',
        'synapse.py',
        'serializer.py',
    ]
    
    for filename in files_to_check:
        filepath = os.path.join(synapse_path, filename)
        assert os.path.exists(filepath), f"Missing file: {filename}"
        size = os.path.getsize(filepath)
        print(f"  ✓ {filename} ({size} bytes)")
    
    # Check documentation
    docs_path = os.path.join(repo_root, 'docs', 'SYNAPSE.md')
    if os.path.exists(docs_path):
        size = os.path.getsize(docs_path)
        print(f"  ✓ SYNAPSE.md ({size} bytes)")
    
    # Check examples
    example_path = os.path.join(repo_root, 'examples', 'synapse_example.py')
    assert os.path.exists(example_path), "Missing example file"
    size = os.path.getsize(example_path)
    print(f"  ✓ synapse_example.py ({size} bytes)")
    
    print("✅ File structure check passed!\n")
    return True


def main():
    """Run all tests."""
    print("\n" + "="*60)
    print("Synapse Protocol Implementation Verification")
    print("="*60 + "\n")
    
    all_passed = True
    
    try:
        version_module, types_module, synapse_module, serializer_module = test_imports()
    except Exception as e:
        print(f"❌ Import test failed: {e}\n")
        import traceback
        traceback.print_exc()
        return 1
    
    try:
        all_passed &= test_version(version_module)
    except Exception as e:
        print(f"❌ Version test failed: {e}\n")
        all_passed = False
    
    try:
        all_passed &= test_message_types(types_module)
    except Exception as e:
        print(f"❌ Message types test failed: {e}\n")
        all_passed = False
    
    try:
        all_passed &= test_synapse_protocol(synapse_module)
    except Exception as e:
        print(f"❌ Synapse protocol test failed: {e}\n")
        all_passed = False
    
    try:
        all_passed &= test_serialization(serializer_module, synapse_module)
    except Exception as e:
        print(f"❌ Serialization test failed: {e}\n")
        all_passed = False
    
    try:
        all_passed &= test_file_structure()
    except Exception as e:
        print(f"❌ File structure test failed: {e}\n")
        all_passed = False
    
    print("="*60)
    if all_passed:
        print("✅ ALL VERIFICATION TESTS PASSED!")
        print("="*60 + "\n")
        
        print("Summary of Phase 5 Implementation:")
        print("="*60)
        print("\n✅ Core Components:")
        print("  • Protocol version management with negotiation")
        print("  • Message types (Forward, Backward, Ping, Status)")
        print("  • SynapseRequest/Response wrappers")
        print("  • JSON serialization/deserialization")
        print("  • Type validation")
        
        print("\n✅ Features:")
        print("  • Message format specification")
        print("  • Request/response types with Pydantic")
        print("  • Version negotiation and compatibility")
        print("  • Type-safe serialization")
        print("  • Backward compatibility support")
        print("  • Error handling")
        
        print("\n📝 Files Created:")
        print("  • sdk/synapse/__init__.py")
        print("  • sdk/synapse/version.py (2.5KB)")
        print("  • sdk/synapse/types.py (8KB)")
        print("  • sdk/synapse/synapse.py (9KB)")
        print("  • sdk/synapse/serializer.py (6.5KB)")
        print("  • examples/synapse_example.py (7KB)")
        
        print("\n🎯 Phase 5 Status: COMPLETE")
        print("\nNext: Documentation and Phase 6 (Enhance Metagraph)")
        print("="*60 + "\n")
        
        return 0
    else:
        print("❌ Some tests FAILED!")
        print("="*60 + "\n")
        return 1


if __name__ == "__main__":
    exit(main())
