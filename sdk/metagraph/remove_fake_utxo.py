from typing import List, Optional

from sdk.compat.luxtensor_types import (
    TransactionBuilder,
    TransactionOutput,
    Address,
    UTxO,
    PlutusV3Script,
    Redeemer,
    BlockFrostChainContext,
    TransactionId,
    ExtendedSigningKey,
    Network,
    VerificationKeyHash,
)

from sdk.config.settings import settings  # Configuration for Cardano network


def remove_fake_utxos(
    payment_xsk: ExtendedSigningKey,
    stake_xsk: Optional[ExtendedSigningKey],
    fake_utxos: List[UTxO],
    script: PlutusV3Script,
    network: Network,
    context: BlockFrostChainContext,
) -> TransactionId:
    """
    Removes fake UTxOs from a Plutus smart contract on the Cardano blockchain.

    This function constructs and submits a transaction that consumes the specified fake UTxOs from a Plutus
    script and returns the total amount of lovelace (ADA in smallest unit) to the owner's address. The
    transaction is signed using the provided payment signing key, and any change (e.g., after accounting
    for fees) is returned to the owner's address.

    Args:
        payment_xsk (ExtendedSigningKey): The extended signing key used for the payment part of the transaction.
        stake_xsk (Optional[ExtendedSigningKey]): The extended signing key for staking, if applicable.
            If provided, the owner's address will include a staking component; otherwise, it will be omitted.
        fake_utxos (List[UTxO]): A list of fake UTxOs to be removed from the smart contract.
        script (PlutusV3Script): The Plutus script associated with the smart contract.
        network (Optional[Network]): The Cardano network to operate on (e.g., Network.TESTNET or Network.MAINNET).
            Defaults to `settings.CARDANO_NETWORK` if not provided.
        context (Optional[BlockFrostChainContext]): The blockchain context (e.g., BlockFrost API) for interacting
            with the Cardano network. Defaults to a function like `get_chain_context()` if not provided.

    Returns:
        TransactionId: The ID of the transaction submitted to the blockchain.

    Raises:
        ValueError: If essential inputs (e.g., `payment_xsk`, `fake_utxos`, or `script`) are invalid or missing.
        Exception: If the transaction fails to build or submit due to network issues or invalid data.
    """
    # Set the network, defaulting to settings.CARDANO_NETWORK if not provided
    network = network or settings.CARDANO_NETWORK

    # Derive the payment verification key from the payment signing key
    pay_xvk = payment_xsk.to_verification_key()

    # Create the owner's address, including the staking part if stake_xsk is provided
    if stake_xsk:
        stk_xvk = stake_xsk.to_verification_key()
        owner_address = Address(
            payment_part=pay_xvk.hash(),
            staking_part=stk_xvk.hash(),
            network=network,
        )
    else:
        owner_address = Address(
            payment_part=pay_xvk.hash(),
            network=network,
        )

    # Define the owner as the hash of the payment verification key
    owner: VerificationKeyHash = pay_xvk.hash()

    # Initialize the TransactionBuilder with the blockchain context
    builder = TransactionBuilder(context=context)

    # Add each fake UTxO as a script input to be consumed
    for utxo in fake_utxos:
        # Use a simple redeemer (Redeemer(0)) for demonstration purposes
        # In practice, the redeemer should match the script's requirements
        redeemer = Redeemer(0)
        builder.add_script_input(
            utxo=utxo,
            script=script,
            redeemer=redeemer,
        )

    # Add an input from the owner's address to cover transaction fees if needed
    builder.add_input_address(owner_address)

    # Calculate the total amount of lovelace from the fake UTxOs
    total_amount = sum(utxo.output.amount.coin for utxo in fake_utxos)

    # Add an output to return the total lovelace to the owner's address
    builder.add_output(
        TransactionOutput(
            address=owner_address,
            amount=total_amount, # type: ignore
        )
    )

    # Specify that the owner's signature is required
    builder.required_signers = [owner]

    # Build and sign the transaction, sending any change back to the owner's address
    signed_tx = builder.build_and_sign(
        signing_keys=[payment_xsk],
        change_address=owner_address,
    )

    # Submit the transaction to the blockchain and return its ID
    tx_id = context.submit_tx(signed_tx)
    return tx_id
