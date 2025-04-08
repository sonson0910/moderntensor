# sdk/service/context.py

from pycardano import BlockFrostChainContext, Network
from sdk.config.settings import settings, logger


def get_chain_context(method="blockfrost"):
    """
    Returns a chain context object for interacting with the Cardano blockchain.

    Currently, only the "blockfrost" method is supported, which uses the 
    BlockFrostChainContext class from pycardano. The function is designed 
    to be extensible, so additional methods could be implemented in the future.

    Args:
        method (str): The name of the method to use for chain context creation.
                      Default is "blockfrost".
        project_id (str): The Blockfrost project ID (API key). 
                          Required if method is "blockfrost".
        network (Network): The Cardano network to connect to 
                           (MAINNET or TESTNET). Default is TESTNET.

    Raises:
        ValueError: If an unsupported method is specified.

    Returns:
        BlockFrostChainContext: If method is "blockfrost", returns a context configured
                                for the specified network.
    """
    # For now, we only support using Blockfrost
    if method == "blockfrost":
        project_id = settings.BLOCKFROST_PROJECT_ID
        network = "TESTNET"  # This is a pycardano.Network enum

        # Determine the base URL depending on the network
        if network == "MAINET":
            base_url = "https://cardano-mainnet.blockfrost.io/api/"
        else:
            base_url = "https://cardano-preprod.blockfrost.io/api/"

        logger.info(f"Initializing BlockFrostChainContext with network={network}, project_id={project_id}")
        return BlockFrostChainContext(project_id=project_id, network=Network.TESTNET, base_url=base_url)
    else:
        raise ValueError(f"Unsupported chain context method: {method}")
