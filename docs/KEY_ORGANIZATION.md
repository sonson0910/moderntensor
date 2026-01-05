# Key Organization in ModernTensor Layer 1

## Overview

ModernTensor Layer 1 uses **BIP44** hierarchical deterministic (HD) wallet structure for key derivation. This provides a standardized, secure way to generate multiple keys from a single seed phrase.

## BIP44 Path Structure

```
m / purpose' / coin_type' / account' / change / address_index
```

### Path Components

1. **m**: Master key (root)
2. **purpose'**: 44' (BIP44 standard) - hardened
3. **coin_type'**: 0' (Bitcoin/generic, compatible with most tools) - hardened  
4. **account'**: 0' or 1' (separate accounts for different purposes) - hardened
5. **change**: 0 (external/receiving) or 1 (internal/change)
6. **address_index**: Sequential index (0, 1, 2, ...)

The apostrophe (') indicates **hardened derivation**, which provides additional security by preventing child private keys from compromising parent keys.

## Key Paths Used in ModernTensor

### 1. Payment Keys (Transactions)

**Path**: `m/44'/0'/0'/0/{index}`

- **Purpose**: Receiving tokens, sending transactions
- **Account**: 0 (primary account)
- **Change**: 0 (external addresses)
- **Examples**:
  - Root wallet: `m/44'/0'/0'/0/0`
  - Hotkey 0: `m/44'/0'/0'/0/0`
  - Hotkey 1: `m/44'/0'/0'/0/1`
  - Hotkey 2: `m/44'/0'/0'/0/2`

### 2. Stake Keys (Staking/Validation)

**Path**: `m/44'/0'/1'/0/{index}`

- **Purpose**: Staking operations, validator keys
- **Account**: 1 (separate account for security)
- **Change**: 0 (external addresses)
- **Examples**:
  - Root stake: `m/44'/0'/1'/0/0`
  - Hotkey 0 stake: `m/44'/0'/1'/0/0`
  - Hotkey 1 stake: `m/44'/0'/1'/0/1`
  - Hotkey 2 stake: `m/44'/0'/1'/0/2`

## Security Considerations

### ✅ Why Separate Accounts?

Using **different accounts** (0 vs 1) for payment and stake keys provides several security benefits:

1. **Key Isolation**: Compromise of payment key doesn't affect stake key
2. **Role Separation**: Clear distinction between operational and staking functions
3. **Audit Trail**: Easier to track which keys are used for what purpose
4. **Best Practice**: Follows industry standards (similar to Cardano's approach)

### ✅ Hardened Derivation

The first three levels use hardened derivation (`'`):
- **m/44'**: Purpose level - hardened
- **m/44'/0'**: Coin type level - hardened  
- **m/44'/0'/X'**: Account level - hardened

This prevents:
- Public key exposure from revealing sibling keys
- Child key compromise from affecting parent keys
- Account-level key leakage

## Implementation Examples

### Creating a Wallet

```python
from sdk.blockchain import L1HDWallet

# Generate new wallet with 24-word mnemonic
wallet = L1HDWallet()
print(f"Mnemonic: {wallet.mnemonic}")

# Or restore from existing mnemonic
wallet = L1HDWallet.from_mnemonic("word1 word2 ... word24")
```

### Deriving Keys

```python
# Payment key for index 0
payment_key_0 = wallet.derive_key("m/44'/0'/0'/0/0")
address_0 = payment_key_0.address()

# Stake key for index 0
stake_key_0 = wallet.derive_key("m/44'/0'/1'/0/0")
stake_address_0 = stake_key_0.address()

# Using helper method for hotkeys
hotkey_1 = wallet.derive_hotkey(1)  # Automatically uses m/44'/0'/0'/0/1
```

### Key Mapping Table

| Index | Payment Path | Stake Path | Use Case |
|-------|-------------|------------|----------|
| 0 | m/44'/0'/0'/0/0 | m/44'/0'/1'/0/0 | Root wallet / First hotkey |
| 1 | m/44'/0'/0'/0/1 | m/44'/0'/1'/0/1 | Second hotkey |
| 2 | m/44'/0'/0'/0/2 | m/44'/0'/1'/0/2 | Third hotkey |
| ... | ... | ... | ... |

## Comparison with Other Chains

### Cardano (CIP-1852)
```
Payment: m/1852'/1815'/0'/0/{index}
Stake:   m/1852'/1815'/0'/2/{index}
```

### Ethereum (BIP44)
```
Default: m/44'/60'/0'/0/{index}
```

### ModernTensor (BIP44)
```
Payment: m/44'/0'/0'/0/{index}
Stake:   m/44'/0'/1'/0/{index}
```

## Future Considerations

### Coin Type Registration

Currently using `0'` (Bitcoin/generic) for maximum compatibility. Future options:

1. **Register ModernTensor coin type** with SLIP-0044
   - Apply for official coin type number
   - Use `m/44'/XXXX'/...` where XXXX is assigned number
   
2. **Ethereum compatibility mode**
   - Use `m/44'/60'/...` for Ethereum tooling compatibility
   
3. **Keep current** (recommended for now)
   - Maintains compatibility with existing tools
   - Easier for users to import into standard wallets

### Change Addresses

Optional future enhancement:
```
External: m/44'/0'/0'/0/{index}  # Receiving addresses
Internal: m/44'/0'/0'/1/{index}  # Change addresses
```

## Best Practices

### For Developers

1. **Always use hardened derivation** for account-level and above
2. **Never reuse keys** between payment and staking
3. **Cache derived keys** to avoid repeated derivation
4. **Validate path format** before derivation
5. **Secure mnemonic storage** with encryption

### For Users

1. **Backup mnemonic phrase** securely (24 words)
2. **Never share mnemonic** with anyone
3. **Use different indices** for different purposes
4. **Keep derivation indices** recorded for recovery
5. **Test recovery** before large amounts

## References

- [BIP32](https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki) - HD Wallets
- [BIP39](https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki) - Mnemonic Phrases
- [BIP44](https://github.com/bitcoin/bips/blob/master/bip-0044.mediawiki) - Multi-Account Hierarchy
- [SLIP-0044](https://github.com/satoshilabs/slips/blob/master/slip-0044.md) - Registered Coin Types

## Summary

ModernTensor's key organization:
- ✅ **Secure**: Hardened derivation, separate accounts
- ✅ **Standard**: Follows BIP44 specification
- ✅ **Compatible**: Works with standard HD wallet tools
- ✅ **Flexible**: Easy to add new key types/purposes
- ✅ **Clear**: Explicit separation between payment and staking

The current implementation provides a solid foundation that balances security, compatibility, and usability.
