"""
Example usage of the Axon server for miners and validators.

This example demonstrates how to set up and use an Axon server
to serve AI/ML models or validation services.
"""

import asyncio
from fastapi import Request
from sdk.axon import Axon, AxonConfig


async def forward_handler(request: Request):
    """
    Handler for forward requests (inference).
    
    This is where you would process incoming requests for AI/ML inference.
    Replace this with your actual model inference logic.
    """
    # Get request data
    data = await request.json()
    
    # Example: Process with your AI/ML model
    # result = your_model.predict(data['input'])
    
    result = {
        "success": True,
        "output": f"Processed input: {data.get('input', 'N/A')}",
        "model": "example-model-v1",
    }
    
    return result


async def backward_handler(request: Request):
    """
    Handler for backward requests (gradient/feedback).
    
    This is where you would handle gradient updates or feedback.
    Replace this with your actual gradient processing logic.
    """
    # Get request data
    data = await request.json()
    
    # Example: Process gradients
    # your_model.update_weights(data['gradients'])
    
    result = {
        "success": True,
        "gradient_received": True,
        "loss": data.get("loss", 0.0),
    }
    
    return result


async def main():
    """Main function to run the Axon server."""
    
    # Create configuration
    config = AxonConfig(
        host="0.0.0.0",
        port=8091,
        uid="miner-001",
        
        # Security settings
        authentication_enabled=True,
        rate_limiting_enabled=True,
        rate_limit_requests=100,
        rate_limit_window=60,
        
        # DDoS protection
        ddos_protection_enabled=True,
        max_concurrent_requests=50,
        
        # Monitoring
        metrics_enabled=True,
        health_check_enabled=True,
        
        # Logging
        log_requests=True,
        log_level="INFO",
    )
    
    # Create Axon server
    axon = Axon(config=config)
    
    # Register API key for testing
    api_key = axon.register_api_key("validator-001")
    print(f"\n{'='*60}")
    print(f"API Key for validator-001: {api_key}")
    print(f"Use this key in the X-API-Key header")
    print(f"{'='*60}\n")
    
    # Attach request handlers
    axon.attach("/forward", forward_handler, methods=["POST"])
    axon.attach("/backward", backward_handler, methods=["POST"])
    
    # Optional: Add IP to whitelist (if whitelist is enabled)
    # axon.whitelist_ip("127.0.0.1")
    
    print("Starting Axon server...")
    print(f"Server will be available at http://{config.host}:{config.port}")
    print(f"Health check: http://{config.host}:{config.port}/health")
    print(f"Metrics: http://{config.host}:{config.port}/metrics")
    print(f"Info: http://{config.host}:{config.port}/info")
    print("\nPress Ctrl+C to stop the server\n")
    
    # Start the server
    try:
        await axon.start(blocking=True)
    except KeyboardInterrupt:
        print("\nShutting down...")
        await axon.stop()


if __name__ == "__main__":
    asyncio.run(main())
