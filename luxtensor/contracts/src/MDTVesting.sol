// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

/**
 * @title MDTVesting
 * @dev Time-locked vesting contract for ModernTensor token allocations
 *
 * Vesting Schedules:
 * - Team & Core Dev: 1yr cliff + 4yr linear (0% TGE)
 * - Private Sale: 1yr cliff + 2yr linear (0% TGE)
 * - IDO: 25% TGE + 6mo linear
 */
contract MDTVesting is Ownable, ReentrancyGuard {
    using SafeERC20 for IERC20;

    /// @notice MDT token address
    IERC20 public immutable token;

    /// @notice TGE timestamp
    uint256 public tgeTimestamp;

    /// @notice Vesting schedule structure
    struct VestingSchedule {
        uint256 totalAmount; // Total tokens allocated
        uint256 claimedAmount; // Already claimed
        uint256 startTime; // Usually TGE
        uint256 cliffDuration; // Days before any unlock
        uint256 vestingDuration; // Days for linear vesting
        uint8 tgePercent; // Percentage unlocked at TGE (0-100)
        bool revocable; // Can be revoked by owner
        bool revoked; // Has been revoked
    }

    /// @notice Beneficiary address => Vesting schedules
    mapping(address => VestingSchedule[]) public vestingSchedules;

    /// @notice Category types for event tracking
    enum Category {
        TeamCoreDev,
        PrivateSale,
        IDO
    }

    /// @notice Track total allocated vs claimed
    uint256 public totalAllocated;
    uint256 public totalClaimed;

    /// @notice Events
    event VestingCreated(
        address indexed beneficiary,
        uint256 amount,
        Category category
    );
    event TokensClaimed(address indexed beneficiary, uint256 amount);
    event VestingRevoked(
        address indexed beneficiary,
        uint256 index,
        uint256 remaining
    );
    event TGESet(uint256 timestamp);

    constructor(address _token) Ownable(msg.sender) {
        require(_token != address(0), "Invalid token address");
        token = IERC20(_token);
    }

    /**
     * @notice Set TGE timestamp (can only be set once)
     */
    function setTGE(uint256 _timestamp) external onlyOwner {
        require(tgeTimestamp == 0, "TGE already set");
        require(_timestamp > 0, "Invalid timestamp");
        tgeTimestamp = _timestamp;
        emit TGESet(_timestamp);
    }

    /**
     * @notice Create vesting for Team/Core Dev (1yr cliff + 4yr linear)
     */
    function createTeamVesting(
        address beneficiary,
        uint256 amount
    ) external onlyOwner {
        _createVesting(
            beneficiary,
            amount,
            365 days, // 1 year cliff
            1460 days, // 4 years linear
            0, // 0% TGE
            true, // Revocable
            Category.TeamCoreDev
        );
    }

    /**
     * @notice Create vesting for Private Sale (1yr cliff + 2yr linear)
     */
    function createPrivateSaleVesting(
        address beneficiary,
        uint256 amount
    ) external onlyOwner {
        _createVesting(
            beneficiary,
            amount,
            365 days, // 1 year cliff
            730 days, // 2 years linear
            0, // 0% TGE
            false, // Not revocable
            Category.PrivateSale
        );
    }

    /**
     * @notice Create vesting for IDO (25% TGE + 6mo linear)
     */
    function createIDOVesting(
        address beneficiary,
        uint256 amount
    ) external onlyOwner {
        _createVesting(
            beneficiary,
            amount,
            0, // No cliff
            180 days, // 6 months linear
            25, // 25% TGE
            false, // Not revocable
            Category.IDO
        );
    }

    /**
     * @dev Internal vesting creation with duplicate prevention
     *
     * SECURITY: Checks for existing schedules with same cliff/duration/tgePercent
     * to prevent accidental duplicate allocations.
     */
    function _createVesting(
        address beneficiary,
        uint256 amount,
        uint256 cliffDuration,
        uint256 vestingDuration,
        uint8 tgePercent,
        bool revocable,
        Category category
    ) internal {
        require(beneficiary != address(0), "Invalid beneficiary");
        require(amount > 0, "Amount must be > 0");
        require(tgeTimestamp > 0, "TGE not set");

        // VT-01: Check for duplicate schedule with same parameters
        VestingSchedule[] storage existing = vestingSchedules[beneficiary];
        for (uint256 i = 0; i < existing.length; i++) {
            if (
                !existing[i].revoked &&
                existing[i].cliffDuration == cliffDuration &&
                existing[i].vestingDuration == vestingDuration &&
                existing[i].tgePercent == tgePercent
            ) {
                revert("Duplicate vesting schedule exists");
            }
        }

        vestingSchedules[beneficiary].push(
            VestingSchedule({
                totalAmount: amount,
                claimedAmount: 0,
                startTime: tgeTimestamp,
                cliffDuration: cliffDuration,
                vestingDuration: vestingDuration,
                tgePercent: tgePercent,
                revocable: revocable,
                revoked: false
            })
        );

        totalAllocated += amount;

        emit VestingCreated(beneficiary, amount, category);
    }

    /**
     * @notice Calculate vested amount for a specific schedule
     */
    function vestedAmount(
        address beneficiary,
        uint256 index
    ) public view returns (uint256) {
        VestingSchedule storage schedule = vestingSchedules[beneficiary][index];

        if (schedule.revoked) {
            return schedule.claimedAmount;
        }

        if (block.timestamp < schedule.startTime) {
            return 0;
        }

        uint256 elapsed = block.timestamp - schedule.startTime;
        uint256 tgeAmount = (schedule.totalAmount * schedule.tgePercent) / 100;
        uint256 vestingAmount = schedule.totalAmount - tgeAmount;

        // During cliff, only TGE amount is vested
        if (elapsed < schedule.cliffDuration) {
            return tgeAmount;
        }

        // After cliff, calculate linear vesting
        uint256 cliffEnd = schedule.cliffDuration;
        uint256 vestingElapsed = elapsed - cliffEnd;

        if (schedule.vestingDuration == 0) {
            return schedule.totalAmount; // Fully vested after cliff
        }

        if (vestingElapsed >= schedule.vestingDuration) {
            return schedule.totalAmount; // Fully vested
        }

        uint256 linearVested = (vestingAmount * vestingElapsed) /
            schedule.vestingDuration;
        return tgeAmount + linearVested;
    }

    /**
     * @notice Calculate claimable amount for a beneficiary (all schedules)
     */
    function claimable(address beneficiary) public view returns (uint256) {
        uint256 total = 0;
        for (uint256 i = 0; i < vestingSchedules[beneficiary].length; i++) {
            VestingSchedule storage schedule = vestingSchedules[beneficiary][i];
            if (!schedule.revoked) {
                uint256 vested = vestedAmount(beneficiary, i);
                total += vested - schedule.claimedAmount;
            }
        }
        return total;
    }

    /**
     * @notice Claim vested tokens
     */
    function claim() external nonReentrant {
        uint256 total = 0;

        for (uint256 i = 0; i < vestingSchedules[msg.sender].length; i++) {
            VestingSchedule storage schedule = vestingSchedules[msg.sender][i];
            if (!schedule.revoked) {
                uint256 vested = vestedAmount(msg.sender, i);
                uint256 claimableAmount = vested - schedule.claimedAmount;

                if (claimableAmount > 0) {
                    schedule.claimedAmount = vested;
                    total += claimableAmount;
                }
            }
        }

        require(total > 0, "Nothing to claim");

        totalClaimed += total;
        token.safeTransfer(msg.sender, total);

        emit TokensClaimed(msg.sender, total);
    }

    /**
     * @notice Revoke a vesting schedule (only for revocable schedules)
     */
    function revoke(address beneficiary, uint256 index) external onlyOwner {
        require(index < vestingSchedules[beneficiary].length, "Invalid index");
        VestingSchedule storage schedule = vestingSchedules[beneficiary][index];

        require(schedule.revocable, "Not revocable");
        require(!schedule.revoked, "Already revoked");

        uint256 vested = vestedAmount(beneficiary, index);
        uint256 remaining = schedule.totalAmount - vested;

        schedule.revoked = true;
        totalAllocated -= remaining;

        // Return unvested tokens to owner
        if (remaining > 0) {
            token.safeTransfer(owner(), remaining);
        }

        emit VestingRevoked(beneficiary, index, remaining);
    }

    /**
     * @notice Get vesting info for a beneficiary
     */
    function getVestingInfo(
        address beneficiary
    )
        external
        view
        returns (
            uint256 scheduleCount,
            uint256 totalVested,
            uint256 totalClaimable,
            uint256 totalClaimed_
        )
    {
        scheduleCount = vestingSchedules[beneficiary].length;

        for (uint256 i = 0; i < scheduleCount; i++) {
            VestingSchedule storage schedule = vestingSchedules[beneficiary][i];
            if (!schedule.revoked) {
                uint256 vested = vestedAmount(beneficiary, i);
                totalVested += vested;
                totalClaimable += vested - schedule.claimedAmount;
                totalClaimed_ += schedule.claimedAmount;
            }
        }
    }

    /**
     * @notice Emergency withdraw (only unused tokens)
     */
    function emergencyWithdraw() external onlyOwner {
        uint256 balance = token.balanceOf(address(this));
        uint256 uncommitted = balance - (totalAllocated - totalClaimed);

        require(uncommitted > 0, "No uncommitted tokens");
        token.safeTransfer(owner(), uncommitted);
    }
}
