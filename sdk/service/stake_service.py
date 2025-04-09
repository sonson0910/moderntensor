import sys
from pycardano import (
    StakeCredential, StakeRegistration, 
    StakeDelegation, PoolKeyHash,TransactionBuilder, 
    Transaction, TransactionWitnessSet,Network, Address,Withdrawals
)
from pycardano.crypto.bech32 import bech32_decode
from blockfrost import ApiError, ApiUrls, BlockFrostApi, BlockFrostIPFS
from sdk.config.settings import settings
from sdk.keymanager.decryption_utils import decode_hotkey_skey
from sdk.service.context import get_chain_context

class Wallet:
    def __init__(self, coldkey_name, hotkey_name, password):
        """
        Khởi tạo ví bằng cách giải mã khóa hotkey.
        """
        self.api = BlockFrostApi(project_id="preprod06dzhzKlynuTInzvxHDH5cXbdHo524DE", base_url="https://cardano-preprod.blockfrost.io/api/")
        self.base_dir = settings.HOTKEY_BASE_DIR
        self.chain_context = get_chain_context()
        self.network = settings.CARDANO_NETWORK
        # Giải mã hotkey từ file
        self.payment_sk,self.stake_sk = decode_hotkey_skey(self.base_dir, coldkey_name, hotkey_name, password)
       
        self.stake_vk = self.stake_sk.to_verification_key()
        self.payment_vk = self.payment_sk.to_verification_key()
        self.stake_key_hash = self.stake_vk.hash()
        self.payment_key_hash = self.payment_vk.hash()
    
        # Địa chỉ ví
        self.payment_address = Address(payment_part=self.payment_key_hash,network=Network.TESTNET)
        self.stake_address = Address(staking_part=self.stake_key_hash,network=Network.TESTNET)
        self.main_address = Address(payment_part=self.payment_key_hash, staking_part=self.stake_key_hash,network=Network.TESTNET)
        print(f'main_address: {self.main_address}')
        print(f'payment_address: {self.payment_address}')
        print(f'stake_address: {self.stake_address}')
    def get_utxos(self):
        
        try:
            utxos = self.api.address_utxos(self.main_address)
        except Exception as e:
            if e.status_code == 404:
                print("Address does not have any UTXOs. ")
                if self.network == "testnet":
                    print(
                        "Request tADA from the faucet: https://docs.cardano.org/cardano-testnets/tools/faucet/"
                    )
            else:
                print(e.message)
            sys.exit(1)

        print(f"hash \t\t\t\t\t\t\t\t\t amount")
        print(
            "--------------------------------------------------------------------------------------"
        )
        return utxos
    def get_balances(self):
        utxos=self.get_utxos()
        for utxo in utxos:
            tokens = ""
            for token in utxo.amount:
                if token.unit != "lovelace":
                    tokens += f"{token.quantity} {token.unit} + "
            print(
                f"{utxo.tx_hash}#{utxo.tx_index} \t {int(utxo.amount[0].quantity)/1000000} ADA [{tokens}]"
            )



    def sign_transaction(self, tx_body):
        """
        Ký giao dịch bằng khóa của ví.
        """
        tx = Transaction(body=tx_body, witness_set=TransactionWitnessSet())
        tx.sign([self.payment_sk, self.stake_sk])
        return tx
    
    def submit_transaction(self, signed_tx):
        """
        Gửi giao dịch lên blockchain.
        """
        return self.chain_context.submit_tx(signed_tx)



class StakingService:
    def __init__(self, wallet: Wallet):
        """
        Khởi tạo dịch vụ staking với ví của người dùng.
        """
        self.chain_context = get_chain_context()
        self.wallet = wallet
        self.current_pool_id = None  # Theo dõi pool hiện tại đang ủy quyền

   
    def delegate_stake(self, pool_id: str):
        """
        Ủy quyền stake đến một pool cụ thể.
        """
        if self.current_pool_id is not None:
            raise Exception("Đã ủy quyền stake. Chỉ có thể ủy quyền lại pool mới bằng re_delegate_stake()")
        
        # B1: Tạo chứng chỉ đăng ký stake
        stake_credential = StakeCredential(self.wallet.stake_key_hash)
        stake_reg = StakeRegistration(stake_credential)

        # B2: Chuyển đổi Pool ID sang PoolKeyHash
        pool_keyhash = PoolKeyHash(bytes.fromhex(pool_id))
        stake_delegate = StakeDelegation(stake_credential, pool_keyhash)

        # B3: Xây dựng giao dịch
        tx_builder = TransactionBuilder(self.chain_context)

        # Thêm địa chỉ chứa ADA để trả phí
        tx_builder.add_input_address(self.wallet.main_address)

        # Thêm chứng chỉ staking vào giao dịch
        tx_builder.certificates = [stake_reg, stake_delegate]

        # Đặt phí buffer để tránh lỗi
        # tx_builder.fee_buffer = 500000

        # Thêm người ký giao dịch
        tx_builder.required_signers = [self.wallet.payment_key_hash, self.wallet.stake_key_hash]

        # Xây dựng giao dịch đã ký
        tx = tx_builder.build_and_sign(
            [self.wallet.payment_sk, self.wallet.stake_sk], 
            change_address=self.wallet.main_address
        )

        # Gửi giao dịch lên blockchain
        self.chain_context.submit_tx(tx)
        self.current_pool_id = pool_id  # Lưu pool hiện tại
        print(f"Transaction successfully submitted: {tx.id}")

    def re_delegate_stake(self, new_pool_id: str):
        if self.current_pool_id is None:
            raise Exception("Chưa ủy quyền stake vào pool nào")
            
        stake_credential = StakeCredential(self.wallet.stake_key_hash)
        # B2: Chuyển đổi Pool ID sang PoolKeyHash
        pool_keyhash = PoolKeyHash(bytes.fromhex(new_pool_id))
        stake_delegate = StakeDelegation(stake_credential, pool_keyhash)

        # B3: Xây dựng giao dịch
        tx_builder = TransactionBuilder(self.chain_context)

        # Thêm địa chỉ chứa ADA để trả phí
        tx_builder.add_input_address(self.wallet.main_address)

        # Thêm chứng chỉ staking vào giao dịch
        tx_builder.certificates = [stake_delegate]

        # Đặt phí buffer để tránh lỗi
        # tx_builder.fee_buffer = 500000

        # Thêm người ký giao dịch
        tx_builder.required_signers = [self.wallet.payment_key_hash, self.wallet.stake_key_hash]

        # Xây dựng giao dịch đã ký
        tx = tx_builder.build_and_sign(
            [self.wallet.payment_sk, self.wallet.stake_sk], 
            change_address=self.wallet.main_address
        )

        # Gửi giao dịch lên blockchain
        self.chain_context.submit_tx(tx)
        self.current_pool_id = new_pool_id  # Cập nhật pool mới
        print(f"Transaction successfully submitted: {tx.id}")
    def withdrawal_reward(self):
        """
        Rút phần thưởng staking về ví chính.
        """
        account_infor=self.wallet.api.accounts(self.wallet.stake_address)
        
        withdrawal_reward_amounts=int(account_infor.withdrawable_amount)
        if withdrawal_reward_amounts == 0:
            print("Không có phần thưởng để rút.")
            return

        print(f"Rút {withdrawal_reward_amounts} Lovelace từ phần thưởng staking.")
        
        # Lấy Stake Key Hash dưới dạng bytes
        # B3: Xây dựng giao dịch
        tx_builder = TransactionBuilder(self.chain_context)
         # Thêm địa chỉ chứa ADA để trả phí
        tx_builder.add_input_address(self.wallet.main_address)
       
        # Tạo đối tượng Withdrawals
        withdrawals = Withdrawals({bytes(self.wallet.stake_address): withdrawal_reward_amounts})# vẫn đang không chắc về hàm này
        # tx_builder.fee_buffer = 500000
        # Thêm thông tin rút phần thưởng
        tx_builder.withdrawals = withdrawals    
        # Thêm người ký giao dịch
        tx_builder.required_signers = [self.wallet.payment_key_hash, self.wallet.stake_key_hash]

        # Xây dựng giao dịch đã ký
        tx = tx_builder.build_and_sign(
            [self.wallet.payment_sk, self.wallet.stake_sk], 
            change_address=self.wallet.main_address
        )

        # Gửi giao dịch lên blockchain
        self.chain_context.submit_tx(tx)

        print(f"Transaction successfully submitted: {tx.id}")    
# wallet = Wallet(coldkey_name='wallet1',hotkey_name='hk1',password='123456')
# wallet.get_utxos()
# wallet.get_balances()

# # Khởi tạo dịch vụ staking
# staking_service = StakingService(wallet)
# print(f"Done constructer staking service")
# Đăng ký stake một khi đăng kí rồi là không có hủy chỉ có thể đăng kí pool mới
# tx_id_delegate_stake = staking_service.delegate_stake(pool_id="429e1cf4c75799b4148402452aa0edd512111485e5e1cbe7cf93b696")


# Đăng ký pool mới
# tx_id_redelegate_stake = staking_service.re_delegate_stake(new_pool_id="998dd7a084430fb80d204b9d4700ef47cdd6d9bdad1e8eb2c87bc2ad")

# Rút phần thưởng: Nhưng để rút được phần thưởng yêu cầu phải ủy quyền vào pool( nếu không ủy quyền thì sẽ có lỗi)

# TransactionFailedException: Failed to submit transaction. 
# Error code: 400. Error message: {"contents":{"contents":{"contents":{"era":"ShelleyBasedEraConway",
# "error":["ConwayWdrlNotDelegatedToDRep (KeyHash {unKeyHash = \"7e10785dc0bc5c4639863e155679f7c9c719deac3021df37453a70a3\"} 
# :| [])"],"kind":"ShelleyTxValidationError"},"tag":"TxValidationErrorInCardanoMode"},"tag":"TxCmdTxSubmitValidationError"},
# "tag":"TxSubmitFail"}
# tx_id_withdrawal_reward = staking_service.withdrawal_reward()