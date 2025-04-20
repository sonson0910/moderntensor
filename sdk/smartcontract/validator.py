from pycardano import (
    PlutusV3Script,
    ScriptHash,
)
import json
import logging
import os


def read_validator(script_filename: str = "plutus.json"):
    """
    Reads the Plutus script details (script CBOR hex and script hash)
    from a JSON file located in the same directory as this script.

    Args:
        script_filename (str): The name of the JSON file containing the script details.
                                Defaults to "plutus.json".

    Returns:
        dict: A dictionary containing 'script' (PlutusV3Script object) and
              'script_hash' (ScriptHash object), or None if reading fails.
    """
    logger = logging.getLogger(__name__)
    try:
        # --- Construct path relative to this script file ---
        # Get the directory where this validator.py script resides
        script_dir = os.path.dirname(os.path.abspath(__file__))
        # Join the directory path with the script filename
        plutus_script_path = os.path.join(script_dir, script_filename)
        # --- End of change ---

        logger.debug(f"Attempting to read validator script from: {plutus_script_path}")

        if not os.path.exists(plutus_script_path):
            logger.error(f"Script file not found at path: {plutus_script_path}")
            return None

        with open(plutus_script_path) as f:
            script_details = json.load(f)

        script_bytes = PlutusV3Script(
            bytes.fromhex(script_details["validators"][0]["compiledCode"])
        )
        script_hash = ScriptHash(bytes.fromhex(script_details["validators"][0]["hash"]))
        return {
            "type": "PlutusV3",
            "script_bytes": script_bytes,
            "script_hash": script_hash,
        }
    except Exception as e:
        logger.error(f"Error reading validator script: {e}")
        return None


def read_validator_static_subnet(script_filename: str = "static_datum_subnet.json"):
    """
    Reads the Plutus script details (script CBOR hex and script hash)
    from a JSON file located in the same directory as this script.

    Args:
        script_filename (str): The name of the JSON file containing the script details.
                                Defaults to "static_datum_subnet.json".

    Returns:
        dict: A dictionary containing 'script' (PlutusV3Script object) and
              'script_hash' (ScriptHash object), or None if reading fails.
    """
    logger = logging.getLogger(__name__)
    try:
        # --- Construct path relative to this script file ---
        # Get the directory where this validator.py script resides
        script_dir = os.path.dirname(os.path.abspath(__file__))
        # Join the directory path with the script filename
        plutus_script_path = os.path.join(script_dir, script_filename)
        # --- End of change ---

        logger.debug(f"Attempting to read validator script from: {plutus_script_path}")

        if not os.path.exists(plutus_script_path):
            logger.error(f"Script file not found at path: {plutus_script_path}")
            return None

        with open(plutus_script_path) as f:
            script_details = json.load(f)

        script_bytes = PlutusV3Script(
            bytes.fromhex(script_details["validators"][0]["compiledCode"])
        )
        script_hash = ScriptHash(bytes.fromhex(script_details["validators"][0]["hash"]))
        return {
            "type": "PlutusV3",
            "script_bytes": script_bytes,
            "script_hash": script_hash,
        }
    except Exception as e:
        logger.error(f"Error reading validator script: {e}")
        return None


def read_validator_minting_tokenomics(script_filename: str = "minting_tokenomics.json"):
    """
    Reads the Plutus script details (script CBOR hex and script hash)
    from a JSON file located in the same directory as this script.

    Args:
        script_filename (str): The name of the JSON file containing the script details.
                                Defaults to "minting_tokenomics.json".

    Returns:
        dict: A dictionary containing 'script' (PlutusV3Script object) and
              'script_hash' (ScriptHash object), or None if reading fails.
    """
    logger = logging.getLogger(__name__)
    try:
        # --- Construct path relative to this script file ---
        # Get the directory where this validator.py script resides
        script_dir = os.path.dirname(os.path.abspath(__file__))
        # Join the directory path with the script filename
        plutus_script_path = os.path.join(script_dir, script_filename)
        # --- End of change ---

        logger.debug(f"Attempting to read validator script from: {plutus_script_path}")

        if not os.path.exists(plutus_script_path):
            logger.error(f"Script file not found at path: {plutus_script_path}")
            return None

        with open(plutus_script_path) as f:
            script_details = json.load(f)

        script_bytes = PlutusV3Script(
            bytes.fromhex(script_details["validators"][0]["compiledCode"])
        )
        script_hash = ScriptHash(bytes.fromhex(script_details["validators"][0]["hash"]))
        return {
            "type": "PlutusV3",
            "script_bytes": script_bytes,
            "script_hash": script_hash,
        }
    except Exception as e:
        logger.error(f"Error reading validator script: {e}")
        return None


def read_validator_dynamic_subnet(script_filename: str = "dynamic_datum_subnet.json"):
    """
    Reads the Plutus script details (script CBOR hex and script hash)
    from a JSON file located in the same directory as this script.

    Args:
        script_filename (str): The name of the JSON file containing the script details.
                                Defaults to "dynamic_datum_subnet.json".

    Returns:
        dict: A dictionary containing 'script' (PlutusV3Script object) and
              'script_hash' (ScriptHash object), or None if reading fails.
    """
    logger = logging.getLogger(__name__)
    try:
        # --- Construct path relative to this script file ---
        # Get the directory where this validator.py script resides
        script_dir = os.path.dirname(os.path.abspath(__file__))
        # Join the directory path with the script filename
        plutus_script_path = os.path.join(script_dir, script_filename)
        # --- End of change ---

        logger.debug(f"Attempting to read validator script from: {plutus_script_path}")

        if not os.path.exists(plutus_script_path):
            logger.error(f"Script file not found at path: {plutus_script_path}")
            return None

        with open(plutus_script_path) as f:
            script_details = json.load(f)

        script_bytes = PlutusV3Script(
            bytes.fromhex(script_details["validators"][0]["compiledCode"])
        )
        script_hash = ScriptHash(bytes.fromhex(script_details["validators"][0]["hash"]))
        return {
            "type": "PlutusV3",
            "script_bytes": script_bytes,
            "script_hash": script_hash,
        }
    except Exception as e:
        logger.error(f"Error reading validator script: {e}")
        return None
