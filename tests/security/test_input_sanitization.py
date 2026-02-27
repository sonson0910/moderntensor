"""Tests for input sanitization, overflow protection, and boundary checks.

Covers:
- Integer overflow / underflow protection
- Hex parsing robustness
- BPS range validation
- Error code boundary checks
- RPC error parsing with malicious input
- Address injection payloads
"""

import pytest

from sdk.utils import (
    to_mdt,
    from_mdt,
    format_mdt,
    validate_address,
    MDT_DECIMALS,
    MDT_WEI_MULTIPLIER,
)
from sdk.utils.bps_utils import (
    float_to_bps,
    bps_to_float,
    percent_to_bps,
    validate_bps,
    calculate_proportional_share,
    distribute_by_scores,
    MAX_BPS,
)
from sdk.errors import (
    RpcError,
    RpcErrorCode,
    LuxtensorConnectionError,
    parse_rpc_error,
    check_rpc_response,
)
from sdk.client.base import BaseClient


# ─── Token Conversion Boundaries ────────────────────────────────────────────


class TestTokenConversionBoundaries:
    def test_zero_wei(self):
        assert to_mdt(0) == 0

    def test_one_token(self):
        assert to_mdt(MDT_WEI_MULTIPLIER) == 1

    def test_very_large_amount(self):
        """21M tokens (total supply) should not overflow."""
        total_supply = 21_000_000 * MDT_WEI_MULTIPLIER
        result = to_mdt(total_supply)
        assert result == 21_000_000

    def test_very_small_amount(self):
        """1 wei should convert to a very small decimal."""
        result = to_mdt(1)
        assert result > 0
        assert result < 1

    def test_from_mdt_zero(self):
        assert from_mdt(0) == 0

    def test_from_mdt_total_supply(self):
        result = from_mdt(21_000_000)
        expected = 21_000_000 * MDT_WEI_MULTIPLIER
        assert result == expected

    def test_roundtrip_precision(self):
        """to_mdt -> from_mdt should preserve value."""
        original = 123_456_789_000_000_000
        mdt = to_mdt(original)
        back = from_mdt(mdt)
        assert back == original

    def test_hex_string_conversion(self):
        assert to_mdt("0xde0b6b3a7640000") == 1  # 1 MDT

    def test_decimal_string_conversion(self):
        assert to_mdt("1000000000000000000") == 1

    def test_format_mdt_zero(self):
        assert "0.0000 MDT" == format_mdt(0)

    def test_from_mdt_string(self):
        assert from_mdt("1.5") == 1_500_000_000_000_000_000

    def test_from_mdt_negative_value(self):
        """Negative value should produce negative wei (no crash)."""
        result = from_mdt(-1)
        assert result == -MDT_WEI_MULTIPLIER


# ─── BPS Validation ──────────────────────────────────────────────────────────


class TestBPSValidation:
    def test_valid_bps_min(self):
        assert validate_bps(0) == 0

    def test_valid_bps_max(self):
        assert validate_bps(MAX_BPS) == MAX_BPS

    def test_invalid_bps_negative(self):
        with pytest.raises(ValueError):
            validate_bps(-1)

    def test_invalid_bps_too_high(self):
        with pytest.raises(ValueError):
            validate_bps(10001)

    def test_invalid_bps_type(self):
        with pytest.raises(TypeError):
            validate_bps(0.5)  # type: ignore

    def test_float_to_bps_boundaries(self):
        assert float_to_bps(0.0) == 0
        assert float_to_bps(1.0) == 10000

    def test_float_to_bps_out_of_range(self):
        with pytest.raises(ValueError):
            float_to_bps(1.1)
        with pytest.raises(ValueError):
            float_to_bps(-0.1)

    def test_percent_to_bps_boundaries(self):
        assert percent_to_bps(0.0) == 0
        assert percent_to_bps(100.0) == 10000

    def test_percent_to_bps_out_of_range(self):
        with pytest.raises(ValueError):
            percent_to_bps(101.0)
        with pytest.raises(ValueError):
            percent_to_bps(-1.0)

    def test_bps_to_float_roundtrip(self):
        for bps in [0, 1, 5000, 9999, 10000]:
            assert float_to_bps(bps_to_float(bps)) == bps

    def test_proportional_share_zero_total(self):
        assert calculate_proportional_share(0, 5000) == 0

    def test_proportional_share_full(self):
        assert calculate_proportional_share(1000, 10000) == 1000

    def test_proportional_share_half(self):
        assert calculate_proportional_share(1000, 5000) == 500

    def test_distribute_by_scores_empty(self):
        assert distribute_by_scores(1000, []) == []

    def test_distribute_by_scores_all_zero(self):
        result = distribute_by_scores(1000, [0, 0, 0])
        assert sum(result) == 999  # integer division gives 333 each
        assert len(result) == 3

    def test_distribute_by_scores_preserves_total(self):
        result = distribute_by_scores(1000, [3000, 5000, 2000])
        assert sum(result) == 1000

    def test_distribute_by_scores_single(self):
        result = distribute_by_scores(1000, [10000])
        assert result == [1000]


# ─── Hex Parsing Robustness ──────────────────────────────────────────────────


class TestHexParsing:
    def test_parse_hex_int_none(self):
        assert BaseClient._parse_hex_int(None) == 0

    def test_parse_hex_int_integer(self):
        assert BaseClient._parse_hex_int(42) == 42

    def test_parse_hex_int_hex_string(self):
        assert BaseClient._parse_hex_int("0x1a") == 26

    def test_parse_hex_int_decimal_string(self):
        assert BaseClient._parse_hex_int("42") == 42

    def test_parse_hex_int_invalid_string(self):
        assert BaseClient._parse_hex_int("not_a_number") == 0

    def test_parse_hex_int_empty_string(self):
        assert BaseClient._parse_hex_int("") == 0

    def test_parse_hex_int_custom_default(self):
        assert BaseClient._parse_hex_int(None, default=-1) == -1


# ─── Error Code & RPC Error Parsing ──────────────────────────────────────────


class TestRpcErrorParsing:
    def test_parse_rpc_error_block_not_found(self):
        err = parse_rpc_error({"code": -32001, "message": "Block not found: 999"})
        assert err.code == RpcErrorCode.BLOCK_NOT_FOUND

    def test_parse_rpc_error_unknown_code(self):
        err = parse_rpc_error({"code": -99999, "message": "weird"})
        assert isinstance(err, RpcError)
        assert err.code == -99999

    def test_parse_rpc_error_missing_fields(self):
        err = parse_rpc_error({})
        assert err.code == RpcErrorCode.INTERNAL_ERROR
        assert err.message == "Unknown error"

    def test_rpc_error_is_retryable(self):
        err = RpcError(code=RpcErrorCode.RATE_LIMITED, message="slow down")
        assert err.is_retryable

    def test_rpc_error_not_retryable(self):
        err = RpcError(code=RpcErrorCode.INVALID_SIGNATURE, message="bad sig")
        assert not err.is_retryable

    def test_rpc_error_name(self):
        err = RpcError(code=RpcErrorCode.NONCE_TOO_LOW, message="nonce")
        assert err.error_name == "NONCE_TOO_LOW"

    def test_check_rpc_response_success(self):
        result = check_rpc_response({"result": 42})
        assert result == 42

    def test_check_rpc_response_error(self):
        with pytest.raises(RpcError):
            check_rpc_response({"error": {"code": -32001, "message": "not found"}})


# ─── LuxtensorConnectionError ───────────────────────────────────────────────


class TestConnectionError:
    def test_connection_error_basic(self):
        err = LuxtensorConnectionError("http://localhost:8545")
        assert "localhost:8545" in str(err)
        assert err.url == "http://localhost:8545"
        assert err.cause is None

    def test_connection_error_with_cause(self):
        cause = IOError("connection refused")
        err = LuxtensorConnectionError("http://localhost:8545", cause)
        assert err.cause is cause
        assert "connection refused" in str(err)

    def test_connection_error_is_catchable(self):
        with pytest.raises(LuxtensorConnectionError):
            raise LuxtensorConnectionError("http://bad:9999")

    def test_connection_error_not_rpc_error(self):
        """LuxtensorConnectionError should NOT be caught by RpcError handler."""
        with pytest.raises(LuxtensorConnectionError):
            try:
                raise LuxtensorConnectionError("http://bad:9999")
            except RpcError:
                pytest.fail("Should not be caught as RpcError")


# ─── Address Injection ───────────────────────────────────────────────────────


class TestAddressInjection:
    INJECTION_PAYLOADS = [
        "0x'; DROP TABLE accounts; --",
        "0x<script>alert(1)</script>",
        "0x" + "a" * 100,  # too long
        "0x" + "g" * 40,   # invalid hex
        "",
        " ",
        "\x00" * 42,  # null bytes
        "0x" + "00" * 20 + "\n",  # newline injection
    ]

    @pytest.mark.parametrize("payload", INJECTION_PAYLOADS)
    def test_validate_address_rejects_injection(self, payload):
        assert not validate_address(payload)
