from pycardano import (
    PlutusV3Script,
    ScriptHash,
)
import json

def read_validator() -> dict:
    with open("/Users/sonson/Documents/code/moderntensor/sdk/smartcontract/plutus.json", "r") as f:
        validator = json.load(f)
    script_bytes = PlutusV3Script(
        bytes.fromhex(validator["validators"][0]["compiledCode"])
    )
    script_hash = ScriptHash(bytes.fromhex(validator["validators"][0]["hash"]))
    return {
        "type": "PlutusV3",
        "script_bytes": script_bytes,
        "script_hash": script_hash,
    }