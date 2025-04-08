# tests/formulas/test_resource_allocation.py
import pytest
from sdk.formulas import calculate_subnet_resource

def test_calculate_subnet_resource_basic():
    """Kiểm tra phân bổ tài nguyên cơ bản."""
    subnet_contrib = 30.0
    total_contrib = 100.0
    total_resources = 1000.0
    allocation = calculate_subnet_resource(subnet_contrib, total_contrib, total_resources)
    assert allocation == pytest.approx(300.0)

def test_calculate_subnet_resource_zero_total():
    """Kiểm tra khi tổng đóng góp bằng 0."""
    allocation = calculate_subnet_resource(30.0, 0.0, 1000.0)
    assert allocation == 0.0

def test_calculate_subnet_resource_zero_subnet():
    """Kiểm tra khi đóng góp của subnet bằng 0."""
    allocation = calculate_subnet_resource(0.0, 100.0, 1000.0)
    assert allocation == 0.0