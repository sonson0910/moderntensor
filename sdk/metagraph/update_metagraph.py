from typing import Optional

from sdk.compat.luxtensor_types import (
    TransactionBuilder,
    TransactionOutput,
    Address,
    UTxO,
    PlutusData,
    Redeemer,
    ScriptHash,
    Network,
    BlockFrostChainContext,
    ExtendedSigningKey,
    PlutusV3Script,
    PaymentExtendedVerificationKey,
)

from sdk.config.settings import settings  # Configuration for Cardano network settings

def update_datum(
    payment_xsk: ExtendedSigningKey,
    stake_xsk: Optional[ExtendedSigningKey],
    script_hash: ScriptHash,
    utxo: UTxO,
    new_datum: PlutusData,
    script: PlutusV3Script,
    context: BlockFrostChainContext,
    network: Optional[Network] = None,
    redeemer: Optional[Redeemer] = None,
) -> str:
    """
    Updates the datum of a UTxO locked in a Plutus smart contract on the Cardano blockchain.

    This function constructs and submits a transaction that consumes an existing UTxO from a Plutus script,
    updates its datum with the provided `new_datum`, and locks the same amount of lovelace back into the
    contract. The transaction is signed using the provided payment signing key, and any change (e.g., from
    transaction fees) is returned to the owner's address.

    Args:
        payment_xsk (ExtendedSigningKey): The extended signing key for the payment part of the transaction.
        stake_xsk (Optional[ExtendedSigningKey]): The extended signing key for staking, if applicable.
            If provided, the owner's address will include a staking part.
        into (ScriptHash): The hash of the Plutus script (smart contract) where the UTxO is locked.
        utxo (UTxO): The Unspent Transaction Output containing the current datum to be updated.
        new_datum (PlutusData): The new datum to attach to the UTxO.
        script (PlutusV3Script): The Plutus script associated with the smart contract.
        context (BlockFrostChainContext): The blockchain context (e.g., BlockFrost API) for interacting
            with the Cardano network.
        network (Optional[Network]): The Cardano network to operate on (e.g., Network.TESTNET or Network.MAINNET).
            Defaults to `settings.CARDANO_NETWORK` if not specified.
        redeemer (Optional[Redeemer]): The redeemer required for script validation. Defaults to a simple
            `HelloWorldRedeemer` if not provided.

    Returns:
        str: The transaction ID of the submitted transaction.

    Raises:
        ValueError: If critical inputs (e.g., `payment_xsk`, `utxo`, or `context`) are invalid or missing.
        Exception: If the transaction fails to build or submit due to network issues or invalid data.
    """
    # Default to configured network if not provided
    network = network or settings.CARDANO_NETWORK # type: ignore

    # Derive payment verification key from signing key
    pay_xvk = PaymentExtendedVerificationKey.from_signing_key(payment_xsk)

    # Construct owner's address, including staking part if stake_xsk is provided
    if stake_xsk:
        stk_xvk = stake_xsk.to_verification_key()
        owner_address = Address(
            payment_part=pay_xvk.hash(),
            staking_part=stk_xvk.hash(),
            network=network, # type: ignore
        )
    else:
        owner_address = Address(
            payment_part=pay_xvk.hash(),
            network=network, # type: ignore
        )

    # Define the owner as the hash of the payment verification key
    owner = pay_xvk.hash()

    # Create the contract address using the provided script hash
    contract_address = Address(
        payment_part=script_hash,
        network=network, # type: ignore
    )

    # Use provided redeemer or default to a simple HelloWorldRedeemer
    redeemer = redeemer or Redeemer(0)

    # Initialize the transaction builder with the blockchain context
    builder = TransactionBuilder(context=context)

    # Add the script input to consume the UTxO from the contract
    builder.add_script_input(
        utxo=utxo,
        script=script,
        redeemer=redeemer,
    )

    # Add an input from the owner's address to cover transaction fees
    builder.add_input_address(owner_address)

    # Specify that the owner's signature is required
    builder.required_signers = [owner]

    # Add an output back to the contract with the same amount and updated datum
    builder.add_output(
        TransactionOutput(
            address=contract_address,
            amount=utxo.output.amount.coin,  # Preserve the original amount # type: ignore
            datum=new_datum,                # Attach the new datum
        )
    )

    # Build and sign the transaction, sending any change back to the owner
    signed_tx = builder.build_and_sign(
        signing_keys=[payment_xsk],
        change_address=owner_address,
    )

    # Submit the transaction to the blockchain and return its ID
    tx_id = context.submit_tx(signed_tx)
    return tx_id
