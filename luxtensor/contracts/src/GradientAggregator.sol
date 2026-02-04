// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

/**
 * @title GradientAggregator
 * @notice Decentralized federated learning gradient aggregation on LuxTensor
 * @dev Implements FedAvg algorithm for on-chain training coordination
 *
 * Architecture:
 * 1. dApp creates training job with model, dataset, and rewards
 * 2. Miners register as trainers for the job
 * 3. Each round: trainers submit gradient hashes with PoT proofs
 * 4. Aggregation triggers round completion and reward distribution
 */
contract GradientAggregator is Ownable, ReentrancyGuard {
    using SafeERC20 for IERC20;

    // ==================== STATE ====================

    /// @notice Training job status lifecycle
    enum JobStatus {
        Open, // Accepting trainer registrations
        Training, // Active training rounds
        Aggregating, // Waiting for gradient aggregation
        Completed, // All rounds finished successfully
        Cancelled // Job cancelled by creator
    }

    /// @notice Training job configuration and state
    struct TrainingJob {
        bytes32 modelId; // IPFS CID of base model
        bytes32 datasetRef; // IPFS reference to dataset descriptor
        uint256 totalRounds; // Total training rounds
        uint256 currentRound; // Current round (0-indexed)
        uint256 minParticipants; // Minimum trainers per round
        uint256 rewardPerRound; // MDT reward per round
        address creator; // Job creator
        JobStatus status; // Current status
        uint256 createdAt; // Creation timestamp
        uint256 depositedRewards; // Total MDT deposited
    }

    /// @notice Gradient submission for a round
    struct GradientSubmission {
        address trainer; // Trainer address
        bytes32 gradientHash; // Hash of gradient vectors
        bytes32 checkpointHash; // Model checkpoint hash for PoT
        uint256 timestamp; // Submission time
        bool verified; // PoT verification status
    }

    /// @notice MDT token for rewards
    IERC20 public immutable mdtToken;

    /// @notice Job counter
    uint256 public nextJobId;

    /// @notice Jobs by ID
    mapping(uint256 => TrainingJob) public jobs;

    /// @notice Trainers registered for each job
    mapping(uint256 => address[]) public jobTrainers;

    /// @notice Is trainer registered for job
    mapping(uint256 => mapping(address => bool)) public isTrainer;

    /// @notice Gradient submissions per job per round
    mapping(uint256 => mapping(uint256 => GradientSubmission[]))
        public roundSubmissions;

    /// @notice Aggregated model checkpoints per job per round
    mapping(uint256 => mapping(uint256 => bytes32)) public roundCheckpoints;

    // ==================== EVENTS ====================

    event JobCreated(
        uint256 indexed jobId,
        bytes32 indexed modelId,
        address creator,
        uint256 totalRounds,
        uint256 rewardPerRound
    );

    event TrainerRegistered(uint256 indexed jobId, address indexed trainer);

    event TrainingStarted(uint256 indexed jobId, uint256 participantCount);

    event GradientSubmitted(
        uint256 indexed jobId,
        uint256 indexed round,
        address indexed trainer,
        bytes32 gradientHash
    );

    event RoundCompleted(
        uint256 indexed jobId,
        uint256 indexed round,
        bytes32 aggregatedCheckpoint,
        uint256 submissionCount
    );

    event JobCompleted(uint256 indexed jobId, uint256 totalRounds);

    event JobCancelled(uint256 indexed jobId, uint256 refundAmount);

    event RewardDistributed(
        uint256 indexed jobId,
        uint256 indexed round,
        address indexed trainer,
        uint256 amount
    );

    // ==================== ERRORS ====================

    error InvalidParameters();
    error InsufficientDeposit();
    error JobNotOpen();
    error JobNotTraining();
    error AlreadyRegistered();
    error NotRegistered();
    error RoundNotActive();
    error AlreadySubmitted();
    error InsufficientSubmissions();
    error NotJobCreator();
    error MaxParticipantsReached();

    // ==================== CONSTANTS ====================

    /// @notice Maximum participants per job to prevent Gas DoS
    /// @dev CR-10 security fix
    uint256 public constant MAX_PARTICIPANTS = 1000;

    // ==================== CONSTRUCTOR ====================

    constructor(address _mdtToken) Ownable(msg.sender) {
        if (_mdtToken == address(0)) revert InvalidParameters();
        mdtToken = IERC20(_mdtToken);
    }

    // ==================== JOB MANAGEMENT ====================

    /**
     * @notice Create a new training job
     * @param modelId IPFS CID of the base model
     * @param datasetRef IPFS reference to dataset
     * @param totalRounds Number of training rounds
     * @param minParticipants Minimum trainers per round
     * @param rewardPerRound MDT reward per round
     * @return jobId The created job ID
     */
    function createJob(
        bytes32 modelId,
        bytes32 datasetRef,
        uint256 totalRounds,
        uint256 minParticipants,
        uint256 rewardPerRound
    ) external nonReentrant returns (uint256 jobId) {
        if (modelId == bytes32(0) || totalRounds == 0 || minParticipants == 0) {
            revert InvalidParameters();
        }

        uint256 totalRewards = totalRounds * rewardPerRound;
        if (totalRewards == 0) revert InvalidParameters();

        // Transfer rewards from creator
        mdtToken.safeTransferFrom(msg.sender, address(this), totalRewards);

        jobId = nextJobId++;
        jobs[jobId] = TrainingJob({
            modelId: modelId,
            datasetRef: datasetRef,
            totalRounds: totalRounds,
            currentRound: 0,
            minParticipants: minParticipants,
            rewardPerRound: rewardPerRound,
            creator: msg.sender,
            status: JobStatus.Open,
            createdAt: block.timestamp,
            depositedRewards: totalRewards
        });

        emit JobCreated(
            jobId,
            modelId,
            msg.sender,
            totalRounds,
            rewardPerRound
        );
    }

    /**
     * @notice Register as a trainer for a job
     * @param jobId Job to register for
     * @dev CR-10: Added MAX_PARTICIPANTS check to prevent Gas DoS
     */
    function registerAsTrainer(uint256 jobId) external {
        TrainingJob storage job = jobs[jobId];
        if (job.status != JobStatus.Open) revert JobNotOpen();
        if (isTrainer[jobId][msg.sender]) revert AlreadyRegistered();
        // CR-10: Prevent unbounded participant arrays
        if (jobTrainers[jobId].length >= MAX_PARTICIPANTS)
            revert MaxParticipantsReached();

        isTrainer[jobId][msg.sender] = true;
        jobTrainers[jobId].push(msg.sender);

        emit TrainerRegistered(jobId, msg.sender);

        // Auto-start if minimum participants reached
        if (jobTrainers[jobId].length >= job.minParticipants) {
            job.status = JobStatus.Training;
            emit TrainingStarted(jobId, jobTrainers[jobId].length);
        }
    }

    // ==================== TRAINING ====================

    /**
     * @notice Submit gradient for current round
     * @param jobId Job ID
     * @param gradientHash Hash of gradient vectors (IPFS CID recommended)
     * @param checkpointHash Model checkpoint hash for PoT verification
     */
    function submitGradient(
        uint256 jobId,
        bytes32 gradientHash,
        bytes32 checkpointHash
    ) external nonReentrant {
        TrainingJob storage job = jobs[jobId];
        if (job.status != JobStatus.Training) revert JobNotTraining();
        if (!isTrainer[jobId][msg.sender]) revert NotRegistered();

        uint256 round = job.currentRound;
        GradientSubmission[] storage submissions = roundSubmissions[jobId][
            round
        ];

        // Check not already submitted
        for (uint256 i = 0; i < submissions.length; i++) {
            if (submissions[i].trainer == msg.sender) revert AlreadySubmitted();
        }

        submissions.push(
            GradientSubmission({
                trainer: msg.sender,
                gradientHash: gradientHash,
                checkpointHash: checkpointHash,
                timestamp: block.timestamp,
                verified: true // TODO: Implement PoT verification
            })
        );

        emit GradientSubmitted(jobId, round, msg.sender, gradientHash);

        // Check if round is complete
        if (submissions.length >= job.minParticipants) {
            _completeRound(jobId);
        }
    }

    /**
     * @dev Complete current round and distribute rewards
     */
    function _completeRound(uint256 jobId) internal {
        TrainingJob storage job = jobs[jobId];
        uint256 round = job.currentRound;
        GradientSubmission[] storage submissions = roundSubmissions[jobId][
            round
        ];

        // Calculate aggregated checkpoint (simplified: hash of all gradients)
        bytes32 aggregatedCheckpoint = _aggregateGradients(jobId, round);
        roundCheckpoints[jobId][round] = aggregatedCheckpoint;

        // Distribute rewards proportionally
        uint256 rewardPerTrainer = job.rewardPerRound / submissions.length;
        for (uint256 i = 0; i < submissions.length; i++) {
            if (submissions[i].verified) {
                mdtToken.safeTransfer(submissions[i].trainer, rewardPerTrainer);
                emit RewardDistributed(
                    jobId,
                    round,
                    submissions[i].trainer,
                    rewardPerTrainer
                );
            }
        }

        emit RoundCompleted(
            jobId,
            round,
            aggregatedCheckpoint,
            submissions.length
        );

        // Move to next round or complete
        job.currentRound++;
        if (job.currentRound >= job.totalRounds) {
            job.status = JobStatus.Completed;
            emit JobCompleted(jobId, job.totalRounds);
        }
    }

    /**
     * @dev Aggregate gradients using FedAvg (simplified on-chain version)
     * @return aggregatedHash Hash representing aggregated model state
     */
    function _aggregateGradients(
        uint256 jobId,
        uint256 round
    ) internal view returns (bytes32) {
        GradientSubmission[] storage submissions = roundSubmissions[jobId][
            round
        ];

        // Simple aggregation: keccak256 of all gradient hashes
        // Real FedAvg would happen off-chain with this as commitment
        bytes memory combined;
        for (uint256 i = 0; i < submissions.length; i++) {
            combined = abi.encodePacked(combined, submissions[i].gradientHash);
        }
        return keccak256(combined);
    }

    // ==================== JOB CONTROL ====================

    /**
     * @notice Cancel job and refund remaining rewards (creator only)
     * @param jobId Job to cancel
     */
    function cancelJob(uint256 jobId) external nonReentrant {
        TrainingJob storage job = jobs[jobId];
        if (msg.sender != job.creator) revert NotJobCreator();
        if (
            job.status == JobStatus.Completed ||
            job.status == JobStatus.Cancelled
        ) {
            revert InvalidParameters();
        }

        // Calculate remaining rewards
        uint256 completedRounds = job.currentRound;
        uint256 usedRewards = completedRounds * job.rewardPerRound;
        uint256 refundAmount = job.depositedRewards - usedRewards;

        job.status = JobStatus.Cancelled;

        if (refundAmount > 0) {
            mdtToken.safeTransfer(job.creator, refundAmount);
        }

        emit JobCancelled(jobId, refundAmount);
    }

    // ==================== VIEW FUNCTIONS ====================

    /**
     * @notice Get job details
     */
    function getJob(uint256 jobId) external view returns (TrainingJob memory) {
        return jobs[jobId];
    }

    /**
     * @notice Get trainers for a job
     */
    function getJobTrainers(
        uint256 jobId
    ) external view returns (address[] memory) {
        return jobTrainers[jobId];
    }

    /**
     * @notice Get submissions for a round
     */
    function getRoundSubmissions(
        uint256 jobId,
        uint256 round
    ) external view returns (GradientSubmission[] memory) {
        return roundSubmissions[jobId][round];
    }

    /**
     * @notice Get active jobs count
     */
    function getActiveJobsCount() external view returns (uint256 count) {
        for (uint256 i = 0; i < nextJobId; i++) {
            if (
                jobs[i].status == JobStatus.Open ||
                jobs[i].status == JobStatus.Training
            ) {
                count++;
            }
        }
    }
}
