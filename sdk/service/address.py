from sdk.config.settings import settings, logger
from sdk.compat.pycardano import (
    TransactionBuilder,
    TransactionOutput,
    Address,
    UTxO,
    PlutusV3Script,
    Redeemer,
    BlockFrostChainContext,
    TransactionId,
    VerificationKeyHash,
    ExtendedSigningKey,
    Network,
)
from typing import Optional


def get_addr(
    payment_xsk: ExtendedSigningKey,
    stake_xsk: Optional[ExtendedSigningKey],
    network: Optional[Network],
):

    resolved_network = network or settings.CARDANO_NETWORK
    if not isinstance(resolved_network, Network):
        network_setting_str = str(resolved_network).lower()
        if network_setting_str == "testnet":
            resolved_network = Network.TESTNET
        elif network_setting_str == "mainnet":
            resolved_network = Network.MAINNET
        else:
            raise ValueError(f"Invalid network value: {resolved_network}")

    pay_xvk = payment_xsk.to_verification_key()
    if stake_xsk:
        stk_xvk = stake_xsk.to_verification_key()
        owner_address = Address(
            payment_part=pay_xvk.hash(),
            staking_part=stk_xvk.hash(),
            network=resolved_network,
        )
    else:
        owner_address = Address(payment_part=pay_xvk.hash(), network=resolved_network)

    return owner_address
