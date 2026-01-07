"""
Synapse message types - Request/response data structures.

Defines all message types for Axon/Dendrite communication.
"""

from typing import Any, Dict, Optional, List
from pydantic import BaseModel, Field
from datetime import datetime


class ForwardRequest(BaseModel):
    """
    Forward request for AI/ML inference.
    
    Sent from Dendrite (validator) to Axon (miner) for model inference.
    """
    
    # Input data
    input: Any = Field(..., description="Input data for model inference")
    
    # Model specification
    model: Optional[str] = Field(None, description="Specific model to use")
    model_version: Optional[str] = Field(None, description="Model version")
    
    # Request parameters
    temperature: Optional[float] = Field(None, ge=0.0, le=2.0, description="Sampling temperature")
    max_tokens: Optional[int] = Field(None, ge=1, description="Maximum tokens to generate")
    top_p: Optional[float] = Field(None, ge=0.0, le=1.0, description="Top-p sampling")
    top_k: Optional[int] = Field(None, ge=1, description="Top-k sampling")
    
    # Metadata
    request_id: Optional[str] = Field(None, description="Unique request identifier")
    priority: Optional[int] = Field(0, ge=0, le=10, description="Request priority (0-10)")
    timeout: Optional[float] = Field(None, ge=0.1, description="Request timeout in seconds")
    
    # Additional parameters
    params: Dict[str, Any] = Field(default_factory=dict, description="Additional parameters")
    
    class Config:
        """Pydantic configuration."""
        json_schema_extra = {
            "example": {
                "input": "What is artificial intelligence?",
                "model": "gpt-example",
                "temperature": 0.7,
                "max_tokens": 100,
                "request_id": "req_123456",
            }
        }


class ForwardResponse(BaseModel):
    """
    Forward response with AI/ML inference result.
    
    Sent from Axon (miner) to Dendrite (validator) with inference results.
    """
    
    # Output data
    output: Any = Field(..., description="Model inference output")
    
    # Model information
    model: Optional[str] = Field(None, description="Model that generated output")
    model_version: Optional[str] = Field(None, description="Model version used")
    
    # Quality metrics
    confidence: Optional[float] = Field(None, ge=0.0, le=1.0, description="Confidence score")
    tokens_used: Optional[int] = Field(None, ge=0, description="Tokens used")
    
    # Performance metrics
    processing_time: Optional[float] = Field(None, ge=0.0, description="Processing time in seconds")
    
    # Status
    success: bool = Field(True, description="Whether request was successful")
    error: Optional[str] = Field(None, description="Error message if failed")
    error_code: Optional[str] = Field(None, description="Error code if failed")
    
    # Metadata
    request_id: Optional[str] = Field(None, description="Original request identifier")
    timestamp: Optional[datetime] = Field(default_factory=datetime.now, description="Response timestamp")
    
    # Additional data
    metadata: Dict[str, Any] = Field(default_factory=dict, description="Additional metadata")
    
    class Config:
        """Pydantic configuration."""
        json_schema_extra = {
            "example": {
                "output": "Artificial intelligence is...",
                "model": "gpt-example",
                "confidence": 0.95,
                "tokens_used": 50,
                "processing_time": 0.123,
                "success": True,
                "request_id": "req_123456",
            }
        }


class BackwardRequest(BaseModel):
    """
    Backward request for gradient/feedback.
    
    Sent from Dendrite (validator) to Axon (miner) for model updates.
    """
    
    # Gradient data
    gradients: Optional[Dict[str, Any]] = Field(None, description="Gradient values")
    
    # Loss information
    loss: Optional[float] = Field(None, description="Computed loss value")
    loss_type: Optional[str] = Field(None, description="Type of loss function")
    
    # Feedback
    feedback: Optional[Dict[str, Any]] = Field(None, description="Feedback data")
    reward: Optional[float] = Field(None, description="Reward signal")
    
    # Target information
    target: Optional[Any] = Field(None, description="Target output for comparison")
    
    # Metadata
    request_id: Optional[str] = Field(None, description="Original forward request identifier")
    batch_id: Optional[str] = Field(None, description="Batch identifier")
    
    # Additional parameters
    params: Dict[str, Any] = Field(default_factory=dict, description="Additional parameters")
    
    class Config:
        """Pydantic configuration."""
        json_schema_extra = {
            "example": {
                "loss": 0.234,
                "loss_type": "cross_entropy",
                "reward": 0.85,
                "request_id": "req_123456",
            }
        }


class BackwardResponse(BaseModel):
    """
    Backward response confirming gradient/feedback receipt.
    
    Sent from Axon (miner) to Dendrite (validator) confirming update.
    """
    
    # Status
    success: bool = Field(True, description="Whether update was successful")
    applied: bool = Field(False, description="Whether gradients were applied")
    
    # Update information
    update_count: Optional[int] = Field(None, description="Number of parameters updated")
    update_magnitude: Optional[float] = Field(None, description="Magnitude of update")
    
    # Error information
    error: Optional[str] = Field(None, description="Error message if failed")
    error_code: Optional[str] = Field(None, description="Error code if failed")
    
    # Metadata
    request_id: Optional[str] = Field(None, description="Original request identifier")
    timestamp: Optional[datetime] = Field(default_factory=datetime.now, description="Response timestamp")
    
    # Additional data
    metadata: Dict[str, Any] = Field(default_factory=dict, description="Additional metadata")
    
    class Config:
        """Pydantic configuration."""
        json_schema_extra = {
            "example": {
                "success": True,
                "applied": True,
                "update_count": 1000,
                "request_id": "req_123456",
            }
        }


class PingRequest(BaseModel):
    """Ping request to check miner availability."""
    
    timestamp: datetime = Field(default_factory=datetime.now, description="Ping timestamp")
    message: Optional[str] = Field(None, description="Optional message")


class PingResponse(BaseModel):
    """Ping response from miner."""
    
    status: str = Field("ok", description="Status message")
    timestamp: datetime = Field(default_factory=datetime.now, description="Response timestamp")
    version: Optional[str] = Field(None, description="Miner version")
    load: Optional[float] = Field(None, ge=0.0, le=1.0, description="Current load (0-1)")


class StatusRequest(BaseModel):
    """Status request to get miner information."""
    
    include_metrics: bool = Field(False, description="Include performance metrics")
    include_config: bool = Field(False, description="Include configuration")


class StatusResponse(BaseModel):
    """Status response with miner information."""
    
    # Status
    status: str = Field("online", description="Miner status")
    uptime: Optional[float] = Field(None, ge=0.0, description="Uptime in seconds")
    
    # Model information
    models: List[str] = Field(default_factory=list, description="Available models")
    active_model: Optional[str] = Field(None, description="Currently active model")
    
    # Capacity
    capacity: Optional[int] = Field(None, ge=0, description="Request capacity")
    queue_size: Optional[int] = Field(None, ge=0, description="Current queue size")
    
    # Metrics (if requested)
    metrics: Optional[Dict[str, Any]] = Field(None, description="Performance metrics")
    
    # Configuration (if requested)
    config: Optional[Dict[str, Any]] = Field(None, description="Configuration")
    
    # Metadata
    timestamp: datetime = Field(default_factory=datetime.now, description="Response timestamp")
