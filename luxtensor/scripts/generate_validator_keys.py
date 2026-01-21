#!/usr/bin/env python3
"""
Validator Key Generator for Luxtensor
Generates 32-byte secret keys for validators
"""

import os
import hashlib
import secrets

def generate_validator_key(validator_id: int) -> tuple:
    """Generate a deterministic validator key for testing"""
    # For production, use: secrets.token_bytes(32)
    # For testing, use deterministic keys
    seed = f"luxtensor-validator-{validator_id}-key".encode()
    secret = hashlib.sha256(seed).digest()

    return secret

def derive_address(secret: bytes) -> str:
    """Derive address from secret (simplified - in production use secp256k1)"""
    # This is simplified - real implementation uses elliptic curve
    public_hash = hashlib.sha256(secret).digest()
    address = public_hash[-20:]
    return "0x" + address.hex()

def main():
    print("=" * 60)
    print("Luxtensor Validator Key Generator")
    print("=" * 60 + "\n")

    for i in range(1, 4):
        key_dir = f"node{i}"
        key_path = os.path.join(key_dir, "validator.key")

        # Create directory if needed
        os.makedirs(key_dir, exist_ok=True)

        # Generate key
        secret = generate_validator_key(i)
        address = derive_address(secret)

        # Save key file
        with open(key_path, "wb") as f:
            f.write(secret)

        print(f"Validator {i}:")
        print(f"  Key file: {key_path}")
        print(f"  Address:  {address}")
        print(f"  Key hex:  0x{secret.hex()}")
        print()

    print("=" * 60)
    print("Update config.toml for each node:")
    print("  validator_key_path = \"./validator.key\"")
    print("=" * 60)

if __name__ == "__main__":
    main()
