from typing import Optional
from sdk.compat.luxtensor_types import (
    ExtendedSigningKey,
    ScriptHash,
    Address,
    PlutusV3Script,
    BlockFrostChainContext,
    Network,
    PlutusData,
    Redeemer,
    UTxO,
)
from sdk.service.utxos import get_utxo_with_lowest_performance
from sdk.metagraph.update_metagraph import update_datum
from sdk.metagraph.metagraph_datum import MinerDatum
from sdk.config.settings import settings


def register_key(
    payment_xsk: ExtendedSigningKey,
    stake_xsk: Optional[ExtendedSigningKey],
    script_hash: ScriptHash,
    new_datum: PlutusData,
    script: PlutusV3Script,
    context: BlockFrostChainContext,
    network: Optional[Network] = None,
    contract_address: Optional[Address] = None,
    redeemer: Optional[Redeemer] = None,
) -> str:
    """
    Service to register a new key by updating the UTxO with the lowest incentive.

    This service identifies the UTxO with the lowest incentive from the smart contract, then uses
    the update_datum function to update that UTxO with the provided new datum.

    Args:
        payment_xsk (ExtendedSigningKey): The user's payment signing key.
        stake_xsk (Optional[ExtendedSigningKey]): The user's stake signing key (optional).
        script_hash (ScriptHash): The hash of the Plutus script (smart contract).
        new_datum (PlutusData): The new datum to attach to the UTxO.
        script (PlutusV3Script): The Plutus script of the smart contract.
        context (BlockFrostChainContext): Blockchain context for interacting with the Cardano network.
        network (Optional[Network]): The Cardano network (mainnet or testnet). Defaults to settings.CARDANO_NETWORK.
        contract_address (Optional[Address]): The address object of the smart contract.
                                            Defaults to an Address derived from settings.TEST_CONTRACT_ADDRESS (ScriptHash).
        redeemer (Optional[Redeemer]): The redeemer used for script validation. Defaults to Redeemer(0).

    Returns:
        str: The transaction ID of the submitted transaction.

    Raises:
        ValueError: If no UTxO is found at the contract address.
    """
    # Determine the network, ensuring it's not None
    resolved_network: Network
    if network:
        resolved_network = network
    elif settings.CARDANO_NETWORK:
        network_setting_str = str(settings.CARDANO_NETWORK).lower()
        if network_setting_str == "testnet":
            resolved_network = Network.TESTNET
        elif network_setting_str == "mainnet":
            resolved_network = Network.MAINNET
        else:
            raise ValueError(
                f"Invalid network string in settings: '{settings.CARDANO_NETWORK}'"
            )
    else:
        raise ValueError(
            "Cardano network could not be determined from arguments or settings."
        )

    # Determine the final contract address object
    final_contract_address: Address
    if contract_address:
        final_contract_address = contract_address
    else:
        default_script_hash_hex = settings.TEST_CONTRACT_ADDRESS
        if not default_script_hash_hex:
            raise ValueError(
                "Default contract address (script hash) not set in settings."
            )
        default_script_hash = ScriptHash(bytes.fromhex(default_script_hash_hex))
        # Use resolved_network which is guaranteed to be a Network object
        final_contract_address = Address(
            payment_part=default_script_hash, network=resolved_network
        )

    # Find the UTxO with the lowest incentive (can be None)
    lowest_utxo: Optional[UTxO] = get_utxo_with_lowest_performance(
        contract_address=final_contract_address,
        datumclass=MinerDatum,  # Assumes MinerDatum is the correct datum class
        context=context,
    )
    if not lowest_utxo:
        raise ValueError("No UTxO found at the contract address.")

    # Update the UTxO with the new datum using the update_datum function
    tx_id = update_datum(
        payment_xsk=payment_xsk,
        stake_xsk=stake_xsk,
        script_hash=script_hash,
        utxo=lowest_utxo,
        new_datum=new_datum,
        script=script,
        context=context,
        network=resolved_network,
        redeemer=redeemer or Redeemer(0),
    )

    return tx_id
