# import click
# from sdk.service.stake_service import StakingService, Wallet
# from sdk.config.settings import settings

# @click.group()
# def stake_cli():
#     """CLI command group for Stake Management."""
#     pass

# @stake_cli.command("init-wallet")
# @click.option("--coldkey", required=True, help="Coldkey name")
# @click.option("--hotkey", required=True, help="Hotkey name")
# @click.option("--password", prompt=True, hide_input=True, help="Wallet password")
# def init_wallet(coldkey, hotkey, password):
#     """Initialize wallet for staking operations."""
#     wallet = Wallet(coldkey_name=coldkey, hotkey_name=hotkey, password=password)
#     click.echo(f"Wallet initialized successfully for {coldkey}/{hotkey}")

# @stake_cli.command("get-balance")
# @click.option("--coldkey", required=True, help="Coldkey name")
# @click.option("--hotkey", required=True, help="Hotkey name")
# @click.option("--password", prompt=True, hide_input=True, help="Wallet password")
# def get_balance(coldkey, hotkey, password):
#     """Get wallet balance and UTXOs."""
#     wallet = Wallet(coldkey_name=coldkey, hotkey_name=hotkey, password=password)
#     wallet.get_balances()

# @stake_cli.command("delegate")
# @click.option("--coldkey", required=True, help="Coldkey name")
# @click.option("--hotkey", required=True, help="Hotkey name")
# @click.option("--password", prompt=True, hide_input=True, help="Wallet password")
# @click.option("--pool-id", required=True, help="Stake pool ID to delegate to")
# def delegate(coldkey, hotkey, password, pool_id):
#     """Delegate stake to a pool."""
#     wallet = Wallet(coldkey_name=coldkey, hotkey_name=hotkey, password=password)
#     staking_service = StakingService(wallet)
#     staking_service.delegate_stake(pool_id=pool_id)
#     click.echo(f"Successfully delegated to pool {pool_id}")

# @stake_cli.command("redelegate")
# @click.option("--coldkey", required=True, help="Coldkey name")
# @click.option("--hotkey", required=True, help="Hotkey name")
# @click.option("--password", prompt=True, hide_input=True, help="Wallet password")
# @click.option("--new-pool-id", required=True, help="New stake pool ID to delegate to")
# def redelegate(coldkey, hotkey, password, new_pool_id):
#     """Redelegate stake to a new pool."""
#     wallet = Wallet(coldkey_name=coldkey, hotkey_name=hotkey, password=password)
#     staking_service = StakingService(wallet)
#     staking_service.re_delegate_stake(new_pool_id=new_pool_id)
#     click.echo(f"Successfully redelegated to pool {new_pool_id}")

# @stake_cli.command("withdraw-rewards")
# @click.option("--coldkey", required=True, help="Coldkey name")
# @click.option("--hotkey", required=True, help="Hotkey name")
# @click.option("--password", prompt=True, hide_input=True, help="Wallet password")
# def withdraw_rewards(coldkey, hotkey, password):
#     """Withdraw staking rewards."""
#     wallet = Wallet(coldkey_name=coldkey, hotkey_name=hotkey, password=password)
#     staking_service = StakingService(wallet)
#     staking_service.withdrawal_reward()
#     click.echo("Successfully withdrew staking rewards")

# if __name__ == "__main__":
#     stake_cli()
