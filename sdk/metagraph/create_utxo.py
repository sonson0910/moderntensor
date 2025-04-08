from pycardano import (
    TransactionBuilder,
    TransactionOutput,
    Address,
    ScriptHash,
    PlutusData,
    BlockFrostChainContext,
    Network,
    TransactionId,
    ExtendedSigningKey,
)
from sdk.config.settings import settings

def create_utxo(
    payment_xsk: ExtendedSigningKey,
    stake_xsk: ExtendedSigningKey,
    amount: int,
    script_hash: ScriptHash,
    datum: PlutusData,
    context: BlockFrostChainContext,
    network: Network = None,
) -> TransactionId:
    """
    Creates a UTxO with the specified amount of lovelace and datum locked into a Plutus smart contract.

    This function builds and submits a transaction that locks a given amount of lovelace into a Plutus script
    address, attaching the provided datum. The transaction is signed with the provided payment signing key,
    and any change is returned to the owner's address.

    Args:
        payment_xsk (ExtendedSigningKey): The extended signing key for payment.
        stake_xsk (ExtendedSigningKey): The extended signing key for staking (optional).
        amount (int): The amount of lovelace to lock into the UTxO (e.g., 2_000_000 for 2 tADA).
        script_hash (ScriptHash): The hash of the Plutus script (smart contract).
        datum (PlutusData): The datum (data) to attach to the UTxO.
        context (BlockFrostChainContext): The blockchain context for interacting with the Cardano network.
        network (Network, optional): The Cardano network to use (e.g., Network.TESTNET or Network.MAINNET).
                                     Defaults to settings.CARDANO_NETWORK if not provided.

    Returns:
        TransactionId: The ID of the transaction submitted to the blockchain.
    """
    # Set the network, defaulting to settings.CARDANO_NETWORK if not provided
    network = network or settings.CARDANO_NETWORK

    # Derive the payment verification key from the signing key
    pay_xvk = payment_xsk.to_verification_key()

    # Create the owner's address, including staking part if stake_xsk is provided
    if stake_xsk:
        stk_xvk = stake_xsk.to_verification_key()
        print(pay_xvk.hash())
        print(stk_xvk.hash())

        owner_address = Address(
            payment_part=pay_xvk.hash(),
            staking_part=stk_xvk.hash(),
            network=network,
        )
        print(owner_address)
    else:
        owner_address = Address(
            payment_part=pay_xvk.hash(),
            network=network,
        )
    print(f"Owner Address: {owner_address}")

    # Create the contract address using the provided script hash
    contract_address = Address(
        payment_part=script_hash,
        network=network,
    )
    print(contract_address)

    # Initialize the transaction builder with the blockchain context
    builder = TransactionBuilder(context=context)

    # Add an input from the owner's address to fund the transaction
    builder.add_input_address(owner_address)

    # Add an output to the contract address with the specified amount and datum
    builder.add_output(
        TransactionOutput(
            address=contract_address,
            amount=amount,
            datum=datum,
        )
    )

    # Build and sign the transaction, sending change back to the owner
    signed_tx = builder.build_and_sign(
        signing_keys=[payment_xsk],
        change_address=owner_address,
    )

    # Submit the transaction to the blockchain and return the transaction ID
    tx_id = context.submit_tx(signed_tx)
    return tx_id