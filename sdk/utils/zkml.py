import logging
import json
import os
from typing import Optional, Dict, Any

# Try importing ezkl, handle if not installed
try:
    import ezkl

    HAS_EZKL = True
except ImportError:
    HAS_EZKL = False

logger = logging.getLogger(__name__)


class ZkmlManager:
    """
    Utility class for Zero-Knowledge Machine Learning (zkML) operations.
    Uses 'ezkl' library to generate and verify proofs for AI model execution.
    """

    def __init__(self, model_path: str = None, settings_path: str = None):
        self.model_path = model_path
        self.settings_path = settings_path

        if not HAS_EZKL:
            logger.warning(
                "ezkl library not found. zkML features will be disabled or mocked."
            )

    def generate_proof(
        self,
        input_data: Any,
        witness_path: str = "witness.json",
        proof_path: str = "proof.json",
    ) -> Optional[str]:
        """
        Generates a zk-SNARK proof for the model execution with the given input.

        Args:
            input_data: Path to the input data file (JSON) OR a dictionary of data.
            witness_path: Path to save the witness file.
            proof_path: Path to save the proof file.

        Returns:
            str: The generated proof (hex string or path), or None if failed.
        """
        if not HAS_EZKL:
            logger.warning("Skipping proof generation (ezkl not installed).")
            return "mock_proof_hex_string"

        temp_input_path = None
        try:
            # Handle input data (path vs dict)
            if isinstance(input_data, dict):
                temp_input_path = "temp_input.json"
                with open(temp_input_path, "w") as f:
                    json.dump(input_data, f)
                input_path = temp_input_path
            else:
                input_path = input_data

            # 1. Generate Witness
            logger.info(f"Generating witness for input: {input_path}")
            ezkl.gen_witness(input_path, self.model_path, witness_path)

            # 2. Generate Proof
            logger.info("Generating zk-SNARK proof...")
            # Assuming setup (pk, vk) is already done and paths are known/configured
            # For simplicity, we use default names or passed args
            pk_path = "pk.key"

            if not os.path.exists(pk_path):
                logger.error(f"Proving key {pk_path} not found.")
                return None

            ezkl.prove(
                witness_path,
                self.model_path,
                pk_path,
                proof_path,
                "kzg",  # Proof system
            )

            with open(proof_path, "r") as f:
                proof_content = json.load(f)

            # Return proof as a serialized string (e.g., hex of the proof bytes)
            return json.dumps(proof_content)

        except Exception as e:
            logger.error(f"Failed to generate zkML proof: {e}")
            return None
        finally:
            if temp_input_path and os.path.exists(temp_input_path):
                os.remove(temp_input_path)

    def verify_proof(self, proof_content: str, vk_path: str = "vk.key") -> bool:
        """
        Verifies a zk-SNARK proof.

        Args:
            proof_content: The proof string (JSON/Hex) received from Miner.
            vk_path: Path to the verification key.

        Returns:
            bool: True if valid, False otherwise.
        """
        if not HAS_EZKL:
            logger.warning(
                "Skipping proof verification (ezkl not installed). Returning True for dev."
            )
            return True

        try:
            # Save proof to temp file for ezkl to read
            temp_proof_path = "temp_verify_proof.json"
            with open(temp_proof_path, "w") as f:
                f.write(proof_content)

            logger.info("Verifying zk-SNARK proof...")
            res = ezkl.verify(
                temp_proof_path,
                self.settings_path,
                vk_path,
                self.settings_path,  # srs path often same or related
            )

            return res
        except Exception as e:
            logger.error(f"zkML proof verification failed: {e}")
            return False
