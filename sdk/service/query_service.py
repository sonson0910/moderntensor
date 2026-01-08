# sdk/node/cardano_service/query_service.py

from typing import Dict
from sdk.compat.luxtensor_types import Address, Value

from sdk.config.settings import logger


def get_address_info(address_str: str, chain_context) -> Dict:
    """
    Retrieve information about a Cardano address, including:
      - The total amount of lovelace (ADA) held at the address.
      - The balances of any additional native tokens.
      - The total number of UTxOs (unspent transaction outputs).

    Args:
        address_str (str): A bech32 or base58-encoded Cardano address.
        chain_context: A chain context, such as a BlockFrostChainContext or OgmiOSChainContext,
                       which provides methods to query UTxOs.

    Returns:
        Dict: A dictionary containing:
              {
                "address": <the original address_str>,
                "lovelace": <total ADA in lovelace>,
                "tokens": {
                    (policy_id, asset_name): amount,
                    ...
                },
                "utxo_count": <number_of_utxos>
              }
    """
    # Convert the address string into a pycardano Address object
    address = Address.from_primitive(address_str)

    # Query the UTxOs for the address using the provided chain_context
    utxos = chain_context.utxos(address)

    # Initialize counters for total lovelace and token balances
    total_lovelace = 0
    token_balances = {}

    # Loop through each UTxO to aggregate lovelace and multi-asset tokens
    for utxo in utxos:
        val: Value = utxo.output.amount  # Represents coin + multi-asset
        # Sum up the lovelace (ADA) amount
        total_lovelace += val.coin

        # If this UTxO has multi-asset tokens, aggregate them into token_balances
        if val.multi_asset:
            for policy, asset_map in val.multi_asset.items():
                for asset_name, amt in asset_map.items():
                    key = (policy, asset_name)
                    token_balances[key] = token_balances.get(key, 0) + amt

    # Prepare a dictionary with the summarized information
    result = {
        "address": address_str,
        "lovelace": total_lovelace,
        "tokens": token_balances,
        "utxo_count": len(utxos),
    }

    # Log the result for debugging or informational purposes
    logger.info(f"[get_address_info] {result}")

    return result
