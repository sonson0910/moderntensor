import sys
import logging
from typing import Optional, cast, Any, List, Union
from pycardano import (
    StakeCredential,
    StakeRegistration,
    StakeDelegation,
    PoolKeyHash,
    TransactionBuilder,
    Transaction,
    TransactionBody,
    TransactionWitnessSet,
    Network,
    Address,
    Withdrawals,
    ExtendedSigningKey,
    VerificationKeyWitness,
    SigningKey,
    VerificationKeyHash,
    ScriptHash,
)
from pycardano.crypto.bech32 import bech32_decode
from blockfrost import ApiError, ApiUrls, BlockFrostApi, BlockFrostIPFS
from sdk.config.settings import settings
from sdk.keymanager.decryption_utils import decode_hotkey_skey
from sdk.service.context import get_chain_context

logger = logging.getLogger(__name__)


class Wallet:
    def __init__(self, coldkey_name, hotkey_name, password):
        """
        Khởi tạo ví bằng cách giải mã khóa hotkey.
        """
        # Resolve network setting to enum
        network_setting = settings.CARDANO_NETWORK
        if isinstance(network_setting, str):
            if network_setting.lower() == "testnet":
                self.network = Network.TESTNET
            elif network_setting.lower() == "mainnet":
                self.network = Network.MAINNET
            else:
                raise ValueError(
                    f"Invalid CARDANO_NETWORK in settings: {network_setting}"
                )
        elif isinstance(network_setting, Network):
            self.network = network_setting
        else:
            raise ValueError(
                f"Invalid type for CARDANO_NETWORK in settings: {type(network_setting)}"
            )

        self.api = BlockFrostApi(
            project_id=settings.BLOCKFROST_PROJECT_ID,
            base_url=(
                ApiUrls.preprod.value
                if self.network == Network.TESTNET
                else ApiUrls.mainnet.value
            ),
        )
        self.base_dir = settings.HOTKEY_BASE_DIR
        # Pass resolved network to context getter if it accepts it
        self.chain_context = (
            get_chain_context()
        )  # Assuming context uses settings internally

        # Giải mã hotkey từ file
        payment_sk_obj, stake_sk_obj = decode_hotkey_skey(
            self.base_dir, coldkey_name, hotkey_name, password
        )
        # Cast to correct types
        self.payment_sk = cast(ExtendedSigningKey, payment_sk_obj)
        self.stake_sk = cast(Optional[ExtendedSigningKey], stake_sk_obj)

        if not self.payment_sk:
            # Raise specific error if payment key decoding failed
            raise ValueError(
                f"Failed to decode payment key for {coldkey_name}/{hotkey_name}"
            )

        self.stake_vk = self.stake_sk.to_verification_key() if self.stake_sk else None
        self.payment_vk = self.payment_sk.to_verification_key()
        self.stake_key_hash = self.stake_vk.hash() if self.stake_vk else None
        self.payment_key_hash = self.payment_vk.hash()

        # Địa chỉ ví
        self.payment_address = Address(
            payment_part=self.payment_key_hash, network=self.network
        )
        self.stake_address = (
            Address(staking_part=self.stake_key_hash, network=self.network)
            if self.stake_key_hash
            else None
        )
        self.main_address = Address(
            payment_part=self.payment_key_hash,
            staking_part=self.stake_key_hash,  # Pass None if stake_key_hash is None
            network=self.network,
        )
        logger.debug(f"[WalletInit] Main address: {self.main_address}")
        logger.debug(f"[WalletInit] Payment address: {self.payment_address}")
        logger.debug(f"[WalletInit] Stake address: {self.stake_address}")

    def get_utxos(self):
        try:
            utxos_raw = self.api.address_utxos(str(self.main_address))
            return utxos_raw
        except ApiError as e:
            if e.status_code == 404:
                logger.warning(
                    f"[get_utxos] No UTxOs found for address {self.main_address}."
                )
                if self.network == Network.TESTNET:
                    logger.info(
                        "Consider requesting tADA from the faucet: https://docs.cardano.org/cardano-testnets/tools/faucet/"
                    )
            else:
                logger.exception(
                    f"[get_utxos] Failed to fetch UTxOs for address {self.main_address}: {e}"
                )
            return []
        except Exception as e:
            logger.exception(
                f"[get_utxos] Failed to fetch UTxOs for address {self.main_address}: {e}"
            )
            return []

    def get_balances(self):
        bf_utxos = self.get_utxos()
        if not bf_utxos:
            logger.info("[get_balances] No UTxOs found to calculate balance.")
            return

        logger.info("--- UTXO Balances --- ")
        total_lovelace = 0
        tokens_summary = {}
        for item in bf_utxos:
            utxo_data: Any = item
            try:
                tx_hash = getattr(utxo_data, "tx_hash", utxo_data["tx_hash"])
                tx_index = getattr(utxo_data, "output_index", utxo_data["output_index"])
                amount_list = getattr(utxo_data, "amount", utxo_data["amount"])
            except (AttributeError, KeyError, TypeError) as access_err:
                logger.warning(
                    f"Could not access expected UTxO attributes: {access_err} for data: {utxo_data}"
                )
                continue

            utxo_lovelace = 0
            utxo_tokens_str = ""
            if isinstance(amount_list, list):
                for token_item in amount_list:
                    token_data: Any = token_item
                    try:
                        unit = getattr(token_data, "unit", token_data["unit"])
                        quantity = getattr(
                            token_data, "quantity", token_data["quantity"]
                        )
                        if unit == "lovelace":
                            utxo_lovelace = int(quantity)
                            total_lovelace += utxo_lovelace
                        else:
                            policy_id = unit[:56] if len(unit) > 56 else unit
                            asset_hex = unit[56:] if len(unit) > 56 else ""
                            try:
                                asset_name = bytes.fromhex(asset_hex).decode(
                                    "utf-8", errors="replace"
                                )
                            except ValueError:
                                asset_name = f"(hex: {asset_hex})"
                            token_key = f"{policy_id[:10]}...{asset_name}"
                            tokens_summary[token_key] = tokens_summary.get(
                                token_key, 0
                            ) + int(quantity)
                            utxo_tokens_str += f" + {quantity} {asset_name}"
                    except (AttributeError, KeyError, TypeError) as token_err:
                        logger.warning(
                            f"Could not process token data: {token_err} for token: {token_data}"
                        )
                        utxo_tokens_str += " + [Error]"
            logger.info(
                f"UTxO: {tx_hash}#{tx_index} \t {utxo_lovelace / 1000000:.6f} ADA{utxo_tokens_str}"
            )
        logger.info(f"Total ADA: {total_lovelace / 1000000:.6f}")
        if tokens_summary:
            logger.info("Total Tokens:")
            for key, total_qty in tokens_summary.items():
                logger.info(f"  - {key}: {total_qty}")
        logger.info("---------------------")

    def sign_transaction(self, tx_body: TransactionBody) -> Transaction:
        """
        Ký giao dịch bằng khóa của ví.
        NOTE: This method might be redundant if TransactionBuilder.build_and_sign is used.
        It manually creates the witness set.
        """
        # Create witnesses manually
        payment_witness = VerificationKeyWitness(
            self.payment_vk, self.payment_sk.sign(tx_body.hash())
        )
        witnesses = [payment_witness]
        if self.stake_sk and self.stake_vk:
            # Only add stake witness if stake key exists and is needed (e.g., for withdrawals, certs)
            # Check if tx body requires stake key signature (logic depends on tx type)
            # For simplicity, we add it if it exists, assuming it might be needed.
            stake_witness = VerificationKeyWitness(
                self.stake_vk, self.stake_sk.sign(tx_body.hash())
            )
            witnesses.append(stake_witness)

        witness_set = TransactionWitnessSet(vkey_witnesses=witnesses)
        # Create the final transaction object
        signed_tx = Transaction(
            transaction_body=tx_body, transaction_witness_set=witness_set
        )
        return signed_tx

    def submit_transaction(self, signed_tx: Transaction) -> str:
        return self.chain_context.submit_tx(signed_tx.to_cbor())


class StakingService:
    def __init__(self, wallet: Wallet):
        """
        Khởi tạo dịch vụ staking với ví của người dùng.
        """
        self.chain_context = get_chain_context()
        self.wallet = wallet

    def delegate_stake(self, pool_id: str):
        """
        Ủy quyền stake đến một pool cụ thể.
        """
        logger.info(f"[delegate_stake] Attempting delegation to pool: {pool_id}")
        # Ensure stake key exists before proceeding
        if not self.wallet.stake_key_hash:
            logger.error(
                "[delegate_stake] Wallet does not have a stake key hash. Cannot delegate."
            )
            return None

        stake_credential = StakeCredential(self.wallet.stake_key_hash)  # Now safe
        stake_reg = StakeRegistration(stake_credential)
        try:
            pool_keyhash = PoolKeyHash(bytes.fromhex(pool_id))
        except ValueError:
            logger.error(f"[delegate_stake] Invalid pool ID format: {pool_id}")
            return None
        stake_delegate = StakeDelegation(stake_credential, pool_keyhash)

        tx_builder = TransactionBuilder(self.chain_context)
        tx_builder.add_input_address(self.wallet.main_address)
        tx_builder.certificates = [stake_reg, stake_delegate]

        # Required signers (must be List[VerificationKeyHash])
        required_signers_list: List[VerificationKeyHash] = [
            self.wallet.payment_key_hash
        ]
        # Only add stake_key_hash if it exists (it's also VerificationKeyHash)
        if self.wallet.stake_key_hash:
            required_signers_list.append(self.wallet.stake_key_hash)
        tx_builder.required_signers = required_signers_list

        try:
            # Define signing keys with proper type hint
            signing_keys: List[Union[SigningKey, ExtendedSigningKey]] = [
                self.wallet.payment_sk
            ]
            if self.wallet.stake_sk:
                signing_keys.append(self.wallet.stake_sk)

            signed_tx = tx_builder.build_and_sign(
                signing_keys=signing_keys,
                change_address=self.wallet.main_address,
            )
            tx_id = self.wallet.submit_transaction(signed_tx)
            logger.info(
                f"[delegate_stake] Delegation transaction successfully submitted: {tx_id}"
            )
            return tx_id
        except Exception as e:
            logger.exception(
                f"[delegate_stake] Failed to build, sign, or submit delegation tx: {e}"
            )
            return None

    def re_delegate_stake(self, new_pool_id: str):
        logger.info(
            f"[re_delegate_stake] Attempting re-delegation to pool: {new_pool_id}"
        )
        # Ensure stake key exists
        if not self.wallet.stake_key_hash:
            logger.error(
                "[re_delegate_stake] Wallet does not have a stake key hash. Cannot re-delegate."
            )
            return None

        stake_credential = StakeCredential(self.wallet.stake_key_hash)  # Safe
        try:
            pool_keyhash = PoolKeyHash(bytes.fromhex(new_pool_id))
        except ValueError:
            logger.error(
                f"[re_delegate_stake] Invalid new pool ID format: {new_pool_id}"
            )
            return None
        stake_delegate = StakeDelegation(stake_credential, pool_keyhash)

        tx_builder = TransactionBuilder(self.chain_context)
        tx_builder.add_input_address(self.wallet.main_address)
        tx_builder.certificates = [stake_delegate]

        # Required signers (must be List[VerificationKeyHash])
        required_signers_list: List[VerificationKeyHash] = [
            self.wallet.payment_key_hash
        ]
        if self.wallet.stake_key_hash:
            required_signers_list.append(self.wallet.stake_key_hash)
        tx_builder.required_signers = required_signers_list

        try:
            # Define signing keys with proper type hint
            signing_keys: List[Union[SigningKey, ExtendedSigningKey]] = [
                self.wallet.payment_sk
            ]
            if self.wallet.stake_sk:
                signing_keys.append(self.wallet.stake_sk)

            signed_tx = tx_builder.build_and_sign(
                signing_keys=signing_keys,
                change_address=self.wallet.main_address,
            )
            tx_id = self.wallet.submit_transaction(signed_tx)
            logger.info(
                f"[re_delegate_stake] Re-delegation transaction successfully submitted: {tx_id}"
            )
            return tx_id
        except Exception as e:
            logger.exception(
                f"[re_delegate_stake] Failed to build, sign, or submit re-delegation tx: {e}"
            )
            return None

    def withdrawal_reward(self):
        """
        Rút phần thưởng staking về ví chính.
        """
        if not self.wallet.stake_address:
            logger.error(
                "[withdrawal_reward] Wallet does not have a stake address. Cannot check rewards."
            )
            return None
        # Ensure stake key hash exists for withdrawal
        if not self.wallet.stake_key_hash:
            logger.error(
                "[withdrawal_reward] Wallet does not have a stake key hash. Cannot withdraw."
            )
            return None

        logger.info(
            f"[withdrawal_reward] Checking rewards for stake address: {self.wallet.stake_address}"
        )
        try:
            # Cast result to Any to access attribute safely
            account_info_any: Any = self.wallet.api.accounts(
                str(self.wallet.stake_address)
            )
            withdrawal_reward_amounts = int(
                getattr(account_info_any, "withdrawable_amount", 0)
            )
        except ApiError as e:
            logger.exception(
                f"[withdrawal_reward] Failed to fetch account info for {self.wallet.stake_address}: {e}"
            )
            return None

        if withdrawal_reward_amounts == 0:
            logger.info("[withdrawal_reward] No withdrawable rewards found.")
            return None

        logger.info(
            f"[withdrawal_reward] Withdrawing {withdrawal_reward_amounts} Lovelace."
        )
        tx_builder = TransactionBuilder(self.chain_context)
        tx_builder.add_input_address(self.wallet.main_address)
        try:
            stake_addr_bytes = bytes(self.wallet.stake_address)
            withdrawals = Withdrawals({stake_addr_bytes: withdrawal_reward_amounts})
            tx_builder.withdrawals = withdrawals
        except Exception as e:
            logger.exception(
                f"[withdrawal_reward] Failed to create Withdrawals object: {e}"
            )
            return None

        # Required signers (must be List[VerificationKeyHash])
        required_signers_list: List[VerificationKeyHash] = [
            self.wallet.payment_key_hash
        ]
        if self.wallet.stake_key_hash:
            required_signers_list.append(self.wallet.stake_key_hash)
        tx_builder.required_signers = required_signers_list

        try:
            # Define signing keys with proper type hint
            signing_keys: List[Union[SigningKey, ExtendedSigningKey]] = [
                self.wallet.payment_sk
            ]
            if self.wallet.stake_sk:
                signing_keys.append(self.wallet.stake_sk)

            signed_tx = tx_builder.build_and_sign(
                signing_keys=signing_keys,
                change_address=self.wallet.main_address,
            )
            tx_id = self.wallet.submit_transaction(signed_tx)
            logger.info(
                f"[withdrawal_reward] Withdrawal transaction successfully submitted: {tx_id}"
            )
            return tx_id
        except Exception as e:
            logger.exception(
                f"[withdrawal_reward] Failed to build, sign, or submit withdrawal tx: {e}"
            )
            return None
