# sdk/config/settings.py
"""
ModernTensor SDK Configuration - LuxTensor Edition

Centralized configuration management for the SDK.
Loads from environment variables or .env file.
"""

import logging
import math
from typing import Optional
import re

from pydantic import Field, field_validator
from pydantic_settings import BaseSettings, SettingsConfigDict

# Network enum for LuxTensor
from enum import Enum


class Network(Enum):
    """Network type for LuxTensor blockchain operations."""
    MAINNET = "mainnet"
    TESTNET = "testnet"
    DEVNET = "devnet"


# Try to import coloredlogs, fallback to basic if not available
try:
    import coloredlogs
    COLOREDLOGS_AVAILABLE = True
except ImportError:
    COLOREDLOGS_AVAILABLE = False

# ANSI color codes
RED = "\033[91m"
YELLOW = "\033[93m"
RESET = "\033[0m"

# Regex for highlighting
HEX_ID_REGEX = re.compile(r"(\b[a-fA-F0-9]{56,64}\b)")
SCORE_REGEX = re.compile(r"(\bScored\b)", re.IGNORECASE)


class Settings(BaseSettings):
    """
    Centralized configuration for ModernTensor SDK.
    Loads from environment variables or .env file.
    Updated for LuxTensor (removed Cardano dependencies).
    """

    model_config = SettingsConfigDict(
        extra="ignore",
        env_file=".env",
        env_file_encoding="utf-8",
        env_prefix="MODERNTENSOR_",
    )

    # =========================================================================
    # LUXTENSOR BLOCKCHAIN SETTINGS
    # =========================================================================

    # RPC endpoint
    LUXTENSOR_RPC_URL: str = Field(
        default="http://localhost:8545",
        alias="LUXTENSOR_RPC_URL",
        description="LuxTensor blockchain RPC endpoint URL"
    )

    # Network
    NETWORK: str = Field(
        default="testnet",
        alias="NETWORK",
        description="Network type: mainnet, testnet, or devnet"
    )

    # =========================================================================
    # KEY MANAGEMENT
    # =========================================================================

    HOTKEY_BASE_DIR: str = Field(
        default="moderntensor",
        alias="HOTKEY_BASE_DIR"
    )
    COLDKEY_NAME: str = Field(
        default="default",
        alias="COLDKEY_NAME"
    )
    HOTKEY_NAME: str = Field(
        default="hotkey1",
        alias="HOTKEY_NAME"
    )

    # =========================================================================
    # ENCRYPTION SETTINGS
    # =========================================================================

    ENCRYPTION_PBKDF2_ITERATIONS: int = Field(
        default=100_000,
        alias="ENCRYPTION_PBKDF2_ITERATIONS",
        description="Number of iterations for PBKDF2 key derivation.",
    )

    # =========================================================================
    # NODE SETTINGS
    # =========================================================================

    API_PORT: int = Field(
        default=8545,
        alias="API_PORT",
        description="Port for RPC server"
    )

    VALIDATOR_UID: Optional[str] = Field(
        default=None,
        alias="VALIDATOR_UID",
        description="UID of this validator node"
    )

    VALIDATOR_ADDRESS: Optional[str] = Field(
        default=None,
        alias="VALIDATOR_ADDRESS",
        description="Address of this validator"
    )

    # =========================================================================
    # LOGGING
    # =========================================================================

    LOG_LEVEL: str = Field(
        default="INFO",
        alias="LOG_LEVEL",
        description="Logging level (DEBUG, INFO, WARNING, ERROR, CRITICAL)",
    )

    # =========================================================================
    # CONSENSUS PARAMETERS
    # =========================================================================

    # Slot-Based Cycle Timing
    CONSENSUS_CYCLE_SLOT_LENGTH: int = Field(
        default=600,
        alias="CONSENSUS_CYCLE_SLOT_LENGTH",
        description="Length of consensus cycle in slots.",
    )

    CONSENSUS_SLOT_QUERY_INTERVAL_SECONDS: float = Field(
        default=0.5,
        alias="CONSENSUS_SLOT_QUERY_INTERVAL_SECONDS",
        description="Interval between slot queries in seconds.",
    )

    # Offsets (slots before cycle end)
    CONSENSUS_COMMIT_SLOTS_OFFSET: int = Field(
        default=30,
        alias="CONSENSUS_COMMIT_SLOTS_OFFSET",
    )
    CONSENSUS_TIMEOUT_SLOTS_OFFSET: int = Field(
        default=60,
        alias="CONSENSUS_TIMEOUT_SLOTS_OFFSET",
    )
    CONSENSUS_BROADCAST_SLOTS_OFFSET: int = Field(
        default=90,
        alias="CONSENSUS_BROADCAST_SLOTS_OFFSET",
    )
    CONSENSUS_TASKING_END_SLOTS_OFFSET: int = Field(
        default=120,
        alias="CONSENSUS_TASKING_END_SLOTS_OFFSET",
    )

    # Tasking Phase
    CONSENSUS_TASKING_PHASE_RATIO: float = Field(
        default=0.85,
        alias="CONSENSUS_TASKING_PHASE_RATIO",
        description="Ratio of cycle for tasking phase.",
    )

    # Mini-Batch Settings
    CONSENSUS_ENABLE_MINI_BATCH: bool = Field(
        default=True,
        alias="CONSENSUS_ENABLE_MINI_BATCH",
    )
    CONSENSUS_MINI_BATCH_SIZE: int = Field(
        default=5,
        alias="CONSENSUS_MINI_BATCH_SIZE",
    )
    CONSENSUS_MINI_BATCH_WAIT_SECONDS: int = Field(
        default=30,
        alias="CONSENSUS_MINI_BATCH_WAIT_SECONDS",
    )
    CONSENSUS_MINI_BATCH_INTERVAL_SECONDS: int = Field(
        default=5,
        alias="CONSENSUS_MINI_BATCH_INTERVAL_SECONDS",
    )

    # Limits
    CONSENSUS_MAX_PERFORMANCE_HISTORY_LEN: int = Field(default=100)
    CONSENSUS_MIN_VALIDATORS_FOR_CONSENSUS: int = Field(default=3)
    CONSENSUS_MAX_RESULTS_BUFFER_SIZE: int = Field(default=10000)
    CONSENSUS_NETWORK_TIMEOUT_SECONDS: int = Field(default=10)

    # Miner Selection
    CONSENSUS_NUM_MINERS_TO_SELECT: int = Field(default=5)
    CONSENSUS_PARAM_BETA: float = Field(default=0.2, ge=0.0)
    CONSENSUS_PARAM_MAX_TIME_BONUS: int = Field(default=10, ge=0)

    # Trust Score
    CONSENSUS_PARAM_DELTA_TRUST: float = Field(default=0.01, ge=0.0, lt=1.0)
    CONSENSUS_PARAM_ALPHA_BASE: float = Field(default=0.1, gt=0.0, le=1.0)
    CONSENSUS_PARAM_K_ALPHA: float = Field(default=1.0, ge=0.0, le=2.0)

    # Incentive Sigmoid
    CONSENSUS_PARAM_UPDATE_SIG_L: float = Field(default=1.0, gt=0.0)
    CONSENSUS_PARAM_UPDATE_SIG_K: float = Field(default=10.0, gt=0.0)
    CONSENSUS_PARAM_UPDATE_SIG_X0: float = Field(default=0.5)
    CONSENSUS_PARAM_INCENTIVE_SIG_L: float = Field(default=1.0, gt=0.0)
    CONSENSUS_PARAM_INCENTIVE_SIG_K: float = Field(default=10.0, gt=0.0)
    CONSENSUS_PARAM_INCENTIVE_SIG_X0: float = Field(default=0.5)

    # Fraud Detection
    CONSENSUS_DATUM_COMPARISON_TOLERANCE: float = Field(default=1e-5, gt=0.0)
    CONSENSUS_PARAM_PENALTY_ETA: float = Field(default=0.1, ge=0.0, le=1.0)
    CONSENSUS_JAILED_SEVERITY_THRESHOLD: float = Field(default=0.2, ge=0.0, le=1.0)
    CONSENSUS_PARAM_MAX_SLASH_RATE: float = Field(default=0.05, ge=0.0, le=1.0)

    # Stake Dampening
    CONSENSUS_STAKE_DAMPENING_ENABLED: bool = Field(default=True)
    CONSENSUS_STAKE_DAMPENING_FACTOR: float = Field(default=0.5, ge=0.1, le=1.0)

    # Weight Calculation
    CONSENSUS_PARAM_DELTA_W: float = Field(default=0.05, ge=0.0, lt=1.0)
    CONSENSUS_PARAM_LAMBDA_BALANCE: float = Field(default=0.5, ge=0.0, le=1.0)
    CONSENSUS_PARAM_STAKE_LOG_BASE: float = Field(default=math.e, gt=1.0)
    CONSENSUS_PARAM_TIME_LOG_BASE: float = Field(default=10, gt=1.0)

    # Validator Performance (E_v)
    CONSENSUS_PARAM_THETA1: float = Field(default=0.1, ge=0.0)
    CONSENSUS_PARAM_THETA2: float = Field(default=0.6, ge=0.0)
    CONSENSUS_PARAM_THETA3: float = Field(default=0.3, ge=0.0)
    CONSENSUS_PARAM_PENALTY_THRESHOLD_DEV: float = Field(default=0.05, ge=0.0)
    CONSENSUS_PARAM_PENALTY_K_PENALTY: float = Field(default=10.0, ge=0.0)
    CONSENSUS_PARAM_PENALTY_P_PENALTY: float = Field(default=1.0, ge=1.0)

    # DAO
    CONSENSUS_PARAM_DAO_KG: float = Field(default=1.0, ge=0.0)
    CONSENSUS_PARAM_DAO_TOTAL_TIME: float = Field(
        default=365.0 * 24 * 60 * 60,
        gt=0.0,
    )

    @field_validator("NETWORK", mode="before")
    def validate_network(cls, value: Optional[str]):
        if value is None:
            value = "testnet"
        normalized = str(value).lower().strip()
        if normalized == "mainnet":
            return Network.MAINNET.value
        elif normalized == "devnet":
            return Network.DEVNET.value
        return Network.TESTNET.value


# Create settings instance
try:
    settings = Settings()

    # Validate theta sum
    theta_sum = (
        settings.CONSENSUS_PARAM_THETA1 +
        settings.CONSENSUS_PARAM_THETA2 +
        settings.CONSENSUS_PARAM_THETA3
    )
    if not math.isclose(theta_sum, 1.0, abs_tol=1e-9):
        logging.warning("Sum of Theta parameters (%s) != 1.0", theta_sum)

except Exception as e:
    print("CRITICAL: Error loading settings: %s" % e)
    settings = Settings()


# Configure logging (scoped to 'sdk' namespace to avoid hijacking root logger)
SDK_LOGGER_NAME = "sdk"
try:
    log_level_str = settings.LOG_LEVEL.upper()
    if log_level_str not in ["DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"]:
        log_level_str = "INFO"
    LOG_LEVEL_CONFIG = getattr(logging, log_level_str)
except Exception:
    LOG_LEVEL_CONFIG = logging.INFO

# Configure logging format
LOG_FORMAT = "%(asctime)s [%(levelname)s] %(name)s: %(message)s"

sdk_logger = logging.getLogger(SDK_LOGGER_NAME)
sdk_logger.setLevel(LOG_LEVEL_CONFIG)

# Clear existing handlers on SDK logger
if sdk_logger.hasHandlers():
    for handler in sdk_logger.handlers[:]:
        sdk_logger.removeHandler(handler)

if COLOREDLOGS_AVAILABLE:
    coloredlogs.install(
        level=LOG_LEVEL_CONFIG,
        fmt=LOG_FORMAT,
        logger=sdk_logger,
        reconfigure=True,
    )
else:
    _handler = logging.StreamHandler()
    _handler.setFormatter(logging.Formatter(LOG_FORMAT))
    sdk_logger.addHandler(_handler)

logger = logging.getLogger(__name__)
logger.info("Settings loaded. Log level: %s", logging.getLevelName(LOG_LEVEL_CONFIG))
