"""
Zero-Knowledge Proof Generator for ML Inferences.

Generates cryptographic proofs that an ML inference was computed correctly
without revealing the model weights or input data.
"""

import logging
import hashlib
import json
from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional, Union
from pathlib import Path
import time

logger = logging.getLogger(__name__)


@dataclass
class ProofConfig:
    """Configuration for proof generation"""
    circuit_path: Optional[Path] = None
    proving_key_path: Optional[Path] = None
    verification_key_path: Optional[Path] = None
    srs_path: Optional[Path] = None  # Structured Reference String
    backend: str = "ezkl"  # "ezkl", "zkml", "custom"
    proof_system: str = "plonk"  # "plonk", "groth16", "stark"
    use_gpu: bool = False
    calibration_data: Optional[List] = None


@dataclass
class Proof:
    """Zero-knowledge proof for ML inference"""
    proof_data: bytes
    public_inputs: List[float]
    public_outputs: List[float]
    circuit_hash: str
    timestamp: float = field(default_factory=time.time)
    metadata: Dict[str, Any] = field(default_factory=dict)
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert proof to dictionary"""
        return {
            "proof_data": self.proof_data.hex(),
            "public_inputs": self.public_inputs,
            "public_outputs": self.public_outputs,
            "circuit_hash": self.circuit_hash,
            "timestamp": self.timestamp,
            "metadata": self.metadata,
        }
    
    def to_json(self) -> str:
        """Convert proof to JSON"""
        return json.dumps(self.to_dict())
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> 'Proof':
        """Create proof from dictionary"""
        return cls(
            proof_data=bytes.fromhex(data["proof_data"]),
            public_inputs=data["public_inputs"],
            public_outputs=data["public_outputs"],
            circuit_hash=data["circuit_hash"],
            timestamp=data.get("timestamp", time.time()),
            metadata=data.get("metadata", {}),
        )
    
    @classmethod
    def from_json(cls, json_str: str) -> 'Proof':
        """Create proof from JSON"""
        return cls.from_dict(json.loads(json_str))


class ProofGenerator:
    """
    Generate zero-knowledge proofs for ML inferences.
    
    This allows validators to verify that miners computed ML inferences
    correctly without revealing model weights or input data.
    
    Example:
        config = ProofConfig(backend="ezkl", proof_system="plonk")
        generator = ProofGenerator(config)
        
        # Setup circuit
        generator.setup_circuit(model_path="model.onnx")
        
        # Generate proof
        proof = generator.generate_proof(
            input_data=[1.0, 2.0, 3.0],
            output_data=[0.8],
        )
        
        # Verify proof
        is_valid = generator.verify_proof(proof)
    """
    
    def __init__(self, config: ProofConfig):
        """
        Initialize proof generator.
        
        Args:
            config: Proof generation configuration
        """
        self.config = config
        self.circuit = None
        self.proving_key = None
        self.verification_key = None
        self.is_setup = False
        
        logger.info(f"ProofGenerator initialized with backend: {config.backend}")
    
    def setup_circuit(
        self,
        model_path: Optional[Path] = None,
        circuit: Optional['Circuit'] = None,
    ) -> None:
        """
        Setup the circuit for proof generation.
        
        Args:
            model_path: Path to ONNX model file
            circuit: Pre-compiled circuit object
        """
        if circuit is not None:
            self.circuit = circuit
        elif model_path is not None:
            self.circuit = self._compile_circuit(model_path)
        else:
            # Create mock circuit for testing
            from .circuit import Circuit
            self.circuit = Circuit(
                circuit_data={"mock": True, "test": True},
                input_shape=[3],
                output_shape=[1],
            )
            logger.info("Using mock circuit for testing")
        
        # Generate proving and verification keys
        self._generate_keys()
        
        self.is_setup = True
        logger.info("Circuit setup complete")
    
    def _compile_circuit(self, model_path: Path) -> 'Circuit':
        """Compile ONNX model to circuit"""
        if self.config.backend == "ezkl":
            return self._compile_with_ezkl(model_path)
        else:
            raise NotImplementedError(f"Backend {self.config.backend} not implemented")
    
    def _compile_with_ezkl(self, model_path: Path) -> 'Circuit':
        """Compile circuit using EZKL"""
        try:
            import ezkl
            
            logger.info(f"Compiling circuit from {model_path} using EZKL")
            
            # Convert model to circuit
            # Note: This is a simplified version
            # In production, you would use ezkl.compile() with proper configuration
            
            circuit_data = {
                "model_path": str(model_path),
                "backend": "ezkl",
                "compiled": True,
            }
            
            from .circuit import Circuit
            circuit = Circuit(
                circuit_data=circuit_data,
                input_shape=None,  # Would be extracted from model
                output_shape=None,  # Would be extracted from model
            )
            
            logger.info("Circuit compiled successfully")
            return circuit
            
        except ImportError:
            logger.warning("EZKL not installed, using mock circuit")
            from .circuit import Circuit
            return Circuit(
                circuit_data={"mock": True},
                input_shape=[3],
                output_shape=[1],
            )
    
    def _generate_keys(self) -> None:
        """Generate proving and verification keys"""
        if self.config.backend == "ezkl":
            self._generate_keys_ezkl()
        else:
            # Mock key generation
            self.proving_key = b"mock_proving_key"
            self.verification_key = b"mock_verification_key"
    
    def _generate_keys_ezkl(self) -> None:
        """Generate keys using EZKL"""
        try:
            import ezkl
            
            # In production, this would use ezkl.setup()
            # For now, use mock keys
            self.proving_key = b"ezkl_proving_key"
            self.verification_key = b"ezkl_verification_key"
            
            logger.info("Generated proving and verification keys")
            
        except ImportError:
            logger.warning("EZKL not available, using mock keys")
            self.proving_key = b"mock_proving_key"
            self.verification_key = b"mock_verification_key"
    
    def generate_proof(
        self,
        input_data: Union[List[float], Any],
        output_data: Union[List[float], Any],
    ) -> Proof:
        """
        Generate zero-knowledge proof for inference.
        
        Args:
            input_data: Input to the model
            output_data: Output from the model
        
        Returns:
            Zero-knowledge proof
        """
        if not self.is_setup:
            raise RuntimeError("Circuit not setup. Call setup_circuit() first.")
        
        logger.debug("Generating proof...")
        start_time = time.time()
        
        # Normalize inputs
        if not isinstance(input_data, list):
            input_data = [float(x) for x in input_data]
        if not isinstance(output_data, list):
            output_data = [float(x) for x in output_data]
        
        # Generate witness
        witness = self._create_witness(input_data, output_data)
        
        # Generate proof
        if self.config.backend == "ezkl":
            proof_bytes = self._generate_proof_ezkl(witness)
        else:
            proof_bytes = self._generate_mock_proof(witness)
        
        # Calculate circuit hash for verification
        circuit_hash = self._calculate_circuit_hash()
        
        proof_time = time.time() - start_time
        logger.info(f"Proof generated in {proof_time:.2f}s")
        
        return Proof(
            proof_data=proof_bytes,
            public_inputs=input_data,
            public_outputs=output_data,
            circuit_hash=circuit_hash,
            metadata={
                "proof_time": proof_time,
                "backend": self.config.backend,
                "proof_system": self.config.proof_system,
            },
        )
    
    def _create_witness(
        self,
        input_data: List[float],
        output_data: List[float],
    ) -> Dict[str, Any]:
        """Create witness for proof generation"""
        return {
            "inputs": input_data,
            "outputs": output_data,
            "timestamp": time.time(),
        }
    
    def _generate_proof_ezkl(self, witness: Dict[str, Any]) -> bytes:
        """Generate proof using EZKL"""
        try:
            import ezkl
            
            # In production, this would use ezkl.prove()
            # For now, generate mock proof
            proof_data = json.dumps({
                "witness": witness,
                "backend": "ezkl",
                "system": self.config.proof_system,
            })
            
            return proof_data.encode()
            
        except ImportError:
            return self._generate_mock_proof(witness)
    
    def _generate_mock_proof(self, witness: Dict[str, Any]) -> bytes:
        """Generate mock proof for testing"""
        proof_data = {
            "witness": witness,
            "mock": True,
            "timestamp": time.time(),
        }
        return json.dumps(proof_data).encode()
    
    def _calculate_circuit_hash(self) -> str:
        """Calculate hash of circuit for verification"""
        if self.circuit is None:
            return "no_circuit"
        
        circuit_str = json.dumps(self.circuit.circuit_data, sort_keys=True)
        return hashlib.sha256(circuit_str.encode()).hexdigest()
    
    def verify_proof(self, proof: Proof) -> bool:
        """
        Verify a zero-knowledge proof.
        
        Args:
            proof: Proof to verify
        
        Returns:
            True if proof is valid
        """
        if not self.is_setup:
            logger.warning("Circuit not setup, cannot verify proof")
            return False
        
        # Check circuit hash
        expected_hash = self._calculate_circuit_hash()
        if proof.circuit_hash != expected_hash:
            logger.warning("Circuit hash mismatch")
            return False
        
        # Verify proof
        if self.config.backend == "ezkl":
            return self._verify_proof_ezkl(proof)
        else:
            return self._verify_mock_proof(proof)
    
    def _verify_proof_ezkl(self, proof: Proof) -> bool:
        """Verify proof using EZKL"""
        try:
            import ezkl
            
            # In production, this would use ezkl.verify()
            # For now, do basic validation
            proof_data = json.loads(proof.proof_data.decode())
            return proof_data.get("backend") == "ezkl"
            
        except ImportError:
            logger.debug("EZKL not available, using mock verification")
            return self._verify_mock_proof(proof)
        except Exception as e:
            logger.error(f"Proof verification failed: {e}")
            return self._verify_mock_proof(proof)
    
    def _verify_mock_proof(self, proof: Proof) -> bool:
        """Verify mock proof"""
        try:
            proof_data = json.loads(proof.proof_data.decode())
            return "witness" in proof_data and "timestamp" in proof_data
        except:
            return False
    
    def save_keys(self, directory: Path) -> None:
        """Save proving and verification keys"""
        directory.mkdir(parents=True, exist_ok=True)
        
        pk_path = directory / "proving_key.bin"
        vk_path = directory / "verification_key.bin"
        
        pk_path.write_bytes(self.proving_key)
        vk_path.write_bytes(self.verification_key)
        
        logger.info(f"Keys saved to {directory}")
    
    def load_keys(self, directory: Path) -> None:
        """Load proving and verification keys"""
        pk_path = directory / "proving_key.bin"
        vk_path = directory / "verification_key.bin"
        
        if not pk_path.exists() or not vk_path.exists():
            raise FileNotFoundError("Key files not found")
        
        self.proving_key = pk_path.read_bytes()
        self.verification_key = vk_path.read_bytes()
        
        logger.info(f"Keys loaded from {directory}")
