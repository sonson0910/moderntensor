# sdk/network/app/dependencies.py
"""
Định nghĩa các dependency providers cho ứng dụng FastAPI.
"""
from fastapi import HTTPException, status
from typing import Optional
from sdk.consensus.node import ValidatorNode

# Biến global để giữ instance của ValidatorNode
# Trong ứng dụng lớn, nên dùng container quản lý dependency chuyên dụng hơn (vd: dependency-injector)
_validator_node_instance: Optional[ValidatorNode] = None

def set_validator_node_instance(node: ValidatorNode):
    """
    Hàm để thiết lập instance ValidatorNode toàn cục (sẽ được gọi khi khởi tạo app).
    """
    global _validator_node_instance
    if ValidatorNode is None:
         print("Error: ValidatorNode class not imported correctly. Cannot set instance.")
         return
    if not isinstance(node, ValidatorNode):
        raise TypeError("Provided instance is not a valid ValidatorNode.")
    _validator_node_instance = node
    print("ValidatorNode instance has been set for API dependencies.")

async def get_validator_node() -> ValidatorNode:
    """
    Dependency function cho FastAPI để lấy instance ValidatorNode.
    """
    if _validator_node_instance is None:
        print("Error: Validator node instance requested but not set.")
        raise HTTPException(
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            detail="Validator node service is not available or not initialized.",
        )
    return _validator_node_instance

# Có thể thêm các dependency khác ở đây nếu cần
