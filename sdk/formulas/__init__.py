# sdk/formulas/__init__.py

"""
Package initializer for the formulas module.

Exports the main calculation functions used throughout the Moderntensor SDK's
consensus and economic mechanisms.
"""

# Import main calculation functions from submodules
from .dao import calculate_voting_power
from .incentive import calculate_miner_incentive, calculate_validator_incentive
from .miner_weight import calculate_miner_weight
from .penalty import (
    calculate_performance_adjustment,
    calculate_slash_amount,
    calculate_fraud_severity_value # Hàm này có thể trừu tượng, export tùy ý
)
from .performance import (
    calculate_task_completion_rate,
    calculate_adjusted_miner_performance,
    calculate_validator_performance,
    calculate_penalty_term
)
from .resource_allocation import calculate_subnet_resource
from .trust_score import (
    update_trust_score, 
    calculate_selection_probability,
    calculate_consensus_score,  # Added for consensus mechanism
    calculate_validator_consensus_weight,  # Added for consensus mechanism
)
from .validator_weight import calculate_validator_weight

# Optional: Import utility functions if they are intended for public use outside the formulas package
# from .utils import sigmoid, calculate_alpha_effective

# Define __all__ to explicitly declare the public API of the formulas package
__all__ = [
    # dao
    "calculate_voting_power",
    # incentive
    "calculate_miner_incentive",
    "calculate_validator_incentive",
    # miner_weight
    "calculate_miner_weight",
    # penalty
    "calculate_performance_adjustment",
    "calculate_slash_amount",
    # "calculate_fraud_severity_value", # Uncomment if needed
    # performance
    "calculate_task_completion_rate",
    "calculate_adjusted_miner_performance",
    "calculate_validator_performance",
    "calculate_penalty_term",
    # resource_allocation
    "calculate_subnet_resource",
    # trust_score
    "update_trust_score",
    "calculate_selection_probability",
    "calculate_consensus_score",  # Added
    "calculate_validator_consensus_weight",  # Added
    # validator_weight
    "calculate_validator_weight",
    # Add utils if exported
    # "sigmoid",
    # "calculate_alpha_effective",
]