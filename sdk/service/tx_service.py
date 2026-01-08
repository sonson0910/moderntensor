# sdk/service/tx_service.py

from typing import Optional

from sdk.compat.luxtensor_types import (
    Address,
    ExtendedSigningKey,
    Network,
    TransactionBuilder,
    TransactionOutput,
    TransactionBody,
    Transaction,
    TransactionWitnessSet,
    Value,
    VerificationKeyWitness,
    MultiAsset,
    Asset,
    BlockFrostChainContext,
)

from sdk.config.settings import settings, logger
from sdk.keymanager.decryption_utils import decode_hotkey_skey


def send_ada(
    chain_context,
    payment_xsk: ExtendedSigningKey,
    stake_xsk: Optional[ExtendedSigningKey] = None,
    to_address_str: str = "",
    lovelace_amount: int = 1_000_000,
    network: Network = None,  # type: ignore
    change_address_str: Optional[str] = None,
) -> str:
    """
    Sends ADA (lovelace) to a specified address using the given signing key(s).

    Steps:
      1) Construct the source address (from_address) using payment_xsk (and stake_xsk if any).
      2) Build a transaction builder, adding from_address as an input.
      3) Create a transaction output sending `lovelace_amount` to the to_address.
      4) Sign the transaction (with payment_xsk) and optionally stake_xsk if needed.
      5) Submit to the network; return the resulting TX ID.

    Args:
        chain_context: The chain context (e.g. BlockFrostChainContext) used for building/submitting.
        payment_xsk (ExtendedSigningKey): A payment extended signing key for input signing.
        stake_xsk (ExtendedSigningKey, optional): A stake extended signing key to include stake key hash.
        to_address_str (str): The destination address in bech32/base58. Defaults to from_address if blank.
        lovelace_amount (int): The ADA amount in lovelace (1_000_000 = 1 ADA). Default is 1_000_000.
        network (Network, optional): The Cardano network; defaults to settings.CARDANO_NETWORK if None.
        change_address_str (str, optional): If provided, leftover change goes here; else goes to from_address.

    Returns:
        str: The transaction ID (tx_id) upon successful submission.
    """
    # Ensure network is a Network object
    resolved_network: Network
    if network is not None:
        resolved_network = network
    elif isinstance(settings.CARDANO_NETWORK, Network):  # Check if already converted
        resolved_network = settings.CARDANO_NETWORK
    elif isinstance(settings.CARDANO_NETWORK, str):
        # Manually convert based on string value (handle potential invalid strings)
        net_str = settings.CARDANO_NETWORK.upper()
        if net_str == "MAINNET":
            resolved_network = Network.MAINNET
        elif net_str == "TESTNET":
            resolved_network = Network.TESTNET
        else:
            logger.warning(
                f"Invalid CARDANO_NETWORK string '{settings.CARDANO_NETWORK}'. Defaulting to TESTNET."
            )
            resolved_network = Network.TESTNET
    else:
        # Fallback if settings value is unexpected type
        logger.warning(
            f"Unexpected type for CARDANO_NETWORK: {type(settings.CARDANO_NETWORK)}. Defaulting to TESTNET."
        )
        resolved_network = Network.TESTNET

    pay_xvk = payment_xsk.to_verification_key()
    if stake_xsk:
        stk_xvk = stake_xsk.to_verification_key()
        from_address = Address(
            payment_part=pay_xvk.hash(),
            staking_part=stk_xvk.hash(),
            network=resolved_network,
        )
    else:
        from_address = Address(payment_part=pay_xvk.hash(), network=resolved_network)

    # Determine destination (default to from_address if blank)
    to_address = (
        Address.from_primitive(to_address_str) if to_address_str else from_address
    )
    # Determine change address
    change_address = (
        Address.from_primitive(change_address_str)
        if change_address_str
        else from_address
    )

    # Build transaction
    builder = TransactionBuilder(chain_context)
    builder.add_input_address(from_address)
    builder.add_output(TransactionOutput(to_address, lovelace_amount))  # type: ignore

    # Sign transaction (with payment_xsk only)
    signed_tx = builder.build_and_sign(
        signing_keys=[payment_xsk], change_address=change_address
    )

    # Submit transaction
    tx_id = chain_context.submit_tx(signed_tx.to_cbor())
    logger.info(f"[send_ada] => TX ID: {tx_id}")
    return tx_id


def send_token(
    chain_context: BlockFrostChainContext,
    base_dir: str,
    coldkey_name: str,
    hotkey_name: str,
    password: str,
    to_address_str: str,
    policy_id_hex: str,
    asset_name: str,
    token_amount: int,
    fee: int = 200_000,
    network: Network = None,  # type: ignore
) -> str:
    """
    Sends a specified native token (policy_id + asset_name) plus a small amount of ADA (e.g. 2 ADA)
    to `to_address_str`, with leftover tokens and ADA returned to from_address as change.

    Steps:
      1) Decode the hotkey to retrieve (payment_xsk, stake_xsk).
      2) Fetch all UTxOs from from_address to sum coin and multi-asset tokens.
      3) Verify enough token balance for `token_amount`.
      4) Construct output #1 with ~2 ADA + `token_amount`.
      5) Calculate leftover coin and tokens => second output as change.
      6) Build and sign the transaction, then submit.
      7) Return the TX ID.

    Args:
        chain_context (BlockFrostChainContext): The chain context for building/submitting tx.
        base_dir (str): Base directory with coldkeys/hotkeys data.
        coldkey_name (str): Folder containing the relevant mnemonic, salt, etc.
        hotkey_name (str): Identifier for the hotkey in hotkeys.json.
        password (str): The password for decrypting the hotkey.
        to_address_str (str): The target address in bech32/base58.
        policy_id_hex (str): Hex policy ID for the native token.
        asset_name (str): The name of the token within that policy.
        token_amount (int): Number of tokens to send.
        fee (int): Tx fee in lovelace. Defaults to 200,000 (0.2 ADA).
        network (Network, optional): Defaults to settings.CARDANO_NETWORK if None.

    Returns:
        str: The transaction ID if successfully submitted.
    """

    # Ensure network is a Network object
    resolved_network: Network
    if network is not None:
        resolved_network = network
    elif isinstance(settings.CARDANO_NETWORK, Network):  # Check if already converted
        resolved_network = settings.CARDANO_NETWORK
    elif isinstance(settings.CARDANO_NETWORK, str):
        # Manually convert based on string value
        net_str = settings.CARDANO_NETWORK.upper()
        if net_str == "MAINNET":
            resolved_network = Network.MAINNET
        elif net_str == "TESTNET":
            resolved_network = Network.TESTNET
        else:
            logger.warning(
                f"Invalid CARDANO_NETWORK string '{settings.CARDANO_NETWORK}'. Defaulting to TESTNET."
            )
            resolved_network = Network.TESTNET
    else:
        logger.warning(
            f"Unexpected type for CARDANO_NETWORK: {type(settings.CARDANO_NETWORK)}. Defaulting to TESTNET."
        )
        resolved_network = Network.TESTNET

    # 1) Decode the hotkey
    payment_xsk, stake_xsk = decode_hotkey_skey(
        base_dir, coldkey_name, hotkey_name, password
    )
    if not payment_xsk:
        raise ValueError(f"Failed to decode Payment XSK for hotkey '{hotkey_name}'")

    pay_xvk = payment_xsk.to_verification_key()  # type: ignore
    stake_xvk = stake_xsk.to_verification_key() if stake_xsk else None  # type: ignore

    # Construct from_address
    from_address = Address(
        payment_part=pay_xvk.hash(),
        staking_part=stake_xvk.hash() if stake_xvk else None,
        network=resolved_network,  # Use resolved_network
    )
    logger.info(f"[send_token] from_address={from_address}")

    # 2) Fetch UTxOs
    utxos = chain_context.utxos(from_address)
    if not utxos:
        raise ValueError(f"No UTxOs found at {from_address}")

    tx_inputs = []
    total_coin_input = 0

    input_multiasset = MultiAsset()

    for utxo in utxos:
        tx_inputs.append(utxo.input)
        val = utxo.output.amount
        total_coin_input += val.coin

        if val.multi_asset:
            for pid, assets_map in val.multi_asset.items():
                if pid not in input_multiasset:
                    input_multiasset[pid] = Asset()
                for aname, qty in assets_map.items():
                    if aname not in input_multiasset[pid]:
                        input_multiasset[pid][aname] = 0
                    input_multiasset[pid][aname] += qty

    # 3) Ensure enough tokens
    pid_bytes = bytes.fromhex(policy_id_hex)
    aname_bytes = asset_name.encode("utf-8")

    from sdk.compat.luxtensor_types import ScriptHash, AssetName

    policy_obj = ScriptHash.from_primitive(pid_bytes)
    asset_obj = AssetName(aname_bytes)

    total_token_input = input_multiasset.get(policy_obj, Asset()).get(asset_obj, 0)
    if total_token_input < token_amount:
        raise ValueError(
            f"Not enough '{asset_name}' tokens under policy={policy_id_hex}. "
            f"Needed: {token_amount}, found: {total_token_input}"
        )

    # 4) Prepare ~2 ADA for output #1 + token_amount
    out1_coin = 2_000_000
    if total_coin_input < fee + out1_coin:
        raise ValueError("Insufficient coin to cover fee + 2 ADA output.")

    # Build first output with token_amount + out1_coin
    ma_value_out1 = [out1_coin, {pid_bytes: {aname_bytes: token_amount}}]
    val_out1 = Value.from_primitive(ma_value_out1)
    out_token = TransactionOutput(Address.from_primitive(to_address_str), val_out1)

    # 5) Calculate leftover coin after fee + out1_coin
    coin_change = total_coin_input - fee - out1_coin
    if coin_change < 0:
        raise ValueError("Negative coin change => insufficient funds.")

    # Leftover tokens = total_token_input - token_amount
    token_change = total_token_input - token_amount

    # Clone input_multiasset and subtract token_amount
    out_change_multiasset = MultiAsset()
    for pid, assets_map in input_multiasset.items():
        out_change_multiasset[pid] = Asset()
        for aname, qty in assets_map.items():
            out_change_multiasset[pid][aname] = qty

    out_change_multiasset[policy_obj][asset_obj] -= token_amount

    # Cleanup if zero
    if out_change_multiasset[policy_obj][asset_obj] == 0:
        del out_change_multiasset[policy_obj][asset_obj]
    if len(out_change_multiasset[policy_obj]) == 0:
        del out_change_multiasset[policy_obj]

    # 6) Build the change output
    val_change = Value(coin_change, out_change_multiasset)
    out_change = TransactionOutput(from_address, val_change)

    # 7) Build transaction body
    tx_body = TransactionBody(
        inputs=tx_inputs, outputs=[out_token, out_change], fee=fee
    )

    # 8) Sign
    signature = payment_xsk.sign(tx_body.hash())  # type: ignore
    vk_witness = VerificationKeyWitness(pay_xvk, signature)
    witness_set = TransactionWitnessSet(vkey_witnesses=[vk_witness])

    # 9) Construct & submit
    signed_tx = Transaction(tx_body, witness_set)
    tx_id = chain_context.submit_tx(signed_tx.to_cbor())

    logger.info(f"[send_token] => TX ID: {tx_id}")
    return tx_id
