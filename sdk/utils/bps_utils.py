"""
BPS (Basis Points) Utilities

Provides conversion utilities between basis points (BPS) and float values.

BPS are used in Luxtensor consensus for deterministic fixed-point arithmetic,
avoiding floating-point precision issues that could cause consensus divergence.

1 BPS = 0.01% = 0.0001
10000 BPS = 100% = 1.0
"""

# Constants
MAX_BPS = 10000
BPS_PRECISION = 10000


def float_to_bps(value: float) -> int:
    """Convert float (0.0-1.0) to BPS (0-10000)."""
    if not 0.0 <= value <= 1.0:
        raise ValueError(f"Value must be between 0.0 and 1.0, got {value}")
    return int(value * BPS_PRECISION)


def bps_to_float(bps: int) -> float:
    """Convert BPS (0-10000) to float (0.0-1.0)."""
    return bps / BPS_PRECISION


def bps_to_percent(bps: int) -> float:
    """Convert BPS to percentage (0-100)."""
    return bps / 100.0


def percent_to_bps(percent: float) -> int:
    """Convert percentage (0-100) to BPS."""
    if not 0.0 <= percent <= 100.0:
        raise ValueError(f"Percent must be between 0.0 and 100.0, got {percent}")
    return int(percent * 100)


def calculate_proportional_share(total_amount: int, share_bps: int) -> int:
    """Calculate proportional share using integer BPS arithmetic."""
    return (total_amount * share_bps) // MAX_BPS


def distribute_by_scores(total_amount: int, scores_bps: list[int]) -> list[int]:
    """Distribute total amount proportionally by BPS scores."""
    if not scores_bps:
        return []

    total_score = sum(scores_bps)
    if total_score == 0:
        base = total_amount // len(scores_bps)
        return [base] * len(scores_bps)

    shares = [(total_amount * score) // total_score for score in scores_bps]

    # Distribute remainder to highest scorers
    remainder = total_amount - sum(shares)
    if remainder > 0:
        indexed = sorted(enumerate(scores_bps), key=lambda x: x[1], reverse=True)
        for i in range(remainder):
            shares[indexed[i % len(indexed)][0]] += 1

    return shares


def validate_bps(bps: int, name: str = "value") -> int:
    """Validate that a value is a valid BPS (0-10000)."""
    if not isinstance(bps, int):
        raise TypeError(f"{name} must be an integer")
    if not 0 <= bps <= MAX_BPS:
        raise ValueError(f"{name} must be between 0 and {MAX_BPS}, got {bps}")
    return bps
