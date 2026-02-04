"""
Structured logging for ModernTensor.

This module provides structured logging with JSON formatting,
compatible with ELK stack (Elasticsearch, Logstash, Kibana) and Loki.
"""

import logging
import json
import sys
from datetime import datetime
from typing import Optional, Dict, Any
import socket


class JSONFormatter(logging.Formatter):
    """
    JSON formatter for structured logging.
    
    Formats log records as JSON documents compatible with
    Elasticsearch, Logstash, and Loki.
    """
    
    def __init__(
        self,
        service_name: str = "moderntensor",
        environment: str = "production",
        include_extra: bool = True,
    ):
        """
        Initialize JSON formatter.
        
        Args:
            service_name: Name of the service
            environment: Environment (production, staging, development)
            include_extra: Include extra fields from LogRecord
        """
        super().__init__()
        self.service_name = service_name
        self.environment = environment
        self.include_extra = include_extra
        self.hostname = socket.gethostname()
    
    def format(self, record: logging.LogRecord) -> str:
        """
        Format log record as JSON.
        
        Args:
            record: Log record to format
            
        Returns:
            JSON formatted log string
        """
        # Base log data
        log_data = {
            "@timestamp": datetime.utcnow().strftime('%Y-%m-%dT%H:%M:%S.%fZ'),
            "level": record.levelname,
            "logger": record.name,
            "message": record.getMessage(),
            "service": {
                "name": self.service_name,
                "environment": self.environment,
            },
            "host": {
                "hostname": self.hostname,
            },
            "process": {
                "pid": record.process,
                "name": record.processName,
            },
            "thread": {
                "id": record.thread,
                "name": record.threadName,
            },
            "file": {
                "path": record.pathname,
                "name": record.filename,
                "line": record.lineno,
            },
            "function": record.funcName,
        }
        
        # Add exception info if present
        if record.exc_info:
            log_data["error"] = {
                "type": record.exc_info[0].__name__,
                "message": str(record.exc_info[1]),
                "stack_trace": self.formatException(record.exc_info),
            }
        
        # Add extra fields
        if self.include_extra and hasattr(record, "__dict__"):
            # Standard fields to exclude
            exclude_fields = {
                'name', 'msg', 'args', 'created', 'filename', 'funcName',
                'levelname', 'levelno', 'lineno', 'module', 'msecs',
                'message', 'pathname', 'process', 'processName',
                'relativeCreated', 'thread', 'threadName', 'exc_info',
                'exc_text', 'stack_info',
            }
            
            extra = {}
            for key, value in record.__dict__.items():
                if key not in exclude_fields and not key.startswith('_'):
                    try:
                        # Ensure value is JSON serializable
                        json.dumps(value)
                        extra[key] = value
                    except (TypeError, ValueError):
                        extra[key] = str(value)
            
            if extra:
                log_data["extra"] = extra
        
        return json.dumps(log_data)


class StructuredLogger:
    """
    Structured logging manager for ModernTensor.
    
    Provides structured logging with JSON formatting and
    context management for enhanced observability.
    """
    
    def __init__(
        self,
        name: str,
        service_name: str = "moderntensor",
        environment: str = "production",
        level: int = logging.INFO,
        log_file: Optional[str] = None,
        json_format: bool = True,
    ):
        """
        Initialize structured logger.
        
        Args:
            name: Logger name
            service_name: Service name
            environment: Environment
            level: Logging level
            log_file: Optional file path for logging
            json_format: Use JSON formatting
        """
        self.name = name
        self.service_name = service_name
        self.environment = environment
        self.json_format = json_format
        self.context: Dict[str, Any] = {}
        
        # Get or create logger
        self.logger = logging.getLogger(name)
        self.logger.setLevel(level)
        
        # Remove existing handlers
        self.logger.handlers.clear()
        
        # Console handler
        console_handler = logging.StreamHandler(sys.stdout)
        console_handler.setLevel(level)
        
        if json_format:
            formatter = JSONFormatter(
                service_name=service_name,
                environment=environment,
            )
        else:
            formatter = logging.Formatter(
                '%(asctime)s - %(name)s - %(levelname)s - %(message)s'
            )
        
        console_handler.setFormatter(formatter)
        self.logger.addHandler(console_handler)
        
        # File handler if specified
        if log_file:
            file_handler = logging.FileHandler(log_file)
            file_handler.setLevel(level)
            file_handler.setFormatter(formatter)
            self.logger.addHandler(file_handler)
    
    def add_context(self, **kwargs):
        """
        Add context fields to all subsequent log messages.
        
        Args:
            **kwargs: Context key-value pairs
        """
        self.context.update(kwargs)
    
    def remove_context(self, *keys):
        """
        Remove context fields.
        
        Args:
            *keys: Context keys to remove
        """
        for key in keys:
            self.context.pop(key, None)
    
    def clear_context(self):
        """Clear all context fields."""
        self.context.clear()
    
    def _log_with_context(
        self,
        level: int,
        message: str,
        exc_info: Optional[Exception] = None,
        **kwargs
    ):
        """
        Log message with context.
        
        Args:
            level: Log level
            message: Log message
            exc_info: Exception info
            **kwargs: Additional fields
        """
        # Merge context with additional fields
        extra = {**self.context, **kwargs}
        
        self.logger.log(
            level,
            message,
            exc_info=exc_info,
            extra=extra,
        )
    
    def debug(self, message: str, **kwargs):
        """Log debug message."""
        self._log_with_context(logging.DEBUG, message, **kwargs)
    
    def info(self, message: str, **kwargs):
        """Log info message."""
        self._log_with_context(logging.INFO, message, **kwargs)
    
    def warning(self, message: str, **kwargs):
        """Log warning message."""
        self._log_with_context(logging.WARNING, message, **kwargs)
    
    def error(self, message: str, exc_info: Optional[Exception] = None, **kwargs):
        """Log error message."""
        self._log_with_context(logging.ERROR, message, exc_info=exc_info, **kwargs)
    
    def critical(self, message: str, exc_info: Optional[Exception] = None, **kwargs):
        """Log critical message."""
        self._log_with_context(logging.CRITICAL, message, exc_info=exc_info, **kwargs)
    
    def exception(self, message: str, **kwargs):
        """Log exception with stack trace."""
        self._log_with_context(
            logging.ERROR,
            message,
            exc_info=sys.exc_info(),
            **kwargs
        )
    
    # Convenience methods for common operations
    
    def log_blockchain_event(
        self,
        event_type: str,
        block_number: Optional[int] = None,
        transaction_hash: Optional[str] = None,
        **kwargs
    ):
        """
        Log blockchain event.
        
        Args:
            event_type: Type of event
            block_number: Block number
            transaction_hash: Transaction hash
            **kwargs: Additional fields
        """
        self.info(
            f"Blockchain event: {event_type}",
            event_type=event_type,
            block_number=block_number,
            transaction_hash=transaction_hash,
            category="blockchain",
            **kwargs
        )
    
    def log_network_event(
        self,
        event_type: str,
        peer_id: Optional[str] = None,
        message_type: Optional[str] = None,
        **kwargs
    ):
        """
        Log network event.
        
        Args:
            event_type: Type of event
            peer_id: Peer identifier
            message_type: Type of message
            **kwargs: Additional fields
        """
        self.info(
            f"Network event: {event_type}",
            event_type=event_type,
            peer_id=peer_id,
            message_type=message_type,
            category="network",
            **kwargs
        )
    
    def log_security_event(
        self,
        event_type: str,
        severity: str,
        ip_address: Optional[str] = None,
        **kwargs
    ):
        """
        Log security event.
        
        Args:
            event_type: Type of event
            severity: Severity level
            ip_address: IP address
            **kwargs: Additional fields
        """
        log_method = {
            "critical": self.critical,
            "high": self.error,
            "medium": self.warning,
            "low": self.info,
        }.get(severity.lower(), self.warning)
        
        log_method(
            f"Security event: {event_type}",
            event_type=event_type,
            severity=severity,
            ip_address=ip_address,
            category="security",
            **kwargs
        )
    
    def log_performance_metric(
        self,
        metric_name: str,
        value: float,
        unit: str = "ms",
        **kwargs
    ):
        """
        Log performance metric.
        
        Args:
            metric_name: Name of metric
            value: Metric value
            unit: Unit of measurement
            **kwargs: Additional fields
        """
        self.info(
            f"Performance metric: {metric_name}={value}{unit}",
            metric_name=metric_name,
            metric_value=value,
            metric_unit=unit,
            category="performance",
            **kwargs
        )
    
    def log_api_request(
        self,
        method: str,
        path: str,
        status_code: int,
        duration_ms: float,
        ip_address: Optional[str] = None,
        user_agent: Optional[str] = None,
        **kwargs
    ):
        """
        Log API request.
        
        Args:
            method: HTTP method
            path: Request path
            status_code: Response status code
            duration_ms: Request duration in milliseconds
            ip_address: Client IP
            user_agent: User agent
            **kwargs: Additional fields
        """
        level = logging.INFO if status_code < 400 else logging.WARNING
        
        self._log_with_context(
            level,
            f"{method} {path} {status_code} {duration_ms:.2f}ms",
            method=method,
            path=path,
            status_code=status_code,
            duration_ms=duration_ms,
            ip_address=ip_address,
            user_agent=user_agent,
            category="api",
            **kwargs
        )


class LoggerFactory:
    """Factory for creating structured loggers."""
    
    _loggers: Dict[str, StructuredLogger] = {}
    _default_config = {
        "service_name": "moderntensor",
        "environment": "production",
        "level": logging.INFO,
        "json_format": True,
    }
    
    @classmethod
    def configure(cls, **kwargs):
        """
        Configure default logger settings.
        
        Args:
            **kwargs: Configuration parameters
        """
        cls._default_config.update(kwargs)
    
    @classmethod
    def get_logger(
        cls,
        name: str,
        **kwargs
    ) -> StructuredLogger:
        """
        Get or create a structured logger.
        
        Args:
            name: Logger name
            **kwargs: Logger configuration (overrides defaults)
            
        Returns:
            StructuredLogger instance
        """
        if name not in cls._loggers:
            config = {**cls._default_config, **kwargs}
            cls._loggers[name] = StructuredLogger(name, **config)
        
        return cls._loggers[name]
    
    @classmethod
    def clear_loggers(cls):
        """Clear all cached loggers."""
        cls._loggers.clear()


# Convenience function
def get_logger(name: str, **kwargs) -> StructuredLogger:
    """
    Get a structured logger.
    
    Args:
        name: Logger name
        **kwargs: Logger configuration
        
    Returns:
        StructuredLogger instance
    """
    return LoggerFactory.get_logger(name, **kwargs)
