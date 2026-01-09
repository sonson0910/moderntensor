"""
Integration tests for Axon ↔ Dendrite communication.

Tests the complete communication flow between validators (Dendrite) and miners (Axon).
"""

import sys
import os

# Add parent directory to path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '../..'))

import asyncio
import time
from sdk.axon import Axon, AxonConfig
from sdk.dendrite import Dendrite, DendriteConfig
from sdk.synapse import Synapse, ForwardRequest, ForwardResponse


async def setup_test_axon(port: int = 8092):
    """Setup a test Axon server."""
    config = AxonConfig(
        uid=f"test-miner-{port}",
        host="127.0.0.1",
        port=port,
        authentication_enabled=False,  # Disable for testing
        rate_limiting_enabled=False,    # Disable for testing
        log_level="WARNING",
    )
    
    axon = Axon(config=config)
    
    # Attach test forward handler
    async def forward_handler(request):
        """Simple forward handler that echoes input."""
        try:
            data = await request.json()
        except:
            # Handle non-JSON requests
            data = {}
        
        input_text = data.get('input', 'no input')
        
        # Simulate processing time
        await asyncio.sleep(0.1)
        
        return {
            "output": f"Processed: {input_text}",
            "model": "test-model",
            "success": True,
            "processing_time": 0.1,
        }
    
    # Attach test backward handler
    async def backward_handler(request):
        """Simple backward handler."""
        data = await request.json()
        return {
            "success": True,
            "applied": True,
            "update_count": 100,
        }
    
    axon.attach("/forward", forward_handler, methods=["POST"])
    axon.attach("/backward", backward_handler, methods=["POST"])
    
    # Start server in background
    asyncio.create_task(axon.start(blocking=False))
    
    # Wait for server to start (longer delay)
    await asyncio.sleep(2)
    
    return axon


async def test_basic_query():
    """Test basic Dendrite query to Axon."""
    print("\n" + "="*60)
    print("Test: Basic Dendrite → Axon Query")
    print("="*60)
    
    # Setup Axon server
    print("Setting up Axon server...")
    axon = await setup_test_axon(port=8092)
    
    try:
        # Setup Dendrite client
        print("Setting up Dendrite client...")
        config = DendriteConfig(
            timeout=10.0,
            max_retries=2,
            parallel_queries=False,
        )
        dendrite = Dendrite(config=config)
        
        # Test query
        print("Sending query to Axon...")
        endpoint = "http://127.0.0.1:8092/forward"
        query_data = {
            "input": "Hello, world!",
            "model": "test-model",
        }
        
        response = await dendrite.query_single(
            endpoint=endpoint,
            data=query_data,
            timeout=10.0,
        )
        
        # Verify response
        assert response is not None, "Response should not be None"
        assert response.get('success') == True, "Response should be successful"
        assert 'output' in response, "Response should contain output"
        assert "Processed: Hello, world!" in response['output'], "Output should contain processed input"
        
        print(f"✓ Query successful: {response['output']}")
        
        # Get metrics
        metrics = dendrite.get_metrics()
        print(f"✓ Metrics: {metrics.successful_queries} successful, {metrics.failed_queries} failed")
        
        await dendrite.close()
        print("✅ Basic query test PASSED!")
        
    finally:
        await axon.stop()


async def test_parallel_queries():
    """Test parallel queries to multiple Axon servers."""
    print("\n" + "="*60)
    print("Test: Parallel Queries to Multiple Axons")
    print("="*60)
    
    # Setup multiple Axon servers
    print("Setting up multiple Axon servers...")
    axon1 = await setup_test_axon(port=8093)
    axon2 = await setup_test_axon(port=8094)
    axon3 = await setup_test_axon(port=8095)
    
    try:
        # Setup Dendrite client
        print("Setting up Dendrite client...")
        config = DendriteConfig(
            timeout=10.0,
            max_retries=2,
            parallel_queries=True,
            max_parallel_queries=5,
            aggregation_strategy="majority",
        )
        dendrite = Dendrite(config=config)
        
        # Test parallel query
        print("Sending parallel queries...")
        endpoints = [
            "http://127.0.0.1:8093/forward",
            "http://127.0.0.1:8094/forward",
            "http://127.0.0.1:8095/forward",
        ]
        query_data = {
            "input": "Test parallel",
            "model": "test-model",
        }
        
        result = await dendrite.query(
            endpoints=endpoints,
            data=query_data,
            aggregate=False,  # Get all responses
        )
        
        # Verify responses
        assert result is not None, "Result should not be None"
        assert len(result) == 3, f"Should get 3 responses, got {len(result)}"
        
        successful = [r for r in result if r is not None and r.get('success')]
        assert len(successful) == 3, f"All 3 should succeed, got {len(successful)}"
        
        print(f"✓ Received {len(successful)}/3 successful responses")
        
        # Get metrics
        metrics = dendrite.get_metrics()
        print(f"✓ Metrics: {metrics.successful_queries} successful, {metrics.failed_queries} failed")
        
        await dendrite.close()
        print("✅ Parallel queries test PASSED!")
        
    finally:
        await axon1.stop()
        await axon2.stop()
        await axon3.stop()


async def test_with_synapse_protocol():
    """Test using Synapse protocol messages."""
    print("\n" + "="*60)
    print("Test: Dendrite → Axon with Synapse Protocol")
    print("="*60)
    
    # Setup Axon server
    print("Setting up Axon server...")
    axon = await setup_test_axon(port=8096)
    
    try:
        # Setup Dendrite client
        print("Setting up Dendrite client...")
        dendrite = Dendrite()
        
        # Create Synapse request
        print("Creating Synapse request...")
        forward_req = ForwardRequest(
            input="Test with Synapse",
            model="test-model",
            temperature=0.7,
        )
        
        synapse_req = Synapse.create_request(
            message_type="forward",
            payload=forward_req.model_dump(),
            sender_uid="validator_001",
            receiver_uid="miner_001",
        )
        
        # Validate request
        Synapse.validate_request(synapse_req)
        print("✓ Synapse request validated")
        
        # Send query (using payload)
        print("Sending query...")
        endpoint = "http://127.0.0.1:8096/forward"
        response = await dendrite.query_single(
            endpoint=endpoint,
            data=synapse_req.payload,
            timeout=10.0,
        )
        
        # Verify response
        assert response is not None, "Response should not be None"
        assert response.get('success') == True, "Response should be successful"
        
        # Create Synapse response
        forward_resp = ForwardResponse(
            output=response['output'],
            model=response['model'],
            success=response['success'],
            processing_time=response['processing_time'],
        )
        
        synapse_resp = Synapse.create_response(
            message_type="forward",
            payload=forward_resp.model_dump(),
            request_id=synapse_req.request_id,
            success=True,
            status_code=200,
        )
        
        # Validate response
        Synapse.validate_response(synapse_resp)
        print("✓ Synapse response validated")
        
        print(f"✓ Output: {forward_resp.output}")
        
        await dendrite.close()
        print("✅ Synapse protocol test PASSED!")
        
    finally:
        await axon.stop()


async def test_error_handling():
    """Test error handling when Axon is unavailable."""
    print("\n" + "="*60)
    print("Test: Error Handling (Unavailable Axon)")
    print("="*60)
    
    # Setup Dendrite client
    print("Setting up Dendrite client...")
    config = DendriteConfig(
        timeout=2.0,
        max_retries=2,
        retry_delay=0.5,
    )
    dendrite = Dendrite(config=config)
    
    try:
        # Test query to non-existent server
        print("Sending query to unavailable server...")
        endpoint = "http://127.0.0.1:9999/forward"  # Non-existent port
        query_data = {"input": "test"}
        
        response = await dendrite.query_single(
            endpoint=endpoint,
            data=query_data,
            timeout=2.0,
        )
        
        # Should return None on failure
        assert response is None, "Response should be None for unavailable server"
        print("✓ Correctly handled unavailable server")
        
        # Get metrics
        metrics = dendrite.get_metrics()
        # Note: metrics.total_queries reflects actual attempts, not failed
        print(f"✓ Metrics: {metrics.total_queries} total queries, {metrics.failed_queries} explicitly failed")
        
        await dendrite.close()
        print("✅ Error handling test PASSED!")
        
    except Exception as e:
        await dendrite.close()
        print(f"❌ Error handling test FAILED: {e}")
        raise


async def test_circuit_breaker():
    """Test circuit breaker functionality."""
    print("\n" + "="*60)
    print("Test: Circuit Breaker")
    print("="*60)
    
    # Setup Dendrite client with circuit breaker
    print("Setting up Dendrite client with circuit breaker...")
    config = DendriteConfig(
        timeout=1.0,
        max_retries=0,  # No retries for faster testing
        circuit_breaker_enabled=True,
        circuit_breaker_threshold=3,  # Open after 3 failures
        circuit_breaker_timeout=2.0,  # Attempt reset after 2s
    )
    dendrite = Dendrite(config=config)
    
    try:
        endpoint = "http://127.0.0.1:9998/forward"  # Non-existent
        query_data = {"input": "test"}
        
        # Make 3 failed requests to trigger circuit breaker
        print("Making requests to trigger circuit breaker...")
        for i in range(3):
            response = await dendrite.query_single(
                endpoint=endpoint,
                data=query_data,
                retry=False,
            )
            assert response is None
            print(f"  ✓ Request {i+1} failed (expected)")
        
        # Circuit should be open now
        # Next request should fail immediately without attempting connection
        print("Testing circuit breaker is open...")
        start_time = time.time()
        response = await dendrite.query_single(
            endpoint=endpoint,
            data=query_data,
            retry=False,
        )
        elapsed = time.time() - start_time
        
        assert response is None
        assert elapsed < 0.5, f"Should fail fast with open circuit, took {elapsed:.2f}s"
        print(f"✓ Circuit breaker prevented request (took {elapsed:.3f}s)")
        
        await dendrite.close()
        print("✅ Circuit breaker test PASSED!")
        
    except Exception as e:
        await dendrite.close()
        print(f"❌ Circuit breaker test FAILED: {e}")
        raise


async def main():
    """Run all integration tests."""
    print("\n" + "="*60)
    print("Axon ↔ Dendrite Integration Tests")
    print("="*60)
    
    all_passed = True
    
    try:
        await test_basic_query()
    except Exception as e:
        print(f"❌ Basic query test failed: {e}")
        all_passed = False
    
    try:
        await test_parallel_queries()
    except Exception as e:
        print(f"❌ Parallel queries test failed: {e}")
        all_passed = False
    
    try:
        await test_with_synapse_protocol()
    except Exception as e:
        print(f"❌ Synapse protocol test failed: {e}")
        all_passed = False
    
    try:
        await test_error_handling()
    except Exception as e:
        print(f"❌ Error handling test failed: {e}")
        all_passed = False
    
    try:
        await test_circuit_breaker()
    except Exception as e:
        print(f"❌ Circuit breaker test failed: {e}")
        all_passed = False
    
    print("\n" + "="*60)
    if all_passed:
        print("✅ ALL INTEGRATION TESTS PASSED!")
        print("="*60)
        print("\n✅ Integration Summary:")
        print("  • Dendrite can successfully query Axon servers")
        print("  • Parallel queries work correctly")
        print("  • Synapse protocol integration works")
        print("  • Error handling works as expected")
        print("  • Circuit breaker prevents cascading failures")
        print("\n🎯 Axon ↔ Dendrite integration: COMPLETE")
        print("="*60 + "\n")
        return 0
    else:
        print("❌ SOME INTEGRATION TESTS FAILED!")
        print("="*60 + "\n")
        return 1


if __name__ == "__main__":
    exit(asyncio.run(main()))
