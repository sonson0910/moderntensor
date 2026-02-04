"""Append final 3 staking methods to staking_mixin.py"""

code_to_append = '''
    def get_delegation(self, delegator: str) -> Optional[Dict[str, Any]]:
        """Get delegation info for a delegator."""
        try:
            result = self._call_rpc("staking_getDelegation", [delegator])
            return result
        except Exception as e:
            logger.warning(f"Failed to get delegation for {delegator}: {e}")
            return None

    def get_nominators(self, hotkey: str) -> List[str]:
        """Get list of nominators for a delegate."""
        try:
            result = self._call_rpc("query_nominators", [hotkey])
            return result
        except Exception as e:
            logger.error(f"Error getting nominators for {hotkey}: {e}")
            raise

    def get_staking_minimums(self) -> Dict[str, int]:
        """Get minimum staking requirements."""
        try:
            result = self._call_rpc("staking_getMinimums", [])
            minimums = {}
            if result:
                min_stake = result.get("minValidatorStake", "0x0")
                min_del = result.get("minDelegation", "0x0")
                minimums["minValidatorStake"] = int(min_stake, 16) if min_stake.startswith("0x") else int(min_stake)
                minimums["minDelegation"] = int(min_del, 16) if min_del.startswith("0x") else int(min_del)
            return minimums
        except Exception as e:
            logger.warning(f"Failed to get staking minimums: {e}")
            return {"minValidatorStake": 0, "minDelegation": 0}
'''

with open('sdk/client/staking_mixin.py', 'a') as f:
    f.write(code_to_append)

print("✅ Added 3 staking methods to StakingMixin")
