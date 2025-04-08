# sdk/metagraph/hash_service.py
import hashlib
import json

def hash_data(data):
    """
    Hash dữ liệu thành SHA-256 (32 bytes).
    - Tham số data: có thể là str, list, dict, hoặc bytearray.
    - Trả về: bytes (32 bytes).
    """
    if isinstance(data, (list, dict)):
        # Serialize dữ liệu thành JSON string rồi hash
        serialized_data = json.dumps(data, sort_keys=True).encode('utf-8')
    elif isinstance(data, str):
        serialized_data = data.encode('utf-8')
    elif isinstance(data, bytearray):
        serialized_data = bytes(data)
    else:
        raise ValueError("Kiểu dữ liệu không hỗ trợ để hash")
    return hashlib.sha256(serialized_data).digest()