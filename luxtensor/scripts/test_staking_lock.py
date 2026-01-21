#!/usr/bin/env python3
"""
Test Staking Lock/Unlock với các trường hợp success và fail
"""

import sys
import json
import time
sys.path.insert(0, 'd:/venera/cardano/moderntensor/moderntensor')

from sdk.luxtensor_client import LuxtensorClient

def main():
    print("=" * 70)
    print("🔒 STAKING LOCK/UNLOCK TEST")
    print("=" * 70)

    client = LuxtensorClient("http://localhost:8545")

    # Check nodes
    block = client.get_block_number()
    print(f"\n📊 Current Block: {block}")

    test_addr = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
    print(f"📍 Test Address: {test_addr}")

    # =========================================================================
    print("\n" + "=" * 70)
    print("TEST 1: Lock stake for 30 days (10% bonus)")
    print("=" * 70)

    try:
        result = client._call_rpc("staking_lockStake", [
            test_addr,
            "0x3635c9adc5dea00000",  # 1000 LUX
            30  # 30 days
        ])
        print(f"✅ Lock successful!")
        print(f"   Amount: {result.get('amount')}")
        print(f"   Lock days: {result.get('lock_days')}")
        print(f"   Bonus rate: {result.get('bonus_rate')}")
        print(f"   Unlock timestamp: {result.get('unlock_timestamp')}")
    except Exception as e:
        print(f"❌ Lock failed: {e}")

    # =========================================================================
    print("\n" + "=" * 70)
    print("TEST 2: Try to unlock BEFORE lock period expires (SHOULD FAIL)")
    print("=" * 70)

    try:
        result = client._call_rpc("staking_unlockStake", [test_addr])
        print(f"⚠️  Unlock succeeded (unexpected!): {result}")
    except Exception as e:
        error_msg = str(e)
        if "Lock not expired" in error_msg:
            print(f"✅ Unlock correctly FAILED!")
            print(f"   Error: {error_msg}")
        else:
            print(f"❌ Unexpected error: {e}")

    # =========================================================================
    print("\n" + "=" * 70)
    print("TEST 3: Get lock info")
    print("=" * 70)

    try:
        result = client._call_rpc("staking_getLockInfo", [test_addr])
        print(f"📋 Lock Info:")
        print(f"   Locked: {result.get('locked')}")
        print(f"   Amount: {result.get('amount')}")
        print(f"   Unlock timestamp: {result.get('unlock_timestamp')}")
        print(f"   Bonus rate: {result.get('bonus_rate')}")
        print(f"   Is unlockable: {result.get('is_unlockable')}")
        print(f"   Remaining seconds: {result.get('remaining_seconds')}")
    except Exception as e:
        print(f"❌ Get lock info failed: {e}")

    # =========================================================================
    print("\n" + "=" * 70)
    print("TEST 4: Try to lock again while already locked (SHOULD FAIL)")
    print("=" * 70)

    try:
        result = client._call_rpc("staking_lockStake", [
            test_addr,
            "0x56bc75e2d63100000",  # 100 LUX
            7  # 7 days
        ])
        print(f"⚠️  Lock succeeded (unexpected!): {result}")
    except Exception as e:
        error_msg = str(e)
        if "already locked" in error_msg:
            print(f"✅ Re-lock correctly FAILED!")
            print(f"   Error: {error_msg}")
        else:
            print(f"❌ Unexpected error: {e}")

    # =========================================================================
    print("\n" + "=" * 70)
    print("TEST 5: Try lock with invalid period (< 7 days) (SHOULD FAIL)")
    print("=" * 70)

    addr2 = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8"
    try:
        result = client._call_rpc("staking_lockStake", [
            addr2,
            "0x56bc75e2d63100000",  # 100 LUX
            3  # Invalid: only 3 days
        ])
        print(f"⚠️  Lock succeeded (unexpected!): {result}")
    except Exception as e:
        error_msg = str(e)
        if "Minimum lock period" in error_msg:
            print(f"✅ Short lock correctly FAILED!")
            print(f"   Error: {error_msg}")
        else:
            print(f"❌ Unexpected error: {e}")

    # =========================================================================
    print("\n" + "=" * 70)
    print("TEST 6: Lock with different periods for bonus rates")
    print("=" * 70)

    test_cases = [
        ("0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC", 7, "0%"),
        ("0x90F79bf6EB2c4f870365E785982E1f101E93b906", 90, "30%"),
        ("0x15d34AAf54267DB7D7c367839AAf71A00a2C6A65", 180, "60%"),
    ]

    for addr, days, expected_bonus in test_cases:
        try:
            result = client._call_rpc("staking_lockStake", [
                addr,
                "0x56bc75e2d63100000",  # 100 LUX
                days
            ])
            actual_bonus = result.get('bonus_rate', 'N/A')
            status = "✅" if actual_bonus == expected_bonus else "⚠️"
            print(f"   {status} {days} days: bonus = {actual_bonus} (expected {expected_bonus})")
        except Exception as e:
            print(f"   ❌ {days} days: failed - {e}")

    # =========================================================================
    print("\n" + "=" * 70)
    print("TEST 7: Unlock for address with no lock (SHOULD FAIL)")
    print("=" * 70)

    unlocked_addr = "0x9965507D1a55bcC2695C58ba16FB37d819B0A4dc"
    try:
        result = client._call_rpc("staking_unlockStake", [unlocked_addr])
        print(f"⚠️  Unlock succeeded (unexpected!): {result}")
    except Exception as e:
        error_msg = str(e)
        if "No locked stake found" in error_msg:
            print(f"✅ Unlock non-existent correctly FAILED!")
            print(f"   Error: {error_msg}")
        else:
            print(f"❌ Unexpected error: {e}")

    print("\n" + "=" * 70)
    print("✅ ALL STAKING LOCK/UNLOCK TESTS COMPLETE!")
    print("=" * 70)

if __name__ == "__main__":
    main()
