// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

/**
 * @title TokenVesting
 * @dev A vesting contract that locks tokens and releases them over time.
 *
 * Features:
 * - Lock tokens for a beneficiary with a cliff and vesting period
 * - Linear vesting after cliff
 * - Revocable by owner (returns unvested tokens)
 * - Emergency withdraw by owner
 */
contract TokenVesting {
    // Vesting schedule structure
    struct VestingSchedule {
        address beneficiary;
        uint256 totalAmount;
        uint256 startTime;
        uint256 cliffDuration; // Cliff period in seconds
        uint256 vestingDuration; // Total vesting period in seconds
        uint256 releasedAmount;
        bool revoked;
        bool initialized;
    }

    // Owner of the contract
    address public owner;

    // Counter for vesting schedule IDs
    uint256 public vestingIdCounter;

    // Mapping from schedule ID to VestingSchedule
    mapping(uint256 => VestingSchedule) public vestingSchedules;

    // Mapping from beneficiary to their schedule IDs
    mapping(address => uint256[]) public beneficiarySchedules;

    // Events
    event VestingCreated(
        uint256 indexed scheduleId,
        address indexed beneficiary,
        uint256 amount,
        uint256 startTime,
        uint256 cliffDuration,
        uint256 vestingDuration
    );

    event TokensReleased(
        uint256 indexed scheduleId,
        address indexed beneficiary,
        uint256 amount
    );

    event VestingRevoked(
        uint256 indexed scheduleId,
        address indexed beneficiary,
        uint256 unvestedAmount
    );

    event Deposit(address indexed sender, uint256 amount);
    event EmergencyWithdraw(address indexed owner, uint256 amount);

    // Modifiers
    modifier onlyOwner() {
        require(msg.sender == owner, "Only owner can call this function");
        _;
    }

    modifier scheduleExists(uint256 scheduleId) {
        require(
            vestingSchedules[scheduleId].initialized,
            "Vesting schedule does not exist"
        );
        _;
    }

    constructor() {
        owner = msg.sender;
        vestingIdCounter = 0;
    }

    /**
     * @dev Deposit tokens into the contract
     */
    receive() external payable {
        emit Deposit(msg.sender, msg.value);
    }

    /**
     * @dev Create a new vesting schedule
     * @param beneficiary Address of the beneficiary
     * @param amount Total amount to vest
     * @param cliffDuration Cliff period in seconds
     * @param vestingDuration Total vesting duration in seconds
     * @return scheduleId The ID of the created schedule
     */
    function createVestingSchedule(
        address beneficiary,
        uint256 amount,
        uint256 cliffDuration,
        uint256 vestingDuration
    ) external onlyOwner returns (uint256 scheduleId) {
        require(
            beneficiary != address(0),
            "Beneficiary cannot be zero address"
        );
        require(amount > 0, "Amount must be greater than 0");
        require(vestingDuration > 0, "Vesting duration must be greater than 0");
        require(
            cliffDuration <= vestingDuration,
            "Cliff cannot exceed vesting duration"
        );
        require(
            address(this).balance >= amount,
            "Insufficient contract balance"
        );

        scheduleId = vestingIdCounter++;

        vestingSchedules[scheduleId] = VestingSchedule({
            beneficiary: beneficiary,
            totalAmount: amount,
            startTime: block.timestamp,
            cliffDuration: cliffDuration,
            vestingDuration: vestingDuration,
            releasedAmount: 0,
            revoked: false,
            initialized: true
        });

        beneficiarySchedules[beneficiary].push(scheduleId);

        emit VestingCreated(
            scheduleId,
            beneficiary,
            amount,
            block.timestamp,
            cliffDuration,
            vestingDuration
        );

        return scheduleId;
    }

    /**
     * @dev Calculate the vested amount for a schedule
     * @param scheduleId The ID of the vesting schedule
     * @return The vested amount
     */
    function vestedAmount(
        uint256 scheduleId
    ) public view scheduleExists(scheduleId) returns (uint256) {
        VestingSchedule storage schedule = vestingSchedules[scheduleId];

        if (schedule.revoked) {
            return schedule.releasedAmount;
        }

        if (block.timestamp < schedule.startTime + schedule.cliffDuration) {
            return 0;
        }

        if (block.timestamp >= schedule.startTime + schedule.vestingDuration) {
            return schedule.totalAmount;
        }

        // Linear vesting
        uint256 elapsed = block.timestamp - schedule.startTime;
        return (schedule.totalAmount * elapsed) / schedule.vestingDuration;
    }

    /**
     * @dev Calculate the releasable amount for a schedule
     * @param scheduleId The ID of the vesting schedule
     * @return The releasable amount
     */
    function releasableAmount(
        uint256 scheduleId
    ) public view scheduleExists(scheduleId) returns (uint256) {
        return
            vestedAmount(scheduleId) -
            vestingSchedules[scheduleId].releasedAmount;
    }

    /**
     * @dev Release vested tokens to the beneficiary
     * @param scheduleId The ID of the vesting schedule
     */
    function release(uint256 scheduleId) external scheduleExists(scheduleId) {
        VestingSchedule storage schedule = vestingSchedules[scheduleId];

        require(!schedule.revoked, "Vesting schedule has been revoked");
        require(
            msg.sender == schedule.beneficiary,
            "Only beneficiary can release tokens"
        );

        uint256 toRelease = releasableAmount(scheduleId);
        require(toRelease > 0, "No tokens available for release");

        schedule.releasedAmount += toRelease;

        (bool success, ) = payable(schedule.beneficiary).call{value: toRelease}(
            ""
        );
        require(success, "Transfer failed");

        emit TokensReleased(scheduleId, schedule.beneficiary, toRelease);
    }

    /**
     * @dev Revoke a vesting schedule (returns unvested tokens to owner)
     * @param scheduleId The ID of the vesting schedule
     */
    function revoke(
        uint256 scheduleId
    ) external onlyOwner scheduleExists(scheduleId) {
        VestingSchedule storage schedule = vestingSchedules[scheduleId];

        require(!schedule.revoked, "Vesting schedule already revoked");

        uint256 vested = vestedAmount(scheduleId);
        uint256 unvested = schedule.totalAmount - vested;

        schedule.revoked = true;

        if (unvested > 0) {
            (bool success, ) = payable(owner).call{value: unvested}("");
            require(success, "Transfer failed");
        }

        emit VestingRevoked(scheduleId, schedule.beneficiary, unvested);
    }

    /**
     * @dev Emergency withdraw all funds (only owner)
     */
    function emergencyWithdraw() external onlyOwner {
        uint256 balance = address(this).balance;
        require(balance > 0, "No funds to withdraw");

        (bool success, ) = payable(owner).call{value: balance}("");
        require(success, "Transfer failed");

        emit EmergencyWithdraw(owner, balance);
    }

    /**
     * @dev Get all schedule IDs for a beneficiary
     * @param beneficiary The beneficiary address
     * @return Array of schedule IDs
     */
    function getSchedulesByBeneficiary(
        address beneficiary
    ) external view returns (uint256[] memory) {
        return beneficiarySchedules[beneficiary];
    }

    /**
     * @dev Get detailed schedule information
     * @param scheduleId The ID of the vesting schedule
     */
    function getScheduleDetails(
        uint256 scheduleId
    )
        external
        view
        scheduleExists(scheduleId)
        returns (
            address beneficiary,
            uint256 totalAmount,
            uint256 startTime,
            uint256 cliffDuration,
            uint256 vestingDuration,
            uint256 releasedAmount,
            uint256 vestedAmountNow,
            uint256 releasableAmountNow,
            bool revoked
        )
    {
        VestingSchedule storage schedule = vestingSchedules[scheduleId];
        return (
            schedule.beneficiary,
            schedule.totalAmount,
            schedule.startTime,
            schedule.cliffDuration,
            schedule.vestingDuration,
            schedule.releasedAmount,
            vestedAmount(scheduleId),
            releasableAmount(scheduleId),
            schedule.revoked
        );
    }

    /**
     * @dev Get contract balance
     */
    function getContractBalance() external view returns (uint256) {
        return address(this).balance;
    }
}
