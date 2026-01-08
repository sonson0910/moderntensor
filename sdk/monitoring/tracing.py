"""
OpenTelemetry distributed tracing for ModernTensor.

This module provides distributed tracing capabilities using OpenTelemetry,
enabling end-to-end request tracking across the ModernTensor network.
"""

import logging
import asyncio
from typing import Optional, Dict, Any, Callable
from contextlib import contextmanager
from functools import wraps
import traceback

try:
    from opentelemetry import trace
    from opentelemetry.sdk.trace import TracerProvider
    from opentelemetry.sdk.trace.export import (
        BatchSpanProcessor,
        ConsoleSpanExporter,
    )
    from opentelemetry.sdk.resources import Resource
    from opentelemetry.exporter.otlp.proto.grpc.trace_exporter import OTLPSpanExporter
    from opentelemetry.instrumentation.fastapi import FastAPIInstrumentor
    from opentelemetry.instrumentation.requests import RequestsInstrumentor
    from opentelemetry.trace import Status, StatusCode
    from opentelemetry.propagate import inject, extract
    OPENTELEMETRY_AVAILABLE = True
except ImportError:
    OPENTELEMETRY_AVAILABLE = False
    trace = None
    TracerProvider = None

logger = logging.getLogger(__name__)


class TracingConfig:
    """Configuration for OpenTelemetry tracing."""
    
    def __init__(
        self,
        service_name: str = "moderntensor",
        service_version: str = "1.0.0",
        environment: str = "production",
        otlp_endpoint: Optional[str] = None,
        console_export: bool = False,
    ):
        """
        Initialize tracing configuration.
        
        Args:
            service_name: Name of the service
            service_version: Version of the service
            environment: Environment (production, staging, development)
            otlp_endpoint: OTLP collector endpoint (e.g., "localhost:4317")
            console_export: Enable console export for debugging
        """
        self.service_name = service_name
        self.service_version = service_version
        self.environment = environment
        self.otlp_endpoint = otlp_endpoint
        self.console_export = console_export


class DistributedTracer:
    """
    Distributed tracing manager for ModernTensor.
    
    Provides OpenTelemetry integration for tracking requests across
    the network, including Axon servers, Dendrite clients, and
    blockchain operations.
    """
    
    def __init__(self, config: Optional[TracingConfig] = None):
        """
        Initialize distributed tracer.
        
        Args:
            config: Tracing configuration
        """
        self.config = config or TracingConfig()
        self.tracer_provider: Optional[TracerProvider] = None
        self.tracer: Optional[trace.Tracer] = None
        self.enabled = False
        
        if not OPENTELEMETRY_AVAILABLE:
            logger.warning(
                "OpenTelemetry not installed. Tracing disabled. "
                "Install with: pip install opentelemetry-api opentelemetry-sdk "
                "opentelemetry-exporter-otlp opentelemetry-instrumentation-fastapi "
                "opentelemetry-instrumentation-requests"
            )
            return
        
        self._setup_tracer()
    
    def _setup_tracer(self):
        """Set up OpenTelemetry tracer provider."""
        if not OPENTELEMETRY_AVAILABLE:
            return
        
        try:
            # Create resource with service information
            resource = Resource.create({
                "service.name": self.config.service_name,
                "service.version": self.config.service_version,
                "deployment.environment": self.config.environment,
            })
            
            # Create tracer provider
            self.tracer_provider = TracerProvider(resource=resource)
            
            # Add console exporter for debugging
            if self.config.console_export:
                console_processor = BatchSpanProcessor(ConsoleSpanExporter())
                self.tracer_provider.add_span_processor(console_processor)
            
            # Add OTLP exporter if endpoint provided
            if self.config.otlp_endpoint:
                otlp_exporter = OTLPSpanExporter(
                    endpoint=self.config.otlp_endpoint,
                    insecure=True,  # Use TLS in production
                )
                otlp_processor = BatchSpanProcessor(otlp_exporter)
                self.tracer_provider.add_span_processor(otlp_processor)
            
            # Set global tracer provider
            trace.set_tracer_provider(self.tracer_provider)
            
            # Get tracer
            self.tracer = trace.get_tracer(
                __name__,
                self.config.service_version,
            )
            
            self.enabled = True
            logger.info(
                f"OpenTelemetry tracing initialized for {self.config.service_name}"
            )
            
        except Exception as e:
            logger.error(f"Failed to setup tracing: {e}")
            self.enabled = False
    
    def instrument_fastapi(self, app):
        """
        Instrument FastAPI application with tracing.
        
        Args:
            app: FastAPI application instance
        """
        if not self.enabled or not OPENTELEMETRY_AVAILABLE:
            return
        
        try:
            FastAPIInstrumentor.instrument_app(app)
            logger.info("FastAPI instrumented with OpenTelemetry")
        except Exception as e:
            logger.error(f"Failed to instrument FastAPI: {e}")
    
    def instrument_requests(self):
        """Instrument requests library with tracing."""
        if not self.enabled or not OPENTELEMETRY_AVAILABLE:
            return
        
        try:
            RequestsInstrumentor().instrument()
            logger.info("Requests library instrumented with OpenTelemetry")
        except Exception as e:
            logger.error(f"Failed to instrument requests: {e}")
    
    @contextmanager
    def start_span(
        self,
        name: str,
        attributes: Optional[Dict[str, Any]] = None,
        parent_context: Optional[Any] = None,
    ):
        """
        Create a new span for tracing an operation.
        
        Args:
            name: Span name
            attributes: Additional attributes to attach
            parent_context: Parent trace context
            
        Yields:
            Span object or None if tracing disabled
        """
        if not self.enabled or not self.tracer:
            yield None
            return
        
        try:
            with self.tracer.start_as_current_span(
                name,
                context=parent_context,
                attributes=attributes or {},
            ) as span:
                yield span
        except Exception as e:
            logger.error(f"Error in span {name}: {e}")
            yield None
    
    def trace_function(
        self,
        name: Optional[str] = None,
        attributes: Optional[Dict[str, Any]] = None,
    ):
        """
        Decorator to trace a function.
        
        Args:
            name: Span name (uses function name if not provided)
            attributes: Additional attributes
            
        Returns:
            Decorated function
        """
        def decorator(func: Callable):
            @wraps(func)
            def wrapper(*args, **kwargs):
                span_name = name or f"{func.__module__}.{func.__name__}"
                
                with self.start_span(span_name, attributes) as span:
                    try:
                        result = func(*args, **kwargs)
                        if span:
                            span.set_status(Status(StatusCode.OK))
                        return result
                    except Exception as e:
                        if span:
                            span.set_status(
                                Status(StatusCode.ERROR, str(e))
                            )
                            span.record_exception(e)
                        raise
            
            @wraps(func)
            async def async_wrapper(*args, **kwargs):
                span_name = name or f"{func.__module__}.{func.__name__}"
                
                with self.start_span(span_name, attributes) as span:
                    try:
                        result = await func(*args, **kwargs)
                        if span:
                            span.set_status(Status(StatusCode.OK))
                        return result
                    except Exception as e:
                        if span:
                            span.set_status(
                                Status(StatusCode.ERROR, str(e))
                            )
                            span.record_exception(e)
                        raise
            
            # Return appropriate wrapper based on function type
            if asyncio.iscoroutinefunction(func):
                return async_wrapper
            return wrapper
        
        return decorator
    
    def add_span_event(
        self,
        name: str,
        attributes: Optional[Dict[str, Any]] = None,
    ):
        """
        Add an event to the current span.
        
        Args:
            name: Event name
            attributes: Event attributes
        """
        if not self.enabled or not OPENTELEMETRY_AVAILABLE:
            return
        
        try:
            current_span = trace.get_current_span()
            if current_span:
                current_span.add_event(name, attributes or {})
        except Exception as e:
            logger.error(f"Failed to add span event: {e}")
    
    def set_span_attribute(self, key: str, value: Any):
        """
        Set an attribute on the current span.
        
        Args:
            key: Attribute key
            value: Attribute value
        """
        if not self.enabled or not OPENTELEMETRY_AVAILABLE:
            return
        
        try:
            current_span = trace.get_current_span()
            if current_span:
                current_span.set_attribute(key, value)
        except Exception as e:
            logger.error(f"Failed to set span attribute: {e}")
    
    def record_exception(self, exception: Exception):
        """
        Record an exception in the current span.
        
        Args:
            exception: Exception to record
        """
        if not self.enabled or not OPENTELEMETRY_AVAILABLE:
            return
        
        try:
            current_span = trace.get_current_span()
            if current_span:
                current_span.record_exception(exception)
                current_span.set_status(
                    Status(StatusCode.ERROR, str(exception))
                )
        except Exception as e:
            logger.error(f"Failed to record exception: {e}")
    
    def inject_context(self, carrier: Dict):
        """
        Inject trace context into carrier for propagation.
        
        Args:
            carrier: Dictionary to inject context into (e.g., HTTP headers)
        """
        if not self.enabled or not OPENTELEMETRY_AVAILABLE:
            return
        
        try:
            inject(carrier)
        except Exception as e:
            logger.error(f"Failed to inject context: {e}")
    
    def extract_context(self, carrier: Dict):
        """
        Extract trace context from carrier.
        
        Args:
            carrier: Dictionary containing context (e.g., HTTP headers)
            
        Returns:
            Extracted context or None
        """
        if not self.enabled or not OPENTELEMETRY_AVAILABLE:
            return None
        
        try:
            return extract(carrier)
        except Exception as e:
            logger.error(f"Failed to extract context: {e}")
            return None
    
    def shutdown(self):
        """Shutdown tracer and flush all spans."""
        if self.tracer_provider:
            try:
                self.tracer_provider.shutdown()
                logger.info("Tracer provider shutdown complete")
            except Exception as e:
                logger.error(f"Error shutting down tracer: {e}")


# Convenience functions for common tracing operations

def trace_blockchain_operation(operation_name: str):
    """
    Decorator for tracing blockchain operations.
    
    Args:
        operation_name: Name of the operation
        
    Returns:
        Decorated function
    """
    def decorator(func: Callable):
        return global_tracer.trace_function(
            name=f"blockchain.{operation_name}",
            attributes={"operation.type": "blockchain"},
        )(func)
    return decorator


def trace_network_operation(operation_name: str):
    """
    Decorator for tracing network operations.
    
    Args:
        operation_name: Name of the operation
        
    Returns:
        Decorated function
    """
    def decorator(func: Callable):
        return global_tracer.trace_function(
            name=f"network.{operation_name}",
            attributes={"operation.type": "network"},
        )(func)
    return decorator


def trace_axon_request(endpoint: str):
    """
    Decorator for tracing Axon requests.
    
    Args:
        endpoint: API endpoint
        
    Returns:
        Decorated function
    """
    def decorator(func: Callable):
        return global_tracer.trace_function(
            name=f"axon.{endpoint}",
            attributes={"operation.type": "axon", "endpoint": endpoint},
        )(func)
    return decorator


def trace_dendrite_query(query_type: str):
    """
    Decorator for tracing Dendrite queries.
    
    Args:
        query_type: Type of query
        
    Returns:
        Decorated function
    """
    def decorator(func: Callable):
        return global_tracer.trace_function(
            name=f"dendrite.{query_type}",
            attributes={"operation.type": "dendrite", "query_type": query_type},
        )(func)
    return decorator


# Global tracer instance
global_tracer = DistributedTracer()


def get_tracer() -> DistributedTracer:
    """
    Get the global tracer instance.
    
    Returns:
        Global DistributedTracer instance
    """
    return global_tracer


def configure_tracing(config: TracingConfig):
    """
    Configure global tracing.
    
    Args:
        config: Tracing configuration
    """
    global global_tracer
    global_tracer = DistributedTracer(config)
