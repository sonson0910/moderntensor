// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

/**
 * @title PaymentEscrow - Pay-per-Compute for AI Services
 * @notice Manages MDT token payments for AI inference requests
 * @dev Core component for Phase 2 Native AI Integration
 *
 * Flow:
 * 1. dApp calls deposit() with MDT tokens when creating AI request
 * 2. Upon successful fulfillment, AIOracle calls release() to pay Miner
 * 3. If timeout, requester can call refund() to retrieve tokens
 */
contract PaymentEscrow is Ownable, ReentrancyGuard {
    using SafeERC20 for IERC20;

    // ========== STATE ==========

    /// @notice MDT token contract
    IERC20 public immutable mdtToken;

    /// @notice AIOracle contract address (authorized to release funds)
    address public aiOracle;

    /// @notice Escrow entry for each request
    struct EscrowEntry {
        address depositor;
        uint256 amount;
        uint256 depositedAt;
        uint256 timeout;
        bool released;
        bool refunded;
    }

    /// @notice Mapping from request_id to escrow entry
    mapping(bytes32 => EscrowEntry) public escrows;

    /// @notice Minimum timeout period (blocks)
    uint256 public minTimeout = 100; // ~10 minutes

    /// @notice Protocol fee (basis points, 100 = 1%)
    uint256 public protocolFeeBps = 50; // 0.5%

    /// @notice Accumulated protocol fees
    uint256 public accumulatedFees;

    // ========== EVENTS ==========

    event Deposited(
        bytes32 indexed requestId,
        address indexed depositor,
        uint256 amount,
        uint256 timeout
    );

    event Released(
        bytes32 indexed requestId,
        address indexed miner,
        uint256 minerAmount,
        uint256 protocolFee
    );

    event Refunded(
        bytes32 indexed requestId,
        address indexed depositor,
        uint256 amount
    );

    event OracleUpdated(address indexed oldOracle, address indexed newOracle);

    // ========== ERRORS ==========

    error NotOracle();
    error InvalidAmount();
    error RequestExists();
    error RequestNotFound();
    error AlreadyReleased();
    error AlreadyRefunded();
    error NotTimedOut();
    error NotDepositor();

    // ========== CONSTRUCTOR ==========

    constructor(address _mdtToken) Ownable(msg.sender) {
        mdtToken = IERC20(_mdtToken);
    }

    // ========== MODIFIERS ==========

    modifier onlyOracle() {
        if (msg.sender != aiOracle) revert NotOracle();
        _;
    }

    // ========== EXTERNAL FUNCTIONS ==========

    /**
     * @notice Deposit MDT tokens for an AI request
     * @param requestId The unique request identifier
     * @param amount Amount of MDT tokens to escrow
     * @param timeout Blocks until timeout (min: minTimeout)
     */
    function deposit(
        bytes32 requestId,
        uint256 amount,
        uint256 timeout
    ) external nonReentrant {
        if (amount == 0) revert InvalidAmount();
        if (escrows[requestId].depositor != address(0)) revert RequestExists();
        if (timeout < minTimeout) timeout = minTimeout;

        // Transfer tokens to escrow
        mdtToken.safeTransferFrom(msg.sender, address(this), amount);

        // Create escrow entry
        escrows[requestId] = EscrowEntry({
            depositor: msg.sender,
            amount: amount,
            depositedAt: block.number,
            timeout: timeout,
            released: false,
            refunded: false
        });

        emit Deposited(requestId, msg.sender, amount, timeout);
    }

    /**
     * @notice Release escrowed funds to miner (called by AIOracle)
     * @param requestId The request identifier
     * @param miner The miner address to pay
     */
    function release(
        bytes32 requestId,
        address miner
    ) external onlyOracle nonReentrant {
        EscrowEntry storage entry = escrows[requestId];
        if (entry.depositor == address(0)) revert RequestNotFound();
        if (entry.released) revert AlreadyReleased();
        if (entry.refunded) revert AlreadyRefunded();

        entry.released = true;

        // Calculate fees
        uint256 protocolFee = (entry.amount * protocolFeeBps) / 10000;
        uint256 minerAmount = entry.amount - protocolFee;

        // Accumulate protocol fees
        accumulatedFees += protocolFee;

        // Pay miner
        mdtToken.safeTransfer(miner, minerAmount);

        emit Released(requestId, miner, minerAmount, protocolFee);
    }

    /**
     * @notice Refund escrowed funds after timeout
     * @param requestId The request identifier
     */
    function refund(bytes32 requestId) external nonReentrant {
        EscrowEntry storage entry = escrows[requestId];
        if (entry.depositor == address(0)) revert RequestNotFound();
        if (entry.released) revert AlreadyReleased();
        if (entry.refunded) revert AlreadyRefunded();
        if (msg.sender != entry.depositor) revert NotDepositor();
        if (block.number < entry.depositedAt + entry.timeout)
            revert NotTimedOut();

        entry.refunded = true;

        // Return full amount to depositor
        mdtToken.safeTransfer(entry.depositor, entry.amount);

        emit Refunded(requestId, entry.depositor, entry.amount);
    }

    // ========== ADMIN FUNCTIONS ==========

    /**
     * @notice Set the AIOracle contract address
     * @param _aiOracle New oracle address
     */
    function setAIOracle(address _aiOracle) external onlyOwner {
        emit OracleUpdated(aiOracle, _aiOracle);
        aiOracle = _aiOracle;
    }

    /**
     * @notice Update protocol fee
     * @param _feeBps New fee in basis points (max 500 = 5%)
     */
    function setProtocolFee(uint256 _feeBps) external onlyOwner {
        require(_feeBps <= 500, "Fee too high");
        protocolFeeBps = _feeBps;
    }

    /**
     * @notice Withdraw accumulated protocol fees
     * @param recipient Address to receive fees
     */
    function withdrawFees(address recipient) external onlyOwner {
        uint256 amount = accumulatedFees;
        accumulatedFees = 0;
        mdtToken.safeTransfer(recipient, amount);
    }

    // ========== VIEW FUNCTIONS ==========

    /**
     * @notice Get escrow details for a request
     * @param requestId The request identifier
     * @return entry The escrow entry
     */
    function getEscrow(
        bytes32 requestId
    ) external view returns (EscrowEntry memory) {
        return escrows[requestId];
    }

    /**
     * @notice Check if a request can be refunded
     * @param requestId The request identifier
     * @return canRefund Whether refund is possible
     */
    function canRefund(bytes32 requestId) external view returns (bool) {
        EscrowEntry storage entry = escrows[requestId];
        return (entry.depositor != address(0) &&
            !entry.released &&
            !entry.refunded &&
            block.number >= entry.depositedAt + entry.timeout);
    }
}
