// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

/**
 * @title SimpleVesting
 * @notice Simple token vesting with lock and unlock functionality
 * @dev Implements checks-effects-interactions pattern with reentrancy protection
 */
contract SimpleVesting {
    // ============ Constants ============
    uint256 public constant MIN_LOCK_DURATION = 1 seconds; // 1s for testing
    uint256 public constant MAX_LOCK_DURATION = 10 * 365 days;

    // ============ State Variables ============
    address public immutable owner;
    address public immutable beneficiary;
    uint256 public lockedAmount;
    uint256 public immutable releaseTime;
    bool public released;
    bool public revoked;

    // ============ Reentrancy Guard ============
    uint256 private constant NOT_ENTERED = 1;
    uint256 private constant ENTERED = 2;
    uint256 private _status;

    modifier nonReentrant() {
        require(_status != ENTERED, "ReentrancyGuard: reentrant call");
        _status = ENTERED;
        _;
        _status = NOT_ENTERED;
    }

    // ============ Events ============
    event Locked(
        address indexed beneficiary,
        uint256 amount,
        uint256 releaseTime
    );
    event Unlocked(address indexed beneficiary, uint256 amount);
    event Revoked(address indexed owner, uint256 amount);

    // ============ Constructor ============
    constructor(address _beneficiary, uint256 _lockDuration) payable {
        require(_beneficiary != address(0), "Invalid beneficiary");
        require(msg.value > 0, "Must lock some tokens");
        require(
            _lockDuration >= MIN_LOCK_DURATION &&
                _lockDuration <= MAX_LOCK_DURATION,
            "Invalid lock duration"
        );

        owner = msg.sender;
        beneficiary = _beneficiary;
        lockedAmount = msg.value;
        releaseTime = block.timestamp + _lockDuration;
        released = false;
        revoked = false;
        _status = NOT_ENTERED;

        emit Locked(_beneficiary, msg.value, releaseTime);
    }

    /**
     * @notice Release locked tokens to beneficiary after lock period
     * @dev Uses nonReentrant modifier and checks-effects-interactions pattern
     */
    function unlock() external nonReentrant {
        require(msg.sender == beneficiary, "Only beneficiary can unlock");
        require(block.timestamp >= releaseTime, "Lock period not ended");
        require(!released, "Already released");
        require(!revoked, "Vesting revoked");
        require(address(this).balance >= lockedAmount, "Insufficient balance");

        // Effects: Update state before external call
        released = true;
        uint256 amount = lockedAmount;
        lockedAmount = 0;

        // Interactions: External call last
        (bool success, ) = payable(beneficiary).call{value: amount}("");
        require(success, "Transfer failed");

        emit Unlocked(beneficiary, amount);
    }

    /**
     * @notice Owner can revoke vesting and reclaim funds (emergency function)
     * @dev Only callable before beneficiary unlocks
     */
    function revoke() external nonReentrant {
        require(msg.sender == owner, "Only owner can revoke");
        require(!released, "Already released");
        require(!revoked, "Already revoked");

        // Effects
        revoked = true;
        uint256 amount = address(this).balance;
        lockedAmount = 0;

        // Interactions
        (bool success, ) = payable(owner).call{value: amount}("");
        require(success, "Transfer failed");

        emit Revoked(owner, amount);
    }

    /**
     * @notice Get remaining lock time in seconds
     * @return Seconds until unlock, or 0 if already unlockable
     */
    function remainingLockTime() external view returns (uint256) {
        if (block.timestamp >= releaseTime) {
            return 0;
        }
        return releaseTime - block.timestamp;
    }

    /**
     * @notice Check if tokens can be unlocked
     * @return True if unlock conditions are met
     */
    function canUnlock() external view returns (bool) {
        return block.timestamp >= releaseTime && !released && !revoked;
    }

    /**
     * @notice Get contract balance
     * @return Current ETH balance of contract
     */
    function getBalance() external view returns (uint256) {
        return address(this).balance;
    }
}
