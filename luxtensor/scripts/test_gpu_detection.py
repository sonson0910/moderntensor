#!/usr/bin/env python3
"""
GPU Detection Test Script for Luxtensor
Tests GPU detection and bonus calculation with your RTX 5070
"""

import subprocess
import json
import sys

def detect_nvidia_gpu():
    """Detect NVIDIA GPU using nvidia-smi"""
    try:
        result = subprocess.run(
            ["nvidia-smi", "--query-gpu=name,memory.total,driver_version,compute_cap",
             "--format=csv,noheader,nounits"],
            capture_output=True, text=True, timeout=10
        )

        if result.returncode != 0:
            print("❌ nvidia-smi failed:", result.stderr)
            return None

        gpus = []
        for line in result.stdout.strip().split('\n'):
            parts = [p.strip() for p in line.split(',')]
            if len(parts) >= 4:
                gpus.append({
                    'name': parts[0],
                    'memory_mb': int(parts[1]),
                    'driver_version': parts[2],
                    'compute_cap': parts[3]
                })
        return gpus

    except FileNotFoundError:
        print("❌ nvidia-smi not found. Is NVIDIA driver installed?")
        return None
    except Exception as e:
        print(f"❌ Error detecting GPU: {e}")
        return None


def classify_gpu(memory_mb: int) -> tuple:
    """Classify GPU and return (capability, bonus_multiplier)"""
    if memory_mb >= 40_000:
        return ("Professional (A100/H100 class)", 1.40)
    elif memory_mb >= 16_000:
        return ("Advanced (RTX 4080/5080 class)", 1.30)
    elif memory_mb >= 8_000:
        return ("Basic (RTX 3060/5070 class)", 1.20)
    else:
        return ("None (insufficient VRAM)", 1.00)


def calculate_reward_example(stake: int, gpu_bonus: float):
    """Calculate example rewards with GPU bonus"""
    # Base reward calculation (simplified)
    base_pool = 1_000_000  # 1M LUX emission pool
    total_stake = 10_000_000  # Assume 10M total staked

    # Logarithmic stake adjustment
    import math
    normalized = stake / (10 ** 18)  # Assume 18 decimals
    if normalized <= 1:
        effective_stake = stake
    else:
        log_factor = math.log(normalized + 1) / (normalized + 1)
        effective_factor = max(log_factor, 0.10)
        effective_stake = stake * effective_factor

    # Calculate share with GPU bonus
    base_reward = (effective_stake / total_stake) * base_pool
    gpu_boosted = base_reward * gpu_bonus

    return {
        'actual_stake': stake,
        'effective_stake': effective_stake,
        'base_reward': base_reward,
        'gpu_boosted_reward': gpu_boosted,
        'bonus_amount': gpu_boosted - base_reward
    }


def main():
    print("=" * 60)
    print("🖥️  LUXTENSOR GPU DETECTION TEST")
    print("=" * 60)
    print()

    # Detect GPU
    print("📡 Detecting NVIDIA GPU...")
    gpus = detect_nvidia_gpu()

    if not gpus:
        print("\n⚠️  No NVIDIA GPU detected. You will run as CPU-only node.")
        print("   GPU Capability: None")
        print("   Bonus Multiplier: 1.0x")
        return

    # Show detected GPUs
    print(f"\n✅ Found {len(gpus)} GPU(s):\n")

    for i, gpu in enumerate(gpus):
        capability, bonus = classify_gpu(gpu['memory_mb'])

        print(f"  GPU #{i + 1}: {gpu['name']}")
        print(f"  ├── VRAM: {gpu['memory_mb']} MB")
        print(f"  ├── Driver: {gpu['driver_version']}")
        print(f"  ├── Compute Capability: {gpu['compute_cap']}")
        print(f"  ├── Luxtensor Classification: {capability}")
        print(f"  └── Bonus Multiplier: {bonus}x (+{int((bonus-1)*100)}%)")
        print()

    # Get best GPU
    best_gpu = max(gpus, key=lambda g: g['memory_mb'])
    _, best_bonus = classify_gpu(best_gpu['memory_mb'])

    # Example reward calculation
    print("=" * 60)
    print("💰 REWARD CALCULATION EXAMPLE")
    print("=" * 60)
    print()

    # Test with different stake amounts
    stakes = [
        1_000 * 10**18,      # 1,000 LUX
        10_000 * 10**18,     # 10,000 LUX
        100_000 * 10**18,    # 100,000 LUX
    ]

    print(f"Using GPU bonus: {best_bonus}x\n")
    print(f"{'Stake':<15} {'Effective':<15} {'Base Reward':<15} {'With GPU':<15} {'Bonus':<10}")
    print("-" * 70)

    for stake in stakes:
        result = calculate_reward_example(stake, best_bonus)
        stake_fmt = f"{stake // 10**18:,} LUX"
        eff_fmt = f"{result['effective_stake'] / 10**18:,.0f} LUX"
        base_fmt = f"{result['base_reward']:,.2f}"
        boosted_fmt = f"{result['gpu_boosted_reward']:,.2f}"
        bonus_fmt = f"+{result['bonus_amount']:,.2f}"
        print(f"{stake_fmt:<15} {eff_fmt:<15} {base_fmt:<15} {boosted_fmt:<15} {bonus_fmt:<10}")

    print()
    print("=" * 60)
    print("🎯 SUMMARY")
    print("=" * 60)
    print(f"  Your GPU: {best_gpu['name']}")
    print(f"  Reward Bonus: +{int((best_bonus-1)*100)}%")
    print(f"  AI Capable: {'✅ Yes' if best_gpu['memory_mb'] >= 8000 else '❌ No'}")
    print()
    print("  Your RTX 5070 qualifies for the Basic GPU tier,")
    print("  giving you a 20% bonus on validator rewards!")
    print()


if __name__ == "__main__":
    main()
