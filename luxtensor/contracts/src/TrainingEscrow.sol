// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

/**
 * @title TrainingEscrow
 * @notice Escrow contract for federated learning training rewards
 * @dev Manages deposits, slashing, and reward distribution for training jobs
 *
 * Economics:
 * - dApps deposit MDT for training jobs
 * - Trainers stake MDT to participate
 * - Rewards distributed per successful round
 * - Invalid proofs result in stake slashing
 */
contract TrainingEscrow is Ownable, ReentrancyGuard {
    using SafeERC20 for IERC20;

    // ==================== STATE ====================

    /// @notice MDT token
    IERC20 public immutable mdtToken;

    /// @notice GradientAggregator contract reference
    address public gradientAggregator;

    /// @notice Minimum stake required to participate
    uint256 public minStake = 100 ether; // 100 MDT

    /// @notice Slash percentage for invalid proofs (in basis points, 1000 = 10%)
    uint256 public slashPercentage = 1000;

    /// @notice Trainer stakes
    mapping(address => uint256) public stakes;

    /// @notice Total slashed amount (goes to insurance fund)
    uint256 public insuranceFund;

    /// @notice Job deposits
    mapping(uint256 => uint256) public jobDeposits;

    /// @notice Job spent amounts
    mapping(uint256 => uint256) public jobSpent;

    /// @notice Trainer reward claims
    mapping(address => mapping(uint256 => uint256)) public trainerClaimedRounds;

    // ==================== EVENTS ====================

    event Staked(address indexed trainer, uint256 amount);
    event Unstaked(address indexed trainer, uint256 amount);
    event JobFunded(
        uint256 indexed jobId,
        address indexed funder,
        uint256 amount
    );
    event RewardClaimed(
        uint256 indexed jobId,
        address indexed trainer,
        uint256 amount
    );
    event Slashed(address indexed trainer, uint256 amount, string reason);
    event InsurancePayout(address indexed recipient, uint256 amount);

    // ==================== ERRORS ====================

    error InsufficientStake();
    error InsufficientBalance();
    error NothingToClaim();
    error InvalidAggregator();
    error OnlyAggregator();

    // ==================== CONSTRUCTOR ====================

    constructor(address _mdtToken, address _aggregator) Ownable(msg.sender) {
        mdtToken = IERC20(_mdtToken);
        gradientAggregator = _aggregator;
    }

    // ==================== MODIFIERS ====================

    modifier onlyAggregator() {
        if (msg.sender != gradientAggregator) revert OnlyAggregator();
        _;
    }

    // ==================== STAKING ====================

    /**
     * @notice Stake MDT to become eligible for training
     * @param amount Amount to stake
     */
    function stake(uint256 amount) external nonReentrant {
        mdtToken.safeTransferFrom(msg.sender, address(this), amount);
        stakes[msg.sender] += amount;
        emit Staked(msg.sender, amount);
    }

    /**
     * @notice Unstake MDT (only available if not actively training)
     * @param amount Amount to unstake
     */
    function unstake(uint256 amount) external nonReentrant {
        if (stakes[msg.sender] < amount) revert InsufficientBalance();

        // TODO: Check if trainer is in active job

        stakes[msg.sender] -= amount;
        mdtToken.safeTransfer(msg.sender, amount);
        emit Unstaked(msg.sender, amount);
    }

    /**
     * @notice Check if address has sufficient stake
     */
    function hasMinStake(address trainer) external view returns (bool) {
        return stakes[trainer] >= minStake;
    }

    // ==================== JOB FUNDING ====================

    /**
     * @notice Fund a training job (called by dApps)
     * @param jobId Job to fund
     * @param amount MDT amount to deposit
     */
    function fundJob(uint256 jobId, uint256 amount) external nonReentrant {
        mdtToken.safeTransferFrom(msg.sender, address(this), amount);
        jobDeposits[jobId] += amount;
        emit JobFunded(jobId, msg.sender, amount);
    }

    /**
     * @notice Get remaining budget for a job
     */
    function getJobBudget(uint256 jobId) external view returns (uint256) {
        if (jobSpent[jobId] >= jobDeposits[jobId]) return 0;
        return jobDeposits[jobId] - jobSpent[jobId];
    }

    // ==================== REWARD DISTRIBUTION ====================

    /**
     * @notice Distribute reward to trainer (called by GradientAggregator)
     * @param jobId Job ID
     * @param trainer Trainer address
     * @param amount Reward amount
     */
    function distributeReward(
        uint256 jobId,
        address trainer,
        uint256 amount
    ) external onlyAggregator nonReentrant {
        if (jobDeposits[jobId] - jobSpent[jobId] < amount) {
            revert InsufficientBalance();
        }

        jobSpent[jobId] += amount;
        mdtToken.safeTransfer(trainer, amount);

        emit RewardClaimed(jobId, trainer, amount);
    }

    // ==================== SLASHING ====================

    /**
     * @notice Slash trainer stake for invalid proof (called by PoT verifier)
     * @param trainer Trainer to slash
     * @param reason Reason for slashing
     */
    function slashTrainer(
        address trainer,
        string calldata reason
    ) external onlyAggregator nonReentrant {
        uint256 stakeAmount = stakes[trainer];
        if (stakeAmount == 0) return;

        uint256 slashAmount = (stakeAmount * slashPercentage) / 10000;
        stakes[trainer] -= slashAmount;
        insuranceFund += slashAmount;

        emit Slashed(trainer, slashAmount, reason);
    }

    // ==================== ADMIN ====================

    /**
     * @notice Update GradientAggregator address
     */
    function setAggregator(address _aggregator) external onlyOwner {
        if (_aggregator == address(0)) revert InvalidAggregator();
        gradientAggregator = _aggregator;
    }

    /**
     * @notice Update minimum stake requirement
     */
    function setMinStake(uint256 _minStake) external onlyOwner {
        minStake = _minStake;
    }

    /**
     * @notice Update slash percentage
     */
    function setSlashPercentage(uint256 _percentage) external onlyOwner {
        require(_percentage <= 5000, "Max 50%"); // Cap at 50%
        slashPercentage = _percentage;
    }

    /**
     * @notice Pay out from insurance fund (emergency governance action)
     */
    function payoutInsurance(
        address recipient,
        uint256 amount
    ) external onlyOwner nonReentrant {
        if (amount > insuranceFund) revert InsufficientBalance();
        insuranceFund -= amount;
        mdtToken.safeTransfer(recipient, amount);
        emit InsurancePayout(recipient, amount);
    }

    // ==================== VIEW FUNCTIONS ====================

    /**
     * @notice Get trainer stake amount
     */
    function getStake(address trainer) external view returns (uint256) {
        return stakes[trainer];
    }

    /**
     * @notice Get insurance fund balance
     */
    function getInsuranceFund() external view returns (uint256) {
        return insuranceFund;
    }
}
