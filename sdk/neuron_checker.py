"""
ModernTensor SDK - Neuron Status Checker

Utilities for checking neuron registration, activity, and liveness.
Similar to Bittensor's registration verification mechanism.

This module provides:
- Registration status checking
- Activity monitoring (based on last_update)
- Endpoint liveness verification
- Validator permit checking

Usage:
    from sdk.neuron_checker import NeuronChecker, NeuronStatus

    checker = NeuronChecker(rpc_url="http://localhost:8545")

    # Check registration
    info = checker.check_registration(subnet_uid=1, hotkey="0x...")
    print(f"Status: {info.status}")

    # Quick activity check
    is_active = checker.is_neuron_active(subnet_uid=1, hotkey="0x...")

Author: ModernTensor Team
Version: 1.0.0
"""

import time
import logging
import asyncio
from typing import Dict, Any, List, Optional, Tuple
from dataclasses import dataclass
from enum import Enum
import httpx

logger = logging.getLogger(__name__)


# =============================================================================
# Enums and Data Classes
# =============================================================================

class NeuronStatus(str, Enum):
    """Neuron registration and activity status"""
    NOT_REGISTERED = "not_registered"  # Not on chain
    REGISTERED = "registered"  # On chain but status unknown
    ACTIVE = "active"  # Registered and recently active
    INACTIVE = "inactive"  # Registered but idle for too long
    UNREACHABLE = "unreachable"  # Registered but endpoint not responding


class NeuronType(str, Enum):
    """Type of neuron"""
    MINER = "miner"
    VALIDATOR = "validator"
    UNKNOWN = "unknown"


@dataclass
class NeuronRegistrationInfo:
    """Complete registration and activity information for a neuron"""

    # Basic info
    is_registered: bool = False
    uid: Optional[int] = None
    hotkey: Optional[str] = None
    coldkey: Optional[str] = None
    subnet_uid: int = 0
    neuron_type: NeuronType = NeuronType.UNKNOWN

    # Activity info
    last_update_block: int = 0
    blocks_since_update: int = 0
    current_block: int = 0
    is_active_on_chain: bool = False

    # Staking info
    stake: int = 0
    stake_formatted: str = "0"

    # Network info
    endpoint: str = ""
    is_reachable: bool = False
    response_time_ms: float = 0

    # Validator specific
    validator_permit: bool = False
    validator_trust: float = 0.0

    # Performance metrics
    trust: float = 0.0
    incentive: float = 0.0
    emission: int = 0

    # Overall status
    status: NeuronStatus = NeuronStatus.NOT_REGISTERED
    status_message: str = ""

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary"""
        return {
            "is_registered": self.is_registered,
            "uid": self.uid,
            "hotkey": self.hotkey,
            "coldkey": self.coldkey,
            "subnet_uid": self.subnet_uid,
            "neuron_type": self.neuron_type.value,
            "last_update_block": self.last_update_block,
            "blocks_since_update": self.blocks_since_update,
            "current_block": self.current_block,
            "is_active_on_chain": self.is_active_on_chain,
            "stake": self.stake,
            "stake_formatted": self.stake_formatted,
            "endpoint": self.endpoint,
            "is_reachable": self.is_reachable,
            "response_time_ms": self.response_time_ms,
            "validator_permit": self.validator_permit,
            "status": self.status.value,
            "status_message": self.status_message,
        }

    def __str__(self) -> str:
        return f"NeuronInfo(uid={self.uid}, status={self.status.value}, hotkey={self.hotkey[:16] if self.hotkey else 'N/A'}...)"


# =============================================================================
# Main Checker Class
# =============================================================================

class NeuronChecker:
    """
    Check neuron registration, activity, and endpoint liveness.

    This class provides methods similar to Bittensor's registration checks:
    - is_hotkey_registered: Check if hotkey is on-chain
    - last_update tracking: Determine if neuron is active
    - endpoint ping: Verify miner/validator is responding

    Activity Threshold:
        By default, a neuron is considered INACTIVE if it hasn't updated
        in 600 blocks (~2 hours at 12s/block). This can be configured.

    Example:
        from sdk.neuron_checker import NeuronChecker

        checker = NeuronChecker()

        # Full registration check
        info = checker.check_registration(subnet_uid=1, hotkey="0x...")

        if info.status == NeuronStatus.ACTIVE:
            print("Neuron is active!")
        elif info.status == NeuronStatus.INACTIVE:
            print(f"Neuron inactive for {info.blocks_since_update} blocks")
        elif info.status == NeuronStatus.UNREACHABLE:
            print("Neuron endpoint not responding")

        # Quick checks
        is_registered = checker.is_registered(1, "0x...")
        is_active = checker.is_neuron_active(1, "0x...")
    """

    # Default activity threshold: 600 blocks (~2 hours)
    DEFAULT_ACTIVITY_THRESHOLD = 600

    # Default endpoint ping timeout
    DEFAULT_PING_TIMEOUT = 10.0

    # Health check paths to try
    HEALTH_PATHS = ["/health", "/", "/forward", "/api/health"]

    def __init__(
        self,
        rpc_url: str = "http://localhost:8545",
        activity_threshold: int = None,
        ping_timeout: float = None,
    ):
        """
        Initialize the checker.

        Args:
            rpc_url: LuxTensor RPC endpoint URL
            activity_threshold: Max blocks since update to be considered active
            ping_timeout: Timeout in seconds for endpoint pings
        """
        from sdk.luxtensor_client import LuxtensorClient

        self.rpc_url = rpc_url
        self.client = LuxtensorClient(url=rpc_url)
        self.activity_threshold = activity_threshold or self.DEFAULT_ACTIVITY_THRESHOLD
        self.ping_timeout = ping_timeout or self.DEFAULT_PING_TIMEOUT

        logger.info(f"NeuronChecker initialized (threshold={self.activity_threshold} blocks)")

    # =========================================================================
    # Main Check Methods
    # =========================================================================

    def check_registration(
        self,
        subnet_uid: int,
        hotkey: str,
        check_endpoint: bool = True,
    ) -> NeuronRegistrationInfo:
        """
        Perform complete registration and activity check.

        Args:
            subnet_uid: Subnet ID to check
            hotkey: Hotkey address (0x...)
            check_endpoint: Whether to ping the endpoint for liveness

        Returns:
            NeuronRegistrationInfo with all details
        """
        info = NeuronRegistrationInfo(
            hotkey=hotkey,
            subnet_uid=subnet_uid,
        )

        try:
            # Step 1: Check if registered on-chain
            is_registered = self.is_registered(subnet_uid, hotkey)
            info.is_registered = is_registered

            if not is_registered:
                info.status = NeuronStatus.NOT_REGISTERED
                info.status_message = "Hotkey not registered on this subnet"
                return info

            # Step 2: Get neuron details from metagraph
            neuron = self._get_neuron_by_hotkey(subnet_uid, hotkey)

            if neuron:
                self._populate_neuron_info(info, neuron)

            # Step 3: Get current block and calculate activity
            info.current_block = self.client.get_block_number()
            info.blocks_since_update = info.current_block - info.last_update_block

            # Step 4: Determine status based on activity
            if info.blocks_since_update > self.activity_threshold:
                info.status = NeuronStatus.INACTIVE
                hours = (info.blocks_since_update * 12) / 3600
                info.status_message = f"Inactive for {hours:.1f} hours ({info.blocks_since_update} blocks)"
            else:
                info.status = NeuronStatus.ACTIVE
                info.status_message = f"Active (last update {info.blocks_since_update} blocks ago)"

            # Step 5: Optionally check endpoint liveness
            if check_endpoint and info.endpoint:
                is_reachable, response_time = self._ping_endpoint_sync(info.endpoint)
                info.is_reachable = is_reachable
                info.response_time_ms = response_time

                if not is_reachable:
                    info.status = NeuronStatus.UNREACHABLE
                    info.status_message = f"Endpoint {info.endpoint} not responding"
            elif check_endpoint and not info.endpoint:
                info.status_message += " (no endpoint configured)"

            return info

        except Exception as e:
            logger.error(f"Error checking registration for {hotkey}: {e}")
            info.status = NeuronStatus.NOT_REGISTERED
            info.status_message = f"Error: {str(e)}"
            return info

    def is_registered(self, subnet_uid: int, hotkey: str) -> bool:
        """
        Quick check if hotkey is registered on subnet.

        Args:
            subnet_uid: Subnet ID
            hotkey: Hotkey address

        Returns:
            True if registered
        """
        try:
            return self.client.is_hotkey_registered(subnet_uid, hotkey)
        except Exception as e:
            logger.error(f"Error checking registration: {e}")
            return False

    def is_neuron_active(
        self,
        subnet_uid: int,
        hotkey: str,
        max_blocks: Optional[int] = None,
    ) -> bool:
        """
        Quick check if neuron is active (recently updated).

        Args:
            subnet_uid: Subnet ID
            hotkey: Hotkey address
            max_blocks: Custom activity threshold (uses default if None)

        Returns:
            True if active
        """
        threshold = max_blocks or self.activity_threshold

        try:
            neuron = self._get_neuron_by_hotkey(subnet_uid, hotkey)

            if not neuron:
                return False

            last_update = neuron.get("last_update", 0)
            current_block = self.client.get_block_number()
            blocks_since = current_block - last_update

            return blocks_since <= threshold

        except Exception:
            return False

    def is_endpoint_reachable(self, endpoint: str) -> bool:
        """
        Check if endpoint is reachable.

        Args:
            endpoint: HTTP endpoint URL

        Returns:
            True if reachable
        """
        is_reachable, _ = self._ping_endpoint_sync(endpoint)
        return is_reachable

    # =========================================================================
    # Batch Operations
    # =========================================================================

    def get_all_neurons_status(
        self,
        subnet_uid: int,
        check_endpoints: bool = False,
    ) -> List[NeuronRegistrationInfo]:
        """
        Get status of all neurons in a subnet.

        Args:
            subnet_uid: Subnet ID
            check_endpoints: Whether to ping all endpoints (slow)

        Returns:
            List of NeuronRegistrationInfo for each neuron
        """
        results = []

        try:
            neurons = self._get_all_neurons(subnet_uid)
            current_block = self.client.get_block_number()

            for neuron in neurons:
                info = NeuronRegistrationInfo(
                    is_registered=True,
                    subnet_uid=subnet_uid,
                    current_block=current_block,
                )

                self._populate_neuron_info(info, neuron)

                # Calculate activity
                info.blocks_since_update = current_block - info.last_update_block

                if info.blocks_since_update > self.activity_threshold:
                    info.status = NeuronStatus.INACTIVE
                else:
                    info.status = NeuronStatus.ACTIVE

                # Optionally check endpoint
                if check_endpoints and info.endpoint:
                    is_reachable, response_time = self._ping_endpoint_sync(info.endpoint)
                    info.is_reachable = is_reachable
                    info.response_time_ms = response_time

                    if not is_reachable:
                        info.status = NeuronStatus.UNREACHABLE

                results.append(info)

            return results

        except Exception as e:
            logger.error(f"Error getting neurons status: {e}")
            return []

    def get_active_miners(self, subnet_uid: int) -> List[NeuronRegistrationInfo]:
        """Get list of active miners in subnet"""
        all_neurons = self.get_all_neurons_status(subnet_uid, check_endpoints=False)
        return [
            n for n in all_neurons
            if n.status == NeuronStatus.ACTIVE and n.neuron_type == NeuronType.MINER
        ]

    def get_active_validators(self, subnet_uid: int) -> List[NeuronRegistrationInfo]:
        """Get list of active validators in subnet"""
        all_neurons = self.get_all_neurons_status(subnet_uid, check_endpoints=False)
        return [
            n for n in all_neurons
            if n.status == NeuronStatus.ACTIVE and n.validator_permit
        ]

    # =========================================================================
    # Async Methods
    # =========================================================================

    async def ping_endpoint(
        self,
        endpoint: str,
        timeout: float = None,
    ) -> Tuple[bool, float]:
        """
        Async ping endpoint to check liveness.

        Args:
            endpoint: HTTP endpoint URL
            timeout: Request timeout in seconds

        Returns:
            Tuple of (is_reachable, response_time_ms)
        """
        timeout = timeout or self.ping_timeout

        try:
            start = time.time()

            async with httpx.AsyncClient() as client:
                for path in self.HEALTH_PATHS:
                    try:
                        url = f"{endpoint.rstrip('/')}{path}"
                        response = await client.get(url, timeout=timeout)
                        response_time = (time.time() - start) * 1000

                        # Any non-5xx response means it's reachable
                        if response.status_code < 500:
                            return True, response_time
                    except httpx.TimeoutException:
                        continue
                    except Exception:
                        continue

            return False, 0

        except Exception as e:
            logger.warning(f"Ping failed for {endpoint}: {e}")
            return False, 0

    async def batch_ping_endpoints(
        self,
        endpoints: List[str],
    ) -> Dict[str, Tuple[bool, float]]:
        """
        Ping multiple endpoints concurrently.

        Args:
            endpoints: List of endpoint URLs

        Returns:
            Dict mapping endpoint -> (is_reachable, response_time_ms)
        """
        tasks = [self.ping_endpoint(ep) for ep in endpoints]
        results = await asyncio.gather(*tasks)
        return dict(zip(endpoints, results))

    # =========================================================================
    # Validator-Specific Methods
    # =========================================================================

    def check_validator_permit(self, subnet_uid: int, hotkey: str) -> bool:
        """Check if hotkey has validator permit"""
        try:
            neuron = self._get_neuron_by_hotkey(subnet_uid, hotkey)
            return neuron.get("validator_permit", False) if neuron else False
        except Exception:
            return False

    def get_validator_weights(
        self,
        subnet_uid: int,
        validator_uid: int,
    ) -> Dict[int, float]:
        """
        Get weights set by a validator.

        Args:
            subnet_uid: Subnet ID
            validator_uid: Validator's UID

        Returns:
            Dict mapping miner_uid -> weight (0-1)
        """
        try:
            weights = self.client._call_rpc(
                "weight_getWeights",
                [subnet_uid, validator_uid]
            )
            # Convert u16 to float
            return {i: w / 65535.0 for i, w in enumerate(weights) if w > 0}
        except Exception as e:
            logger.error(f"Error getting weights: {e}")
            return {}

    # =========================================================================
    # Private Helpers
    # =========================================================================

    def _get_all_neurons(self, subnet_uid: int) -> List[Dict]:
        """Get all neurons from RPC"""
        try:
            return self.client._call_rpc("neuron_getAll", [subnet_uid])
        except Exception:
            return []

    def _get_neuron_by_hotkey(
        self,
        subnet_uid: int,
        hotkey: str,
    ) -> Optional[Dict]:
        """Find neuron by hotkey"""
        neurons = self._get_all_neurons(subnet_uid)
        hotkey_lower = hotkey.lower()

        for n in neurons:
            if n.get("hotkey", "").lower() == hotkey_lower:
                return n

        return None

    def _populate_neuron_info(
        self,
        info: NeuronRegistrationInfo,
        neuron: Dict,
    ) -> None:
        """Populate NeuronRegistrationInfo from RPC response"""
        info.uid = neuron.get("uid")
        info.coldkey = neuron.get("coldkey")
        info.stake = neuron.get("stake", 0)
        info.stake_formatted = f"{info.stake / 1e18:.6f}"
        info.endpoint = neuron.get("endpoint", "")
        info.last_update_block = neuron.get("last_update", 0)
        info.is_active_on_chain = neuron.get("active", False)
        info.validator_permit = neuron.get("validator_permit", False)
        info.validator_trust = neuron.get("validator_trust", 0)
        info.trust = neuron.get("trust", 0)
        info.incentive = neuron.get("incentive", 0)
        info.emission = neuron.get("emission", 0)

        # Determine neuron type
        if info.validator_permit:
            info.neuron_type = NeuronType.VALIDATOR
        else:
            info.neuron_type = NeuronType.MINER

    def _ping_endpoint_sync(self, endpoint: str) -> Tuple[bool, float]:
        """Synchronous endpoint ping"""
        try:
            loop = asyncio.get_event_loop()
            if loop.is_running():
                # Already in async context
                import concurrent.futures
                with concurrent.futures.ThreadPoolExecutor() as pool:
                    future = pool.submit(
                        asyncio.run,
                        self.ping_endpoint(endpoint)
                    )
                    return future.result(timeout=self.ping_timeout + 5)
            else:
                return loop.run_until_complete(self.ping_endpoint(endpoint))
        except RuntimeError:
            return asyncio.run(self.ping_endpoint(endpoint))


# =============================================================================
# CLI Entry Point
# =============================================================================

def main():
    """CLI for checking neuron status"""
    import argparse
    import json

    parser = argparse.ArgumentParser(
        description="Check neuron registration and activity status",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
    # Check specific neuron
    python -m sdk.neuron_checker --subnet-uid 1 --hotkey 0x123...

    # List all neurons in subnet
    python -m sdk.neuron_checker --subnet-uid 1 --all

    # List only active miners
    python -m sdk.neuron_checker --subnet-uid 1 --active-miners

    # Ping specific endpoint
    python -m sdk.neuron_checker --ping http://miner:8091
        """,
    )

    parser.add_argument("--subnet-uid", type=int, default=1, help="Subnet UID")
    parser.add_argument("--hotkey", help="Hotkey address to check")
    parser.add_argument("--rpc", default="http://localhost:8545", help="RPC URL")
    parser.add_argument("--all", action="store_true", help="List all neurons")
    parser.add_argument("--active-miners", action="store_true", help="List active miners")
    parser.add_argument("--active-validators", action="store_true", help="List active validators")
    parser.add_argument("--ping", help="Ping specific endpoint")
    parser.add_argument("--threshold", type=int, default=600, help="Activity threshold (blocks)")

    args = parser.parse_args()

    checker = NeuronChecker(
        rpc_url=args.rpc,
        activity_threshold=args.threshold,
    )

    if args.ping:
        print(f"Pinging {args.ping}...")
        is_reachable, response_time = asyncio.run(checker.ping_endpoint(args.ping))
        print(f"  Reachable: {'✅' if is_reachable else '❌'}")
        if is_reachable:
            print(f"  Response time: {response_time:.2f}ms")

    elif args.hotkey:
        print(f"Checking registration for {args.hotkey}...")
        info = checker.check_registration(args.subnet_uid, args.hotkey)
        print(json.dumps(info.to_dict(), indent=2))

    elif args.active_miners:
        print(f"Getting active miners in subnet {args.subnet_uid}...")
        miners = checker.get_active_miners(args.subnet_uid)
        for m in miners:
            print(f"  UID {m.uid}: {m.hotkey[:20]}... stake={m.stake_formatted}")
        print(f"\nTotal: {len(miners)} active miners")

    elif args.active_validators:
        print(f"Getting active validators in subnet {args.subnet_uid}...")
        validators = checker.get_active_validators(args.subnet_uid)
        for v in validators:
            print(f"  UID {v.uid}: {v.hotkey[:20]}... permit={v.validator_permit}")
        print(f"\nTotal: {len(validators)} active validators")

    elif args.all:
        print(f"Getting all neurons in subnet {args.subnet_uid}...")
        statuses = checker.get_all_neurons_status(args.subnet_uid)

        for info in statuses:
            status_icon = {
                NeuronStatus.ACTIVE: "✅",
                NeuronStatus.INACTIVE: "⏸️",
                NeuronStatus.UNREACHABLE: "❌",
            }.get(info.status, "❓")

            print(f"  {status_icon} UID {info.uid}: {info.status.value} - {info.hotkey[:20]}...")

        # Summary
        active = sum(1 for s in statuses if s.status == NeuronStatus.ACTIVE)
        inactive = sum(1 for s in statuses if s.status == NeuronStatus.INACTIVE)
        print(f"\nTotal: {len(statuses)} neurons ({active} active, {inactive} inactive)")

    else:
        parser.print_help()


if __name__ == "__main__":
    main()
