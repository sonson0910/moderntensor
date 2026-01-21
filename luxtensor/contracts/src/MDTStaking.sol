// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

/**
 * @title MDTStaking
 * @dev Staking contract for MDT tokens with time-based lock bonuses
 *
 * Bonus Rates (per tokenomics):
 * - 30+ days: 10%
 * - 90+ days: 25%
 * - 180+ days: 50%
 * - 365+ days: 100%
 */
contract MDTStaking is Ownable, ReentrancyGuard {
    using SafeERC20 for IERC20;

    IERC20 public immutable token;

    struct StakeLock {
        uint256 amount;
        uint256 lockTime;
        uint256 unlockTime;
        uint256 bonusRate; // In basis points (1000 = 10%)
        bool withdrawn;
    }

    mapping(address => StakeLock[]) public stakeLocks;

    uint256 public totalStaked;
    uint256 public totalBonusPaid;

    event Staked(
        address indexed staker,
        uint256 amount,
        uint256 lockDays,
        uint256 bonusRate,
        uint256 unlockTime
    );
    event Unstaked(
        address indexed staker,
        uint256 amount,
        uint256 bonus,
        uint256 totalReturned
    );

    constructor(address _token) Ownable(msg.sender) {
        require(_token != address(0), "Invalid token");
        token = IERC20(_token);
    }

    /**
     * @notice Calculate bonus rate based on lock duration
     */
    function getBonusRate(uint256 lockDays) public pure returns (uint256) {
        if (lockDays >= 365) return 10000; // 100%
        if (lockDays >= 180) return 5000; // 50%
        if (lockDays >= 90) return 2500; // 25%
        if (lockDays >= 30) return 1000; // 10%
        return 0;
    }

    /**
     * @notice Lock MDT tokens for a period
     * @param amount Amount of MDT to lock
     * @param lockDays Number of days to lock
     */
    function lock(uint256 amount, uint256 lockDays) external nonReentrant {
        require(amount > 0, "Amount must be > 0");
        require(lockDays > 0, "Lock days must be > 0");
        require(lockDays <= 365, "Max lock is 365 days");

        // Transfer tokens to contract
        token.safeTransferFrom(msg.sender, address(this), amount);

        uint256 bonusRate = getBonusRate(lockDays);
        uint256 unlockTime = block.timestamp + (lockDays * 1 days);

        stakeLocks[msg.sender].push(
            StakeLock({
                amount: amount,
                lockTime: block.timestamp,
                unlockTime: unlockTime,
                bonusRate: bonusRate,
                withdrawn: false
            })
        );

        totalStaked += amount;

        emit Staked(msg.sender, amount, lockDays, bonusRate, unlockTime);
    }

    /**
     * @notice Lock MDT tokens for seconds (for testing)
     */
    function lockSeconds(
        uint256 amount,
        uint256 lockSeconds_
    ) external nonReentrant {
        require(amount > 0, "Amount must be > 0");
        require(lockSeconds_ > 0, "Lock seconds must be > 0");

        token.safeTransferFrom(msg.sender, address(this), amount);

        uint256 lockDays = lockSeconds_ / 86400;
        uint256 bonusRate = getBonusRate(lockDays);
        uint256 unlockTime = block.timestamp + lockSeconds_;

        stakeLocks[msg.sender].push(
            StakeLock({
                amount: amount,
                lockTime: block.timestamp,
                unlockTime: unlockTime,
                bonusRate: bonusRate,
                withdrawn: false
            })
        );

        totalStaked += amount;

        emit Staked(msg.sender, amount, lockDays, bonusRate, unlockTime);
    }

    /**
     * @notice Unlock and withdraw staked tokens with bonus
     * @param index Index of the stake lock to unlock
     */
    function unlock(uint256 index) external nonReentrant {
        require(index < stakeLocks[msg.sender].length, "Invalid index");
        StakeLock storage stake = stakeLocks[msg.sender][index];

        require(!stake.withdrawn, "Already withdrawn");
        require(block.timestamp >= stake.unlockTime, "Lock not expired");

        uint256 bonus = (stake.amount * stake.bonusRate) / 10000;
        uint256 totalReturn = stake.amount + bonus;

        stake.withdrawn = true;
        totalStaked -= stake.amount;
        totalBonusPaid += bonus;

        // Transfer tokens back with bonus
        token.safeTransfer(msg.sender, totalReturn);

        emit Unstaked(msg.sender, stake.amount, bonus, totalReturn);
    }

    /**
     * @notice Get user's stake info
     */
    function getStakeInfo(
        address staker
    )
        external
        view
        returns (
            uint256 activeStakes,
            uint256 totalLocked,
            uint256 pendingBonus
        )
    {
        for (uint256 i = 0; i < stakeLocks[staker].length; i++) {
            StakeLock storage stake = stakeLocks[staker][i];
            if (!stake.withdrawn) {
                activeStakes++;
                totalLocked += stake.amount;
                pendingBonus += (stake.amount * stake.bonusRate) / 10000;
            }
        }
    }

    /**
     * @notice Get specific stake lock details
     */
    function getStakeLock(
        address staker,
        uint256 index
    )
        external
        view
        returns (
            uint256 amount,
            uint256 lockTime,
            uint256 unlockTime,
            uint256 bonusRate,
            bool withdrawn,
            bool canUnlock
        )
    {
        require(index < stakeLocks[staker].length, "Invalid index");
        StakeLock storage stake = stakeLocks[staker][index];
        return (
            stake.amount,
            stake.lockTime,
            stake.unlockTime,
            stake.bonusRate,
            stake.withdrawn,
            block.timestamp >= stake.unlockTime && !stake.withdrawn
        );
    }

    /**
     * @notice Fund contract with bonus tokens (owner only)
     */
    function fundBonusPool(uint256 amount) external onlyOwner {
        token.safeTransferFrom(msg.sender, address(this), amount);
    }

    /**
     * @notice Get number of stakes for a user
     */
    function getStakeCount(address staker) external view returns (uint256) {
        return stakeLocks[staker].length;
    }
}
