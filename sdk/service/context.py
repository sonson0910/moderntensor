# sdk/service/context.py

from sdk.blockchain import L1ChainContext, L1Network
from sdk.config.settings import settings, logger


def get_chain_context(method="l1_rpc"):
    """
    Returns a chain context object for interacting with the Layer 1 blockchain.

    The Layer 1 blockchain uses JSON-RPC instead of Cardano's BlockFrost API.
    This function creates an L1ChainContext that connects to the appropriate
    network (mainnet, testnet, or devnet).

    **DEPRECATION NOTICE:**
    This function returns a placeholder L1ChainContext. For actual blockchain
    interaction with Luxtensor, use `LuxtensorClient` directly:
    
        from sdk.luxtensor_client import LuxtensorClient
        client = LuxtensorClient("http://localhost:9944")
        balance = client.get_balance(address)

    Args:
        method (str): The name of the method to use for chain context creation.
                      Default is "l1_rpc" (Layer 1 JSON-RPC).
                      "blockfrost" is kept for backward compatibility only.

    Raises:
        ValueError: If an unsupported method is specified.

    Returns:
        L1ChainContext: Chain context configured for the specified network.
                        This is a placeholder - use LuxtensorClient for real operations.
    """
    if method == "l1_rpc" or method == "blockfrost":  # blockfrost for backward compat
        # Get network from settings
        network_str = getattr(settings, 'NETWORK', 'testnet')
        
        if network_str.upper() == "MAINNET":
            network = L1Network.MAINNET
        elif network_str.upper() == "TESTNET":
            network = L1Network.TESTNET
        else:
            network = L1Network.DEVNET
        
        # Get RPC URL from settings or use default
        rpc_url = getattr(settings, 'L1_RPC_URL', None)
        api_key = getattr(settings, 'L1_API_KEY', None)

        logger.info(
            f"Initializing L1ChainContext with network={network}, rpc_url={rpc_url}"
        )
        
        return L1ChainContext(
            rpc_url=rpc_url,
            network=network,
            api_key=api_key
        )
    else:
        raise ValueError(f"Unsupported chain context method: {method}")

