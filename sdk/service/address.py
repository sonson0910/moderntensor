from sdk.config.settings import settings, logger
from pycardano import (
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
    Network
)

def get_addr(payment_xsk: ExtendedSigningKey, stake_xsk: ExtendedSigningKey, network: Network):
    
    network = network or settings.CARDANO_NETWORK

    pay_xvk = payment_xsk.to_verification_key()
    if stake_xsk:
        stk_xvk = stake_xsk.to_verification_key()
        owner_address = Address(payment_part=pay_xvk.hash(), staking_part=stk_xvk.hash(), network=network)
    else:
        owner_address = Address(payment_part=pay_xvk.hash(), network=network)

    return owner_address