"""
Core Synapse protocol implementation.

Defines the base Synapse class for protocol messages.
"""

from typing import Any, Dict, Optional, TypeVar
from pydantic import BaseModel, Field
from datetime import datetime
import uuid

from .version import CURRENT_VERSION


T = TypeVar('T', bound=BaseModel)


class SynapseRequest(BaseModel):
    """
    Base class for all Synapse requests.
    
    Wraps request payload with protocol metadata.
    """
    
    # Protocol metadata
    protocol_version: str = Field(
        default=CURRENT_VERSION,
        description="Protocol version"
    )
    
    # Request identification
    request_id: str = Field(
        default_factory=lambda: str(uuid.uuid4()),
        description="Unique request identifier"
    )
    
    # Sender information
    sender_uid: Optional[str] = Field(None, description="Sender unique identifier")
    sender_address: Optional[str] = Field(None, description="Sender network address")
    
    # Receiver information
    receiver_uid: Optional[str] = Field(None, description="Receiver unique identifier")
    receiver_address: Optional[str] = Field(None, description="Receiver network address")
    
    # Message type and payload
    message_type: str = Field(..., description="Type of message (forward, backward, ping, status)")
    payload: Dict[str, Any] = Field(..., description="Message payload")
    
    # Timing
    timestamp: datetime = Field(default_factory=datetime.now, description="Request timestamp")
    timeout: Optional[float] = Field(None, ge=0.1, description="Request timeout in seconds")
    
    # Priority and routing
    priority: int = Field(0, ge=0, le=10, description="Request priority (0-10)")
    route: Optional[str] = Field(None, description="Routing path")
    
    # Authentication
    signature: Optional[str] = Field(None, description="Request signature for authentication")
    nonce: Optional[str] = Field(None, description="Nonce for replay protection")
    
    # Metadata
    metadata: Dict[str, Any] = Field(default_factory=dict, description="Additional metadata")
    
    class Config:
        """Pydantic configuration."""
        json_schema_extra = {
            "example": {
                "protocol_version": "1.0",
                "request_id": "req_123456",
                "sender_uid": "validator_001",
                "receiver_uid": "miner_001",
                "message_type": "forward",
                "payload": {"input": "test"},
                "priority": 5,
            }
        }


class SynapseResponse(BaseModel):
    """
    Base class for all Synapse responses.
    
    Wraps response payload with protocol metadata.
    """
    
    # Protocol metadata
    protocol_version: str = Field(
        default=CURRENT_VERSION,
        description="Protocol version"
    )
    
    # Response identification
    response_id: str = Field(
        default_factory=lambda: str(uuid.uuid4()),
        description="Unique response identifier"
    )
    request_id: Optional[str] = Field(None, description="Original request identifier")
    
    # Sender information (responder)
    sender_uid: Optional[str] = Field(None, description="Sender unique identifier")
    sender_address: Optional[str] = Field(None, description="Sender network address")
    
    # Message type and payload
    message_type: str = Field(..., description="Type of message (forward, backward, ping, status)")
    payload: Dict[str, Any] = Field(..., description="Response payload")
    
    # Status
    success: bool = Field(True, description="Whether request was successful")
    status_code: int = Field(200, description="HTTP-style status code")
    status_message: Optional[str] = Field(None, description="Status message")
    
    # Error information
    error: Optional[str] = Field(None, description="Error message if failed")
    error_code: Optional[str] = Field(None, description="Error code if failed")
    error_details: Optional[Dict[str, Any]] = Field(None, description="Detailed error information")
    
    # Timing
    timestamp: datetime = Field(default_factory=datetime.now, description="Response timestamp")
    processing_time: Optional[float] = Field(None, ge=0.0, description="Processing time in seconds")
    
    # Authentication
    signature: Optional[str] = Field(None, description="Response signature")
    
    # Metadata
    metadata: Dict[str, Any] = Field(default_factory=dict, description="Additional metadata")
    
    class Config:
        """Pydantic configuration."""
        json_schema_extra = {
            "example": {
                "protocol_version": "1.0",
                "response_id": "resp_123456",
                "request_id": "req_123456",
                "sender_uid": "miner_001",
                "message_type": "forward",
                "payload": {"output": "result"},
                "success": True,
                "status_code": 200,
            }
        }


class Synapse:
    """
    Synapse protocol manager.
    
    Provides utilities for creating and validating protocol messages.
    """
    
    @staticmethod
    def create_request(
        message_type: str,
        payload: Dict[str, Any],
        sender_uid: Optional[str] = None,
        receiver_uid: Optional[str] = None,
        priority: int = 0,
        timeout: Optional[float] = None,
        **kwargs
    ) -> SynapseRequest:
        """
        Create a Synapse request.
        
        Args:
            message_type: Type of message (forward, backward, ping, status)
            payload: Message payload
            sender_uid: Sender unique identifier
            receiver_uid: Receiver unique identifier
            priority: Request priority (0-10)
            timeout: Request timeout in seconds
            **kwargs: Additional request fields
            
        Returns:
            SynapseRequest instance
        """
        return SynapseRequest(
            message_type=message_type,
            payload=payload,
            sender_uid=sender_uid,
            receiver_uid=receiver_uid,
            priority=priority,
            timeout=timeout,
            **kwargs
        )
    
    @staticmethod
    def create_response(
        message_type: str,
        payload: Dict[str, Any],
        request_id: Optional[str] = None,
        sender_uid: Optional[str] = None,
        success: bool = True,
        status_code: int = 200,
        error: Optional[str] = None,
        **kwargs
    ) -> SynapseResponse:
        """
        Create a Synapse response.
        
        Args:
            message_type: Type of message (forward, backward, ping, status)
            payload: Response payload
            request_id: Original request identifier
            sender_uid: Sender unique identifier
            success: Whether request was successful
            status_code: HTTP-style status code
            error: Error message if failed
            **kwargs: Additional response fields
            
        Returns:
            SynapseResponse instance
        """
        return SynapseResponse(
            message_type=message_type,
            payload=payload,
            request_id=request_id,
            sender_uid=sender_uid,
            success=success,
            status_code=status_code,
            error=error,
            **kwargs
        )
    
    @staticmethod
    def validate_request(request: SynapseRequest) -> bool:
        """
        Validate a Synapse request.
        
        Args:
            request: SynapseRequest instance
            
        Returns:
            True if valid
            
        Raises:
            ValueError: If request is invalid
        """
        if not request.message_type:
            raise ValueError("Message type is required")
        
        if not request.payload:
            raise ValueError("Payload is required")
        
        if request.priority < 0 or request.priority > 10:
            raise ValueError("Priority must be between 0 and 10")
        
        if request.timeout is not None and request.timeout < 0.1:
            raise ValueError("Timeout must be at least 0.1 seconds")
        
        return True
    
    @staticmethod
    def validate_response(response: SynapseResponse) -> bool:
        """
        Validate a Synapse response.
        
        Args:
            response: SynapseResponse instance
            
        Returns:
            True if valid
            
        Raises:
            ValueError: If response is invalid
        """
        if not response.message_type:
            raise ValueError("Message type is required")
        
        if not response.payload:
            raise ValueError("Payload is required")
        
        if response.status_code < 100 or response.status_code >= 600:
            raise ValueError("Status code must be between 100 and 599")
        
        if not response.success and not response.error:
            raise ValueError("Error message required when success is False")
        
        return True
