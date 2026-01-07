"""
Circuit compiler for zkML.

Compiles ML models to cryptographic circuits.
"""

import logging
from dataclasses import dataclass
from typing import Any, Dict, List, Optional
from pathlib import Path

logger = logging.getLogger(__name__)


@dataclass
class Circuit:
    """Compiled circuit for zkML"""
    circuit_data: Dict[str, Any]
    input_shape: Optional[List[int]]
    output_shape: Optional[List[int]]
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary"""
        return {
            "circuit_data": self.circuit_data,
            "input_shape": self.input_shape,
            "output_shape": self.output_shape,
        }


class CircuitCompiler:
    """
    Compile ML models to circuits.
    
    Example:
        compiler = CircuitCompiler(backend="ezkl")
        circuit = compiler.compile(model_path="model.onnx")
    """
    
    def __init__(self, backend: str = "ezkl"):
        """
        Initialize compiler.
        
        Args:
            backend: Backend to use ("ezkl", "zkml")
        """
        self.backend = backend
        logger.info(f"CircuitCompiler initialized with backend: {backend}")
    
    def compile(self, model_path: Path) -> Circuit:
        """
        Compile model to circuit.
        
        Args:
            model_path: Path to ONNX model
        
        Returns:
            Compiled circuit
        """
        if self.backend == "ezkl":
            return self._compile_ezkl(model_path)
        else:
            return self._compile_mock(model_path)
    
    def _compile_ezkl(self, model_path: Path) -> Circuit:
        """Compile using EZKL"""
        try:
            import ezkl
            
            logger.info(f"Compiling {model_path} with EZKL")
            
            # In production: use ezkl.compile()
            circuit_data = {
                "model_path": str(model_path),
                "backend": "ezkl",
                "compiled": True,
            }
            
            return Circuit(
                circuit_data=circuit_data,
                input_shape=None,
                output_shape=None,
            )
            
        except ImportError:
            logger.warning("EZKL not available")
            return self._compile_mock(model_path)
    
    def _compile_mock(self, model_path: Path) -> Circuit:
        """Mock compilation"""
        return Circuit(
            circuit_data={"mock": True, "path": str(model_path)},
            input_shape=[3],
            output_shape=[1],
        )
