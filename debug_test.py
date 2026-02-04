"""Test consensus method fault"""
import sys
import traceback
sys.path.insert(0, '.')

from sdk.client import LuxtensorClient

client = LuxtensorClient()
client.init_consensus()

print("Consensus initialized:", client._consensus_initialized)

try:
    state = client.get_consensus_state()
    print("SUCCESS:", state)
except Exception as e:
    print("ERROR:", e)
    traceback.print_exc()
