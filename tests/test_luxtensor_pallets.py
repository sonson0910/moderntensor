"""
Unit tests for Luxtensor pallet encoding module.

Tests all pallet call encoding functions to ensure they produce
valid transaction data for the Luxtensor blockchain.
"""

import pytest
from sdk.luxtensor_pallets import (
    encode_stake_add,
    encode_stake_remove,
    encode_claim_rewards,
    encode_register_on_subnet,
    encode_set_weights,
    decode_function_selector,
    estimate_gas_for_pallet_call,
    FUNCTION_SELECTORS,
)


class TestStakingPallets:
    """Test staking pallet encoding functions."""
    
    def test_encode_stake_add(self):
        """Test encoding of stake add call."""
        hotkey = "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2"
        amount = 1000000000  # 1 MDT in base units
        
        call = encode_stake_add(hotkey, amount)
        
        # Check structure
        assert len(call.data) == 40  # 4 bytes selector + 20 bytes address + 16 bytes amount
        assert call.data[:4] == FUNCTION_SELECTORS['stake_add']
        assert call.gas_estimate == 150000
        assert "stake" in call.description.lower()
        assert hotkey.lower() in call.description.lower()
    
    def test_encode_stake_remove(self):
        """Test encoding of stake remove (unstake) call."""
        hotkey = "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2"
        amount = 500000000
        
        call = encode_stake_remove(hotkey, amount)
        
        # Check structure
        assert len(call.data) == 40
        assert call.data[:4] == FUNCTION_SELECTORS['stake_remove']
        assert call.gas_estimate == 100000
        assert "remove" in call.description.lower() or "unstake" in call.description.lower()
    
    def test_encode_claim_rewards(self):
        """Test encoding of claim rewards call."""
        hotkey = "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2"
        
        call = encode_claim_rewards(hotkey)
        
        # Check structure
        assert len(call.data) == 24  # 4 bytes selector + 20 bytes address
        assert call.data[:4] == FUNCTION_SELECTORS['stake_claim']
        assert call.gas_estimate == 100000
        assert "claim" in call.description.lower()
        assert "reward" in call.description.lower()
    
    def test_invalid_address_length(self):
        """Test that invalid address length raises error."""
        invalid_address = "0x742d35"  # Too short
        
        with pytest.raises(ValueError, match="Invalid address length"):
            encode_stake_add(invalid_address, 1000)


class TestSubnetPallets:
    """Test subnet pallet encoding functions."""
    
    def test_encode_register_on_subnet_with_endpoint(self):
        """Test registration with API endpoint."""
        subnet_uid = 1
        hotkey = "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2"
        stake = 1000000000
        endpoint = "http://localhost:8080"
        
        call = encode_register_on_subnet(subnet_uid, hotkey, stake, endpoint)
        
        # Check structure
        assert call.data[:4] == FUNCTION_SELECTORS['subnet_register']
        assert call.gas_estimate == 250000
        assert "register" in call.description.lower()
        assert str(subnet_uid) in call.description
    
    def test_encode_register_on_subnet_without_endpoint(self):
        """Test registration without API endpoint."""
        subnet_uid = 2
        hotkey = "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2"
        stake = 500000000
        
        call = encode_register_on_subnet(subnet_uid, hotkey, stake, None)
        
        # Check structure
        assert call.data[:4] == FUNCTION_SELECTORS['subnet_register']
        # Should still be valid even without endpoint
        assert len(call.data) >= 44  # selector + subnet_uid + address + stake + empty string length


class TestWeightPallets:
    """Test weight pallet encoding functions."""
    
    def test_encode_set_weights(self):
        """Test encoding of set weights call."""
        subnet_uid = 1
        neuron_uids = [0, 1, 2, 3, 4]
        weights = [100, 200, 150, 50, 300]
        
        call = encode_set_weights(subnet_uid, neuron_uids, weights)
        
        # Check structure
        assert call.data[:4] == FUNCTION_SELECTORS['weight_set']
        # Gas should scale with number of weights
        assert call.gas_estimate == 150000 + (len(weights) * 5000)
        assert "weight" in call.description.lower()
        assert str(len(weights)) in call.description
    
    def test_encode_set_weights_empty(self):
        """Test encoding with empty weight arrays."""
        subnet_uid = 1
        neuron_uids = []
        weights = []
        
        call = encode_set_weights(subnet_uid, neuron_uids, weights)
        
        # Should still produce valid call
        assert call.data[:4] == FUNCTION_SELECTORS['weight_set']
        assert call.gas_estimate == 150000  # Base gas only
    
    def test_encode_set_weights_mismatch(self):
        """Test that mismatched arrays raise error."""
        subnet_uid = 1
        neuron_uids = [0, 1, 2]
        weights = [100, 200]  # One less
        
        with pytest.raises(ValueError, match="Mismatch in array lengths"):
            encode_set_weights(subnet_uid, neuron_uids, weights)


class TestUtilityFunctions:
    """Test utility functions."""
    
    def test_decode_function_selector(self):
        """Test decoding function selector from call data."""
        # Create a stake add call
        call = encode_stake_add("0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2", 1000)
        
        # Decode selector
        function_name = decode_function_selector(call.data)
        
        assert function_name == 'stake_add'
    
    def test_decode_function_selector_unknown(self):
        """Test decoding unknown selector."""
        unknown_data = b'\xff\xff\xff\xff\x00\x00\x00\x00'
        
        function_name = decode_function_selector(unknown_data)
        
        assert function_name is None
    
    def test_decode_function_selector_too_short(self):
        """Test decoding with insufficient data."""
        short_data = b'\x12\x34'
        
        function_name = decode_function_selector(short_data)
        
        assert function_name is None
    
    def test_estimate_gas_for_pallet_call(self):
        """Test gas estimation for different call types."""
        # Test known call types
        assert estimate_gas_for_pallet_call('stake_add') == 150000
        assert estimate_gas_for_pallet_call('stake_remove') == 100000
        assert estimate_gas_for_pallet_call('subnet_register') == 250000
        
        # Test with additional data size
        gas_with_data = estimate_gas_for_pallet_call('stake_add', data_size=100)
        assert gas_with_data == 150000 + (100 * 68)
        
        # Test unknown call type (should use default)
        assert estimate_gas_for_pallet_call('unknown_call') == 100000


class TestDataEncoding:
    """Test proper data encoding formats."""
    
    def test_address_encoding(self):
        """Test that addresses are encoded as 20 bytes."""
        hotkey = "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2"
        call = encode_claim_rewards(hotkey)
        
        # Extract address from call data (skip 4-byte selector)
        address_bytes = call.data[4:24]
        
        # Verify it's the correct address
        assert len(address_bytes) == 20
        assert address_bytes.hex() == hotkey[2:].lower()
    
    def test_u128_encoding(self):
        """Test that u128 amounts are encoded correctly."""
        hotkey = "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2"
        amount = 1000000000  # 1 MDT
        
        call = encode_stake_add(hotkey, amount)
        
        # Extract amount (skip 4-byte selector + 20-byte address)
        amount_bytes = call.data[24:40]
        
        # Verify it's 16 bytes (u128)
        assert len(amount_bytes) == 16
    
    def test_u32_encoding(self):
        """Test that u32 values are encoded correctly."""
        subnet_uid = 42
        neuron_uids = [1, 2, 3]
        weights = [100, 200, 150]
        
        call = encode_set_weights(subnet_uid, neuron_uids, weights)
        
        # Extract subnet UID (skip 4-byte selector)
        subnet_bytes = call.data[4:8]
        
        # Verify it's 4 bytes (u32) and matches
        assert len(subnet_bytes) == 4
        # Decode as little endian u32
        import struct
        decoded_subnet = struct.unpack('<I', subnet_bytes)[0]
        assert decoded_subnet == subnet_uid


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
