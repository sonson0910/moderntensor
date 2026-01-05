import logging
import json
import os
from typing import Dict, Any, Optional
from sdk.compat.pycardano import (
    BlockFrostChainContext,
    TransactionBuilder,
    TransactionOutput,
    Value,
    Address,
    PlutusV3Script,
    Redeemer,
    ExtendedSigningKey,
    Network,
)

logger = logging.getLogger(__name__)


class SubnetManager:
    """
    Manages the creation and registration of dynamic subnets.
    """

    def __init__(self, context: BlockFrostChainContext, network: Network):
        self.context = context
        self.network = network

    def create_subnet(
        self,
        signing_key: ExtendedSigningKey,
        subnet_name: str,
        subnet_metadata: Dict[str, Any],
        initial_stake: int = 100_000_000,  # 100 ADA
        dynamic_script_path: str = "sdk/smartcontract/dynamic_datum_subnet.json",
    ) -> Optional[str]:
        """
        Creates a new subnet by deploying a transaction with the dynamic subnet script.

        Args:
            signing_key: The signing key of the creator.
            subnet_name: Name of the subnet.
            subnet_metadata: Metadata for the subnet (e.g., description, task_type).
            initial_stake: Initial ADA stake for the subnet pool.
            dynamic_script_path: Path to the Plutus script JSON.

        Returns:
            str: The transaction ID of the creation tx, or None if failed.
        """
        try:
            # 1. Load Dynamic Script
            if not os.path.exists(dynamic_script_path):
                logger.error(
                    f"Dynamic subnet script not found at {dynamic_script_path}"
                )
                return None

            with open(dynamic_script_path, "r") as f:
                script_data = json.load(f)

            script_cbor = script_data["validators"][0]["compiledCode"]
            script_bytes = bytes.fromhex(script_cbor)
            plutus_script = PlutusV3Script(script_bytes)
            script_hash = plutus_script.hash()

            contract_address = Address(payment_part=script_hash, network=self.network)
            logger.info(
                f"Creating Subnet '{subnet_name}' at address: {contract_address}"
            )

            # 2. Prepare Owner Address
            owner_vkey = signing_key.to_verification_key()
            owner_address = Address(
                payment_part=owner_vkey.hash(), network=self.network
            )

            # 3. Build Transaction
            builder = TransactionBuilder(context=self.context)
            builder.add_input_address(owner_address)

            # Create Subnet Datum (This needs to match the on-chain Datum structure)
            # For now, we use a placeholder or a generic dictionary if supported
            # In reality, this must be a PlutusData object matching the script's Datum
            from sdk.compat.pycardano import PlutusData, IndefiniteList

            # Mock Datum: [subnet_name_bytes, owner_pkh, metadata_hash]
            # This is highly dependent on the actual Plutus script logic
            subnet_datum = PlutusData()  # Placeholder

            # Output to the script address with initial stake
            builder.add_output(
                TransactionOutput(
                    address=contract_address,
                    amount=Value(coin=initial_stake),
                    datum=subnet_datum,
                )
            )

            # 4. Sign and Submit
            signed_tx = builder.build_and_sign(
                signing_keys=[signing_key], change_address=owner_address
            )

            self.context.submit_tx(signed_tx)
            tx_id = str(signed_tx.id)
            logger.info(f"Subnet creation transaction submitted! TxID: {tx_id}")
            return tx_id

        except Exception as e:
            logger.error(f"Failed to create subnet: {e}")
            return None
