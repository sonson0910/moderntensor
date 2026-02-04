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
from Crypto.Hash import keccak


def _compute_selector(signature: str) -> bytes:
    """Compute 4-byte function selector from signature using keccak256."""
    k = keccak.new(digest_bits=256)
    k.update(signature.encode('utf-8'))
    return k.digest()[:4]


# Function selectors for Luxtensor pallets (matching Rust implementation)
# Computed as first 4 bytes of keccak256 hash of function signature
FUNCTION_SELECTORS = {
    # Staking pallet
    'stake_add': _compute_selector('addStake(address,uint256)'),
    'stake_remove': _compute_selector('removeStake(address,uint256)'),
    'stake_claim': _compute_selector('claimRewards(address)'),

    # Subnet pallet
    'subnet_create': _compute_selector('createSubnet(string,uint256)'),
    'subnet_register': _compute_selector('registerOnSubnet(uint256,address,uint256,string)'),

    # Neuron pallet
    'neuron_register': _compute_selector('registerNeuron(uint256,address)'),
    'neuron_deregister': _compute_selector('deregisterNeuron(uint256,address)'),

    # Weight pallet
    'weight_set': _compute_selector('setWeights(uint256,uint256[],uint256[])'),
    'weight_commit': _compute_selector('commitWeights(uint256,bytes32)'),
    'weight_reveal': _compute_selector('revealWeights(uint256,uint256[],uint256[],bytes32)'),

    # Weight consensus pallet (updated 2026-01-29 for stake-weighted voting)
    'weight_propose': _compute_selector('proposeWeights(uint256,uint256[],uint256[])'),
    'weight_vote': _compute_selector('voteProposal(bytes32,bool,uint128)'),  # +stake_weight
    'weight_finalize': _compute_selector('finalizeProposal(bytes32)'),
}


@dataclass
class EncodedCall:
    """
    Represents an encoded pallet call.

    Attributes:
        data: Encoded call data (function selector + parameters)
        gas_estimate: Estimated gas for this call
        description: Human-readable description
        contract_address: Target contract address (optional)
    """
    data: bytes
    gas_estimate: int
    description: str
    contract_address: Optional[str] = None


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
        'weight_commit': 100000,
        'weight_reveal': 200000,
    }

    # Get base gas or use default
    gas = base_gas.get(call_type, 100000)

    # Add gas for data size (if applicable)
    gas += data_size * 68

    return gas


# =============================================================================
# Commit-Reveal Encoding Functions
# =============================================================================

# Contract address for commit-reveal operations
COMMIT_REVEAL_CONTRACT = "0x0000000000000000000000000000000000000003"


def encode_commit_weights(subnet_uid: int, commit_hash: str) -> EncodedCall:
    """
    Encode a commitWeights call for commit-reveal mechanism.

    Args:
        subnet_uid: Subnet unique identifier (u32)
        commit_hash: Hash of weights+salt (32 bytes, hex string with 0x prefix)

    Returns:
        EncodedCall with encoded data and gas estimate

    Example:
        >>> from sdk.commit_reveal import compute_commit_hash, generate_salt
        >>> weights = [(0, 500), (1, 300)]
        >>> salt = generate_salt()
        >>> commit_hash = compute_commit_hash(weights, salt)
        >>> call = encode_commit_weights(1, commit_hash)
    """
    # Function selector
    selector = FUNCTION_SELECTORS['weight_commit']

    # Encode subnet UID (u32, 4 bytes little endian)
    subnet_bytes = struct.pack('<I', subnet_uid)

    # Encode commit hash (32 bytes)
    hash_str = commit_hash[2:] if commit_hash.startswith('0x') else commit_hash
    hash_bytes = bytes.fromhex(hash_str)
    if len(hash_bytes) != 32:
        raise ValueError(f"Invalid commit hash length: {len(hash_bytes)}, expected 32")

    # Combine
    data = selector + subnet_bytes + hash_bytes

    return EncodedCall(
        data=data,
        gas_estimate=100000,
        description=f"Commit weights hash for subnet {subnet_uid}",
        contract_address=COMMIT_REVEAL_CONTRACT,
    )


def encode_reveal_weights(
    subnet_uid: int,
    weights: List[tuple],
    salt: str
) -> EncodedCall:
    """
    Encode a revealWeights call for commit-reveal mechanism.

    Args:
        subnet_uid: Subnet unique identifier (u32)
        weights: List of (uid, weight) tuples
        salt: Salt used for commit (32 bytes, hex string)

    Returns:
        EncodedCall with encoded data and gas estimate

    Example:
        >>> weights = [(0, 500), (1, 300)]
        >>> salt = "abc123..."  # 64 hex chars
        >>> call = encode_reveal_weights(1, weights, salt)
    """
    # Function selector
    selector = FUNCTION_SELECTORS['weight_reveal']

    # Encode subnet UID (u32, 4 bytes little endian)
    subnet_bytes = struct.pack('<I', subnet_uid)

    # Encode UIDs array
    uids = [w[0] for w in weights]
    weight_values = [w[1] for w in weights]

    uids_length = struct.pack('<I', len(uids))
    uids_data = b''.join(struct.pack('<I', uid) for uid in uids)

    # Encode weights array
    weights_length = struct.pack('<I', len(weight_values))
    weights_data = b''.join(struct.pack('<H', w) for w in weight_values)  # u16

    # Encode salt (32 bytes)
    salt_str = salt[2:] if salt.startswith('0x') else salt
    salt_bytes = bytes.fromhex(salt_str)
    if len(salt_bytes) != 32:
        raise ValueError(f"Invalid salt length: {len(salt_bytes)}, expected 32")

    # Combine
    data = (
        selector +
        subnet_bytes +
        uids_length + uids_data +
        weights_length + weights_data +
        salt_bytes
    )

    # Gas estimate scales with number of weights
    gas_estimate = 200000 + (len(weights) * 5000)

    return EncodedCall(
        data=data,
        gas_estimate=gas_estimate,
        description=f"Reveal {len(weights)} weights for subnet {subnet_uid}",
        contract_address=COMMIT_REVEAL_CONTRACT,
    )


# =============================================================================
# Weight Consensus Encoding Functions
# =============================================================================

# Contract address for weight consensus operations
WEIGHT_CONSENSUS_CONTRACT = "0x0000000000000000000000000000000000000004"


def encode_propose_weights(
    subnet_uid: int,
    weights: List[tuple]
) -> EncodedCall:
    """
    Encode a proposeWeights call for multi-validator consensus.

    Args:
        subnet_uid: Subnet unique identifier (u32)
        weights: List of (uid, weight) tuples

    Returns:
        EncodedCall with encoded data and gas estimate
    """
    # Function selector
    selector = FUNCTION_SELECTORS['weight_propose']

    # Encode subnet UID (u32, 4 bytes little endian)
    subnet_bytes = struct.pack('<I', subnet_uid)

    # Encode UIDs array
    uids = [w[0] for w in weights]
    weight_values = [w[1] for w in weights]

    uids_length = struct.pack('<I', len(uids))
    uids_data = b''.join(struct.pack('<I', uid) for uid in uids)

    # Encode weights array
    weights_length = struct.pack('<I', len(weight_values))
    weights_data = b''.join(struct.pack('<I', w) for w in weight_values)

    # Combine
    data = (
        selector +
        subnet_bytes +
        uids_length + uids_data +
        weights_length + weights_data
    )

    gas_estimate = 200000 + (len(weights) * 5000)

    return EncodedCall(
        data=data,
        gas_estimate=gas_estimate,
        description=f"Propose {len(weights)} weights for subnet {subnet_uid}",
        contract_address=WEIGHT_CONSENSUS_CONTRACT,
    )


def encode_vote_proposal(proposal_id: str, approve: bool, stake_weight: int = 0) -> EncodedCall:
    """
    Encode a voteProposal call with stake-weighted voting.

    Args:
        proposal_id: Proposal ID (32 bytes, hex string with 0x prefix)
        approve: True to approve, False to reject
        stake_weight: Voter's stake weight for Sybil-resistant consensus (u128)

    Returns:
        EncodedCall with encoded data and gas estimate

    Note:
        Updated 2026-01-29 to support stake-weighted voting.
        stake_weight should be the voter's current stake amount.
    """
    # Function selector
    selector = FUNCTION_SELECTORS['weight_vote']

    # Encode proposal ID (32 bytes)
    id_str = proposal_id[2:] if proposal_id.startswith('0x') else proposal_id
    id_bytes = bytes.fromhex(id_str)
    if len(id_bytes) != 32:
        raise ValueError(f"Invalid proposal ID length: {len(id_bytes)}, expected 32")

    # Encode approve (1 byte)
    approve_byte = b'\x01' if approve else b'\x00'

    # Encode stake_weight (u128, 16 bytes little endian)
    stake_bytes = struct.pack('<QQ', stake_weight & 0xFFFFFFFFFFFFFFFF, stake_weight >> 64)

    # Combine
    data = selector + id_bytes + approve_byte + stake_bytes

    return EncodedCall(
        data=data,
        gas_estimate=100000,
        description=f"Vote {'approve' if approve else 'reject'} on proposal (stake: {stake_weight})",
        contract_address=WEIGHT_CONSENSUS_CONTRACT,
    )


def encode_finalize_proposal(proposal_id: str) -> EncodedCall:
    """
    Encode a finalizeProposal call.

    Args:
        proposal_id: Proposal ID (32 bytes, hex string with 0x prefix)

    Returns:
        EncodedCall with encoded data and gas estimate
    """
    # Function selector
    selector = FUNCTION_SELECTORS['weight_finalize']

    # Encode proposal ID (32 bytes)
    id_str = proposal_id[2:] if proposal_id.startswith('0x') else proposal_id
    id_bytes = bytes.fromhex(id_str)
    if len(id_bytes) != 32:
        raise ValueError(f"Invalid proposal ID length: {len(id_bytes)}, expected 32")

    # Combine
    data = selector + id_bytes

    return EncodedCall(
        data=data,
        gas_estimate=150000,
        description="Finalize proposal and apply weights",
        contract_address=WEIGHT_CONSENSUS_CONTRACT,
    )


