"""
Luxtensor Pallet Encoding Module

This module provides encoding functions for Luxtensor blockchain pallet calls.
Luxtensor uses a custom pallet-based architecture (inspired by Substrate) for
specialized operations like staking, subnet management, and neuron registration.

Each pallet call is encoded as:
    - Function selector (4 bytes): Identifies the specific pallet function
    - Parameters (variable): Encoded function parameters

This matches the Luxtensor Rust implementation in luxtensor-contracts.
"""

import struct
from typing import Optional, List
from dataclasses import dataclass


# Function selectors for Luxtensor pallets (matching Rust implementation)
# These are derived from keccak256 hash of function signatures
# NOTE: These are placeholder values for development. In production, these should be
# replaced with actual keccak256 hashes of function signatures:
# Example: keccak256("addStake(address,uint256)")[:4]
FUNCTION_SELECTORS = {
    # Staking pallet
    'stake_add': bytes.fromhex('12345678'),       # PLACEHOLDER: addStake(address,uint256)
    'stake_remove': bytes.fromhex('87654321'),    # PLACEHOLDER: removeStake(address,uint256)
    'stake_claim': bytes.fromhex('abcdef12'),     # PLACEHOLDER: claimRewards(address)
    
    # Subnet pallet
    'subnet_create': bytes.fromhex('11223344'),   # PLACEHOLDER: createSubnet(string,uint256)
    'subnet_register': bytes.fromhex('abcd1234'), # PLACEHOLDER: registerOnSubnet(uint256,address,uint256,string)
    
    # Neuron pallet
    'neuron_register': bytes.fromhex('22334455'),  # PLACEHOLDER: registerNeuron(uint256,address)
    'neuron_deregister': bytes.fromhex('33445566'), # PLACEHOLDER: deregisterNeuron(uint256,address)
    
    # Weight pallet
    'weight_set': bytes.fromhex('44556677'),      # PLACEHOLDER: setWeights(uint256,uint256[],uint256[])
    'weight_commit': bytes.fromhex('55667788'),   # PLACEHOLDER: commitWeights(uint256,bytes32)
}


@dataclass
class EncodedCall:
    """
    Represents an encoded pallet call.
    
    Attributes:
        data: Encoded call data (function selector + parameters)
        gas_estimate: Estimated gas for this call
        description: Human-readable description
    """
    data: bytes
    gas_estimate: int
    description: str


def encode_stake_add(hotkey: str, amount: int) -> EncodedCall:
    """
    Encode an addStake call.
    
    Args:
        hotkey: Hotkey address to stake to (0x...)
        amount: Amount to stake in base units (u128)
    
    Returns:
        EncodedCall with encoded data and gas estimate
    
    Example:
        >>> call = encode_stake_add("0x1234...", 1000000000)
        >>> tx_data = call.data
    """
    # Function selector
    selector = FUNCTION_SELECTORS['stake_add']
    
    # Encode address (20 bytes)
    hotkey_bytes = bytes.fromhex(hotkey[2:] if hotkey.startswith('0x') else hotkey)
    if len(hotkey_bytes) != 20:
        raise ValueError(f"Invalid address length: {len(hotkey_bytes)}, expected 20")
    
    # Encode amount (u128, 16 bytes little endian)
    amount_bytes = struct.pack('<QQ', amount & 0xFFFFFFFFFFFFFFFF, amount >> 64)
    
    # Combine
    data = selector + hotkey_bytes + amount_bytes
    
    return EncodedCall(
        data=data,
        gas_estimate=150000,  # Estimated gas for stake operation
        description=f"Add {amount} stake to {hotkey}"
    )


def encode_stake_remove(hotkey: str, amount: int) -> EncodedCall:
    """
    Encode a removeStake call (unstake).
    
    Args:
        hotkey: Hotkey address to unstake from (0x...)
        amount: Amount to unstake in base units (u128)
    
    Returns:
        EncodedCall with encoded data and gas estimate
    """
    # Function selector
    selector = FUNCTION_SELECTORS['stake_remove']
    
    # Encode address (20 bytes)
    hotkey_bytes = bytes.fromhex(hotkey[2:] if hotkey.startswith('0x') else hotkey)
    if len(hotkey_bytes) != 20:
        raise ValueError(f"Invalid address length: {len(hotkey_bytes)}, expected 20")
    
    # Encode amount (u128, 16 bytes little endian)
    amount_bytes = struct.pack('<QQ', amount & 0xFFFFFFFFFFFFFFFF, amount >> 64)
    
    # Combine
    data = selector + hotkey_bytes + amount_bytes
    
    return EncodedCall(
        data=data,
        gas_estimate=100000,  # Estimated gas for unstake operation
        description=f"Remove {amount} stake from {hotkey}"
    )


def encode_claim_rewards(hotkey: str) -> EncodedCall:
    """
    Encode a claimRewards call.
    
    Args:
        hotkey: Hotkey address to claim rewards for (0x...)
    
    Returns:
        EncodedCall with encoded data and gas estimate
    """
    # Function selector
    selector = FUNCTION_SELECTORS['stake_claim']
    
    # Encode address (20 bytes)
    hotkey_bytes = bytes.fromhex(hotkey[2:] if hotkey.startswith('0x') else hotkey)
    if len(hotkey_bytes) != 20:
        raise ValueError(f"Invalid address length: {len(hotkey_bytes)}, expected 20")
    
    # Combine
    data = selector + hotkey_bytes
    
    return EncodedCall(
        data=data,
        gas_estimate=100000,  # Estimated gas for claim operation
        description=f"Claim rewards for {hotkey}"
    )


def encode_subnet_create(name: str, initial_emission: int) -> EncodedCall:
    """
    Encode a createSubnet call.
    
    Args:
        name: Subnet name (string)
        initial_emission: Initial emission rate (u128)
    
    Returns:
        EncodedCall with encoded data and gas estimate
    """
    # Function selector
    selector = FUNCTION_SELECTORS['subnet_create']
    
    # Encode name as UTF-8 bytes with length prefix (u32)
    name_bytes = name.encode('utf-8')
    name_length = struct.pack('<I', len(name_bytes))
    
    # Encode emission (u128, 16 bytes little endian)
    emission_bytes = struct.pack('<QQ', initial_emission & 0xFFFFFFFFFFFFFFFF, initial_emission >> 64)
    
    # Combine
    data = selector + name_length + name_bytes + emission_bytes
    
    return EncodedCall(
        data=data,
        gas_estimate=200000,  # Estimated gas for subnet creation
        description=f"Create subnet '{name}' with emission {initial_emission}"
    )


def encode_register_on_subnet(
    subnet_uid: int,
    hotkey: str,
    stake: int,
    api_endpoint: Optional[str] = None
) -> EncodedCall:
    """
    Encode a registerOnSubnet call.
    
    Args:
        subnet_uid: Subnet unique identifier (u32)
        hotkey: Hotkey address to register (0x...)
        stake: Initial stake amount in base units (u128)
        api_endpoint: Optional API endpoint for the neuron
    
    Returns:
        EncodedCall with encoded data and gas estimate
    """
    # Function selector
    selector = FUNCTION_SELECTORS['subnet_register']
    
    # Encode subnet UID (u32, 4 bytes little endian)
    subnet_bytes = struct.pack('<I', subnet_uid)
    
    # Encode hotkey address (20 bytes)
    hotkey_bytes = bytes.fromhex(hotkey[2:] if hotkey.startswith('0x') else hotkey)
    if len(hotkey_bytes) != 20:
        raise ValueError(f"Invalid address length: {len(hotkey_bytes)}, expected 20")
    
    # Encode stake (u128, 16 bytes little endian)
    stake_bytes = struct.pack('<QQ', stake & 0xFFFFFFFFFFFFFFFF, stake >> 64)
    
    # Encode API endpoint (optional string with length prefix)
    if api_endpoint:
        endpoint_bytes = api_endpoint.encode('utf-8')
        endpoint_length = struct.pack('<I', len(endpoint_bytes))
        endpoint_data = endpoint_length + endpoint_bytes
    else:
        # Empty string: length = 0
        endpoint_data = struct.pack('<I', 0)
    
    # Combine
    data = selector + subnet_bytes + hotkey_bytes + stake_bytes + endpoint_data
    
    return EncodedCall(
        data=data,
        gas_estimate=250000,  # Estimated gas for registration
        description=f"Register {hotkey} on subnet {subnet_uid} with stake {stake}"
    )


def encode_set_weights(
    subnet_uid: int,
    neuron_uids: List[int],
    weights: List[int]
) -> EncodedCall:
    """
    Encode a setWeights call for validator weight setting.
    
    Args:
        subnet_uid: Subnet unique identifier (u32)
        neuron_uids: List of neuron UIDs (u32[])
        weights: List of weight values (u32[], same length as neuron_uids)
    
    Returns:
        EncodedCall with encoded data and gas estimate
    
    Raises:
        ValueError: If neuron_uids and weights have different lengths
    """
    if len(neuron_uids) != len(weights):
        raise ValueError(
            f"Mismatch in array lengths: {len(neuron_uids)} UIDs vs {len(weights)} weights"
        )
    
    # Function selector
    selector = FUNCTION_SELECTORS['weight_set']
    
    # Encode subnet UID (u32, 4 bytes little endian)
    subnet_bytes = struct.pack('<I', subnet_uid)
    
    # Encode neuron UIDs array
    # Format: length (u32) + elements (u32 each)
    uids_length = struct.pack('<I', len(neuron_uids))
    uids_data = b''.join(struct.pack('<I', uid) for uid in neuron_uids)
    
    # Encode weights array
    # Format: length (u32) + elements (u32 each)
    weights_length = struct.pack('<I', len(weights))
    weights_data = b''.join(struct.pack('<I', w) for w in weights)
    
    # Combine
    data = (
        selector +
        subnet_bytes +
        uids_length + uids_data +
        weights_length + weights_data
    )
    
    # Gas estimate scales with number of weights
    gas_estimate = 150000 + (len(weights) * 5000)
    
    return EncodedCall(
        data=data,
        gas_estimate=gas_estimate,
        description=f"Set {len(weights)} weights on subnet {subnet_uid}"
    )


def decode_function_selector(data: bytes) -> Optional[str]:
    """
    Decode function selector from call data.
    
    Args:
        data: Encoded call data (at least 4 bytes)
    
    Returns:
        Function name if recognized, None otherwise
    """
    if len(data) < 4:
        return None
    
    selector = data[:4]
    
    # Reverse lookup in function selectors
    for name, sel in FUNCTION_SELECTORS.items():
        if sel == selector:
            return name
    
    return None


def estimate_gas_for_pallet_call(call_type: str, data_size: int = 0) -> int:
    """
    Estimate gas required for a pallet call.
    
    Args:
        call_type: Type of pallet call (e.g., 'stake_add', 'subnet_register')
        data_size: Size of additional data in bytes
    
    Returns:
        Estimated gas limit
    """
    base_gas = {
        'stake_add': 150000,
        'stake_remove': 100000,
        'stake_claim': 100000,
        'subnet_create': 200000,
        'subnet_register': 250000,
        'weight_set': 150000,
    }
    
    # Get base gas or use default
    gas = base_gas.get(call_type, 100000)
    
    # Add gas for data size (if applicable)
    gas += data_size * 68
    
    return gas
