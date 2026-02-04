"""
Synapse serializer - JSON serialization/deserialization.

Handles encoding and decoding of Synapse messages.
"""

from typing import Any, Dict, Type, TypeVar, Union
import json
from pydantic import BaseModel

from .synapse import SynapseRequest, SynapseResponse
from .types import (
    ForwardRequest,
    ForwardResponse,
    BackwardRequest,
    BackwardResponse,
    PingRequest,
    PingResponse,
    StatusRequest,
    StatusResponse,
)


T = TypeVar('T', bound=BaseModel)


class SynapseSerializer:
    """
    Serializer for Synapse protocol messages.
    
    Handles JSON serialization/deserialization with type validation.
    """
    
    # Message type registry
    REQUEST_TYPES = {
        "forward": ForwardRequest,
        "backward": BackwardRequest,
        "ping": PingRequest,
        "status": StatusRequest,
    }
    
    RESPONSE_TYPES = {
        "forward": ForwardResponse,
        "backward": BackwardResponse,
        "ping": PingResponse,
        "status": StatusResponse,
    }
    
    @staticmethod
    def serialize_request(request: SynapseRequest) -> str:
        """
        Serialize a SynapseRequest to JSON string.
        
        Args:
            request: SynapseRequest instance
            
        Returns:
            JSON string
        """
        return request.model_dump_json(exclude_none=True)
    
    @staticmethod
    def deserialize_request(data: Union[str, bytes, Dict]) -> SynapseRequest:
        """
        Deserialize JSON to SynapseRequest.
        
        Args:
            data: JSON string, bytes, or dictionary
            
        Returns:
            SynapseRequest instance
        """
        if isinstance(data, (str, bytes)):
            request_dict = json.loads(data)
        else:
            request_dict = data
        
        return SynapseRequest(**request_dict)
    
    @staticmethod
    def serialize_response(response: SynapseResponse) -> str:
        """
        Serialize a SynapseResponse to JSON string.
        
        Args:
            response: SynapseResponse instance
            
        Returns:
            JSON string
        """
        return response.model_dump_json(exclude_none=True)
    
    @staticmethod
    def deserialize_response(data: Union[str, bytes, Dict]) -> SynapseResponse:
        """
        Deserialize JSON to SynapseResponse.
        
        Args:
            data: JSON string, bytes, or dictionary
            
        Returns:
            SynapseResponse instance
        """
        if isinstance(data, (str, bytes)):
            response_dict = json.loads(data)
        else:
            response_dict = data
        
        return SynapseResponse(**response_dict)
    
    @classmethod
    def serialize_typed_request(cls, message_type: str, request: BaseModel) -> str:
        """
        Serialize a typed request (e.g., ForwardRequest) to JSON.
        
        Args:
            message_type: Type of message
            request: Typed request instance
            
        Returns:
            JSON string
        """
        return request.model_dump_json(exclude_none=True)
    
    @classmethod
    def deserialize_typed_request(
        cls,
        message_type: str,
        data: Union[str, bytes, Dict]
    ) -> BaseModel:
        """
        Deserialize JSON to typed request.
        
        Args:
            message_type: Type of message
            data: JSON string, bytes, or dictionary
            
        Returns:
            Typed request instance
            
        Raises:
            ValueError: If message type is unknown
        """
        if message_type not in cls.REQUEST_TYPES:
            raise ValueError(f"Unknown message type: {message_type}")
        
        request_class = cls.REQUEST_TYPES[message_type]
        
        if isinstance(data, (str, bytes)):
            return request_class.model_validate_json(data)
        else:
            return request_class(**data)
    
    @classmethod
    def serialize_typed_response(cls, message_type: str, response: BaseModel) -> str:
        """
        Serialize a typed response (e.g., ForwardResponse) to JSON.
        
        Args:
            message_type: Type of message
            response: Typed response instance
            
        Returns:
            JSON string
        """
        return response.model_dump_json(exclude_none=True)
    
    @classmethod
    def deserialize_typed_response(
        cls,
        message_type: str,
        data: Union[str, bytes, Dict]
    ) -> BaseModel:
        """
        Deserialize JSON to typed response.
        
        Args:
            message_type: Type of message
            data: JSON string, bytes, or dictionary
            
        Returns:
            Typed response instance
            
        Raises:
            ValueError: If message type is unknown
        """
        if message_type not in cls.RESPONSE_TYPES:
            raise ValueError(f"Unknown message type: {message_type}")
        
        response_class = cls.RESPONSE_TYPES[message_type]
        
        if isinstance(data, (str, bytes)):
            return response_class.model_validate_json(data)
        else:
            return response_class(**data)
    
    @staticmethod
    def to_dict(obj: BaseModel) -> Dict[str, Any]:
        """
        Convert Pydantic model to dictionary.
        
        Args:
            obj: Pydantic model instance
            
        Returns:
            Dictionary representation
        """
        return obj.model_dump(exclude_none=True)
    
    @staticmethod
    def from_dict(data: Dict[str, Any], model_class: Type[T]) -> T:
        """
        Create Pydantic model from dictionary.
        
        Args:
            data: Dictionary data
            model_class: Pydantic model class
            
        Returns:
            Model instance
        """
        return model_class(**data)
    
    @staticmethod
    def register_request_type(message_type: str, request_class: Type[BaseModel]):
        """
        Register a custom request type.
        
        Args:
            message_type: Message type identifier
            request_class: Request class
        """
        SynapseSerializer.REQUEST_TYPES[message_type] = request_class
    
    @staticmethod
    def register_response_type(message_type: str, response_class: Type[BaseModel]):
        """
        Register a custom response type.
        
        Args:
            message_type: Message type identifier
            response_class: Response class
        """
        SynapseSerializer.RESPONSE_TYPES[message_type] = response_class
