# sdk/metagraph/verify_service.py
import hashlib
import json

def verify_hash(data, stored_hash):
    """
    Kiểm tra xem hash của data có khớp với stored_hash không.
    - Tham số data: dữ liệu gốc cần kiểm tra.
    - Tham số stored_hash: hash đã lưu trong datum (bytes).
    - Trả về: bool (True nếu khớp, False nếu không).
    """
    if isinstance(data, (list, dict)):
        serialized_data = json.dumps(data, sort_keys=True).encode('utf-8')
    elif isinstance(data, str):
        serialized_data = data.encode('utf-8')
    elif isinstance(data, bytearray):
        serialized_data = bytes(data)
    else:
        raise ValueError("Kiểu dữ liệu không hỗ trợ để hash")
    computed_hash = hashlib.sha256(serialized_data).digest()
    return computed_hash == stored_hash