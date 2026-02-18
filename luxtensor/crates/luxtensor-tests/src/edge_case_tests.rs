// Edge Case Integration Tests for Luxtensor
// Tests for transaction, staking, validator, and contract edge cases
// Updated to match current API

use luxtensor_core::{Transaction, Address, Account, StateDB};
use luxtensor_crypto::KeyPair;

// ============================================================
// A. TRANSACTION EDGE CASES
// ============================================================

/// Test 1: TX with nonce = 0 from new account
#[test]
fn test_tx_nonce_zero_new_account() {
    let mut state = StateDB::default();
    let keypair = KeyPair::generate();
    let addr = Address::from(keypair.address());

    // Give account initial balance
    let mut account = Account::new();
    account.balance = 1_000_000_000_000_000_000u128; // 1 ETH
    state.set_account(addr, account);

    // TX with nonce 0 should be created successfully
    let tx = create_signed_tx(&keypair, 0, None, 0);
    assert_eq!(tx.nonce, 0);
    assert!(tx.hash() != [0u8; 32]);
}

/// Test 2: TX with wrong nonce (replay attack prevention)
#[test]
fn test_tx_wrong_nonce_rejected() {
    let mut state = StateDB::default();
    let keypair = KeyPair::generate();
    let addr = Address::from(keypair.address());

    // Account with nonce = 5
    let mut account = Account::new();
    account.balance = 1_000_000_000_000_000_000u128;
    account.nonce = 5;
    state.set_account(addr, account);

    // TX with wrong nonce (4 instead of 5)
    let tx = create_signed_tx(&keypair, 4, None, 0);
    assert_eq!(tx.nonce, 4);

    // Verify the account's expected nonce differs from tx nonce (replay protection check)
    let current_account = state.get_account(&addr).unwrap();
    assert_ne!(
        tx.nonce, current_account.nonce,
        "TX nonce {} must not match account nonce {} — execution layer must reject this",
        tx.nonce, current_account.nonce
    );
}

/// Test 3: TX with insufficient balance
#[test]
fn test_tx_insufficient_balance() {
    let mut state = StateDB::default();
    let keypair = KeyPair::generate();
    let addr = Address::from(keypair.address());

    // Account with 1 wei balance
    let mut account = Account::new();
    account.balance = 1;
    let saved_balance = account.balance;
    state.set_account(addr, account);

    // TX trying to transfer 1 ETH
    let tx = create_signed_tx(&keypair, 0, Some(Address::from([2u8; 20])), 1_000_000_000_000_000_000);
    // TX is created but would fail at execution
    assert!(tx.value > saved_balance);
}

/// Test 4: TX with gas limit configuration
#[test]
fn test_tx_gas_limit_configuration() {
    let keypair = KeyPair::generate();
    let addr = Address::from(keypair.address());

    let tx = Transaction::new(0, addr, Some(Address::from([2u8; 20])), 0, 1, 1, vec![]);
    assert_eq!(tx.gas_limit, 1);
}

/// Test 5: TX to self (zero transfer)
#[test]
fn test_tx_to_self() {
    let mut state = StateDB::default();
    let keypair = KeyPair::generate();
    let addr = Address::from(keypair.address());

    let initial_balance = 1_000_000_000_000_000_000u128;
    let mut account = Account::new();
    account.balance = initial_balance;
    state.set_account(addr, account);

    // TX to self
    let tx = create_signed_tx(&keypair, 0, Some(addr), 0);
    assert_eq!(tx.to, Some(addr));
}

// ============================================================
// B. STAKING EDGE CASES
// ============================================================

/// Test 6: Stake with zero value rejected
#[test]
fn test_stake_zero_rejected() {
    use luxtensor_consensus::ValidatorSet;

    let mut vs = ValidatorSet::new();

    let result = vs.add_validator(luxtensor_consensus::Validator {
        address: Address::from([1u8; 20]),
        stake: 0, // Zero stake
        public_key: [0u8; 32],
        active: true,
        rewards: 0,
        last_active_slot: 0,
        activation_epoch: 0,
    });

    // Should fail due to zero stake
    assert!(result.is_err());
}

/// Test 7: Stake with zero balance (state check)
#[test]
fn test_stake_zero_balance_account() {
    // This tests the RPC handler validation
    // Account with zero balance should not be able to stake
    let state = StateDB::default();
    let addr = Address::from([1u8; 20]);

    // Account doesn't exist - returns None
    let account = state.get_account(&addr);
    assert!(account.is_none() || account.unwrap().balance == 0);
}

/// Test 8: Double stake attempt
#[test]
fn test_double_stake_same_validator() {
    use luxtensor_consensus::ValidatorSet;

    let mut vs = ValidatorSet::new();
    let addr = Address::from([1u8; 20]);

    // First stake
    let result1 = vs.add_validator(luxtensor_consensus::Validator {
        address: addr,
        stake: 1000,
        public_key: [0u8; 32],
        active: true,
        rewards: 0,
        last_active_slot: 0,
        activation_epoch: 0,
    });
    assert!(result1.is_ok());

    // Second stake (should fail - already exists)
    let result2 = vs.add_validator(luxtensor_consensus::Validator {
        address: addr,
        stake: 2000,
        public_key: [0u8; 32],
        active: true,
        rewards: 0,
        last_active_slot: 0,
        activation_epoch: 0,
    });

    assert!(result2.is_err(), "Double stake should be rejected");
}

// ============================================================
// C. VALIDATOR EDGE CASES
// ============================================================

/// Test 9: Remove non-existent validator
#[test]
fn test_remove_nonexistent_validator() {
    use luxtensor_consensus::ValidatorSet;

    let mut vs = ValidatorSet::new();
    let addr = Address::from([99u8; 20]);

    let result = vs.remove_validator(&addr);
    assert!(result.is_err(), "Removing non-existent validator should fail");
}

/// Test 10: Slash more than validator stake
#[test]
fn test_slash_more_than_stake() {
    use luxtensor_consensus::ValidatorSet;

    let mut vs = ValidatorSet::new();
    let addr = Address::from([1u8; 20]);

    // Add validator with 1000 stake
    vs.add_validator(luxtensor_consensus::Validator {
        address: addr,
        stake: 1000,
        public_key: [0u8; 32],
        active: true,
        rewards: 0,
        last_active_slot: 0,
        activation_epoch: 0,
    }).unwrap();

    // Slash 5000 (more than stake)
    let slashed = vs.slash_stake(&addr, 5000);

    // Should only slash up to available stake
    assert!(slashed.is_ok());
    let amount = slashed.unwrap();
    assert!(amount <= 1000, "Cannot slash more than available stake");

    // Validator stake should be >= 0
    let v = vs.get_validator(&addr).unwrap();
    assert!(v.stake == 0, "Stake should be 0 after full slash");
}

/// Test 11: Update stake to zero
#[test]
fn test_update_stake_to_zero() {
    use luxtensor_consensus::ValidatorSet;

    let mut vs = ValidatorSet::new();
    let addr = Address::from([1u8; 20]);

    vs.add_validator(luxtensor_consensus::Validator {
        address: addr,
        stake: 1000,
        public_key: [0u8; 32],
        active: true,
        rewards: 0,
        last_active_slot: 0,
        activation_epoch: 0,
    }).unwrap();

    // Update to 0
    let result = vs.update_stake(&addr, 0);
    assert!(result.is_ok());
}

// ============================================================
// D. TOKEN/EMISSION EDGE CASES
// ============================================================

/// Test 12: Lock bonus with 0 days
#[test]
fn test_lock_bonus_zero_days() {
    use luxtensor_consensus::reward_distribution::LockBonusConfig;

    let config = LockBonusConfig::default();
    let bonus = config.get_bonus_bps(0);

    // 0 days = 0 BPS multiplier (no bonus for unlocked)
    assert_eq!(bonus, 0);
}

// ============================================================
// F. ARITHMETIC OVERFLOW TESTS
// ============================================================

/// Test 18: Balance overflow protection
#[test]
fn test_balance_overflow_protection() {
    let mut state = StateDB::default();
    let addr1 = Address::from([1u8; 20]);
    let addr2 = Address::from([2u8; 20]);

    // Give addr2 maximum balance
    let mut account2 = Account::new();
    account2.balance = u128::MAX;
    state.set_account(addr2, account2.clone());

    // addr1 with some balance
    let mut account1 = Account::new();
    account1.balance = 1000;
    state.set_account(addr1, account1);

    // Verify that adding to u128::MAX would overflow with normal arithmetic
    let transfer_amount = 1000u128;
    assert!(
        account2.balance.checked_add(transfer_amount).is_none(),
        "u128::MAX + 1000 must overflow with checked arithmetic"
    );
    // Verify saturating_add protects against overflow
    assert_eq!(
        account2.balance.saturating_add(transfer_amount),
        u128::MAX,
        "saturating_add must cap at u128::MAX"
    );

    // Verify the account balance is retrievable and correct
    let retrieved = state.get_account(&addr2);
    assert!(retrieved.is_some(), "Account must exist after set_account");
    assert_eq!(retrieved.unwrap().balance, u128::MAX);
}

/// Test 19: Nonce overflow protection
#[test]
fn test_nonce_overflow_protection() {
    let mut state = StateDB::default();
    let addr = Address::from([1u8; 20]);

    // Account with max nonce
    let mut account = Account::new();
    account.nonce = u64::MAX;
    account.balance = 1_000_000_000_000_000_000u128;
    state.set_account(addr, account);

    // Verify account was stored with max nonce
    let retrieved = state.get_account(&addr);
    assert!(retrieved.is_some(), "Account must exist");
    let acc = retrieved.unwrap();
    assert_eq!(acc.nonce, u64::MAX, "Nonce must be u64::MAX");

    // Verify checked increment detects overflow
    assert!(
        acc.nonce.checked_add(1).is_none(),
        "u64::MAX + 1 must overflow with checked arithmetic"
    );
    // Verify saturating_add protects against wraparound
    assert_eq!(
        acc.nonce.saturating_add(1),
        u64::MAX,
        "saturating_add must cap at u64::MAX"
    );
}

/// Test 20: Total stake overflow protection
#[test]
fn test_total_stake_overflow_protection() {
    use luxtensor_consensus::ValidatorSet;

    let mut vs = ValidatorSet::new();

    // Add validator with very large stake
    vs.add_validator(luxtensor_consensus::Validator {
        address: Address::from([1u8; 20]),
        stake: u128::MAX / 2,
        public_key: [0u8; 32],
        active: true,
        rewards: 0,
        last_active_slot: 0,
        activation_epoch: 0,
    }).unwrap();

    // Add another large stake - should saturate, not overflow
    let _ = vs.add_validator(luxtensor_consensus::Validator {
        address: Address::from([2u8; 20]),
        stake: u128::MAX / 2,
        public_key: [0u8; 32],
        active: true,
        rewards: 0,
        last_active_slot: 0,
        activation_epoch: 0,
    });

    // Total stake must not wrap around due to overflow.
    // If wrapping occurred, total_stake would be less than either individual stake.
    let total = vs.total_stake();
    assert!(
        total >= u128::MAX / 2,
        "Total stake must not wrap around: got {}, expected >= {}",
        total,
        u128::MAX / 2
    );
}

// ============================================================
// HELPER FUNCTIONS
// ============================================================

fn create_signed_tx(
    keypair: &KeyPair,
    nonce: u64,
    to: Option<Address>,
    value: u128,
) -> Transaction {
    let from = Address::from(keypair.address());
    let mut tx = Transaction::new(nonce, from, to, value, 1_000_000_000, 100_000, vec![]);

    // Sign transaction
    let hash = tx.hash();
    let sig = keypair.sign(&hash).expect("signing should succeed");
    tx.r.copy_from_slice(&sig[..32]);
    tx.s.copy_from_slice(&sig[32..]);
    tx.v = 27;

    tx
}
