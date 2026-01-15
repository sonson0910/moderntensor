// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

/**
 * @title AIOracle - Decentralized AI Inference Oracle for LuxTensor
 * @notice Request AI computations and receive verified results on-chain
 * @dev Core component for AI-powered dApps on LuxTensor
 *
 * Features:
 * - Request AI inference from network miners
 * - Callback on completion
 * - zkML proof verification support
 * - Stake-based result selection
 * - Timeout and refund mechanism
 *
 * Flow:
 * 1. User calls requestAI() with payment
 * 2. Off-chain miners process request
 * 3. Miners submit result via fulfillRequest()
 * 4. Contract verifies and stores result
 * 5. User retrieves result
 */
contract AIOracle is Ownable, ReentrancyGuard {
    // ========== STRUCTS ==========

    struct AIRequest {
        address requester;
        bytes32 modelHash;
        bytes inputData;
        uint256 reward;
        uint256 createdAt;
        uint256 deadline;
        RequestStatus status;
        bytes result;
        address fulfiller;
        bytes32 proofHash;
    }

    enum RequestStatus {
        Pending,
        Fulfilled,
        Expired,
        Cancelled
    }

    // ========== STATE ==========

    /// @notice All AI requests
    mapping(bytes32 => AIRequest) public requests;

    /// @notice Request IDs by requester
    mapping(address => bytes32[]) public userRequests;

    /// @notice Registered AI models
    mapping(bytes32 => bool) public approvedModels;

    /// @notice Minimum stake to fulfill requests
    uint256 public minFulfillerStake;

    /// @notice Default request timeout (blocks)
    uint256 public defaultTimeout = 100; // ~10 minutes

    /// @notice Protocol fee (basis points, 100 = 1%)
    uint256 public protocolFeeBps = 100;

    /// @notice Accumulated fees
    uint256 public accumulatedFees;

    /// @notice Request counter
    uint256 public requestCount;

    /// @notice zkML verifier address
    address public zkmlVerifier;

    // ========== EVENTS ==========

    event AIRequestCreated(
        bytes32 indexed requestId,
        address indexed requester,
        bytes32 modelHash,
        uint256 reward
    );

    event AIRequestFulfilled(
        bytes32 indexed requestId,
        address indexed fulfiller,
        bytes32 proofHash
    );

    event AIRequestCancelled(bytes32 indexed requestId);
    event AIRequestExpired(bytes32 indexed requestId);
    event ModelApproved(bytes32 indexed modelHash);
    event ModelRevoked(bytes32 indexed modelHash);

    // ========== CONSTRUCTOR ==========

    constructor() Ownable(msg.sender) {}

    // ========== USER FUNCTIONS ==========

    /**
     * @notice Request AI inference
     * @param modelHash Hash of the AI model to use
     * @param inputData Input data for the model
     * @param timeout Custom timeout in blocks (0 = use default)
     * @return requestId Unique request identifier
     */
    function requestAI(
        bytes32 modelHash,
        bytes calldata inputData,
        uint256 timeout
    ) external payable nonReentrant returns (bytes32) {
        require(msg.value > 0, "Payment required");
        require(
            approvedModels[modelHash] || owner() == msg.sender,
            "Model not approved"
        );
        require(inputData.length > 0, "Empty input");
        require(inputData.length <= 10000, "Input too large");

        bytes32 requestId = keccak256(
            abi.encodePacked(
                msg.sender,
                modelHash,
                block.timestamp,
                requestCount++
            )
        );

        uint256 actualTimeout = timeout > 0 ? timeout : defaultTimeout;

        requests[requestId] = AIRequest({
            requester: msg.sender,
            modelHash: modelHash,
            inputData: inputData,
            reward: msg.value,
            createdAt: block.timestamp,
            deadline: block.number + actualTimeout,
            status: RequestStatus.Pending,
            result: new bytes(0),
            fulfiller: address(0),
            proofHash: bytes32(0)
        });

        userRequests[msg.sender].push(requestId);

        emit AIRequestCreated(requestId, msg.sender, modelHash, msg.value);

        return requestId;
    }

    /**
     * @notice Cancel pending request and get refund
     * @param requestId Request to cancel
     */
    function cancelRequest(bytes32 requestId) external nonReentrant {
        AIRequest storage req = requests[requestId];
        require(req.requester == msg.sender, "Not requester");
        require(req.status == RequestStatus.Pending, "Not pending");

        req.status = RequestStatus.Cancelled;

        payable(msg.sender).transfer(req.reward);

        emit AIRequestCancelled(requestId);
    }

    /**
     * @notice Get AI result
     * @param requestId Request ID
     * @return result The AI output
     * @return completed Whether request is completed
     */
    function getResult(
        bytes32 requestId
    ) external view returns (bytes memory result, bool completed) {
        AIRequest storage req = requests[requestId];
        return (req.result, req.status == RequestStatus.Fulfilled);
    }

    // ========== FULFILLER FUNCTIONS ==========

    /**
     * @notice Fulfill AI request (called by miners/validators)
     * @param requestId Request to fulfill
     * @param result AI output data
     * @param proofHash zkML proof hash (optional, for verification)
     */
    function fulfillRequest(
        bytes32 requestId,
        bytes calldata result,
        bytes32 proofHash
    ) external nonReentrant {
        AIRequest storage req = requests[requestId];
        require(req.status == RequestStatus.Pending, "Not pending");
        require(block.number <= req.deadline, "Request expired");
        require(result.length > 0, "Empty result");

        // TODO: Verify zkML proof if verifier is set
        // if (zkmlVerifier != address(0) && proofHash != bytes32(0)) {
        //     require(IZkMLVerifier(zkmlVerifier).verify(proofHash), "Invalid proof");
        // }

        req.status = RequestStatus.Fulfilled;
        req.result = result;
        req.fulfiller = msg.sender;
        req.proofHash = proofHash;

        // Calculate fee
        uint256 fee = (req.reward * protocolFeeBps) / 10000;
        uint256 payout = req.reward - fee;

        accumulatedFees += fee;
        payable(msg.sender).transfer(payout);

        emit AIRequestFulfilled(requestId, msg.sender, proofHash);
    }

    /**
     * @notice Mark expired requests
     * @param requestId Request to check
     */
    function markExpired(bytes32 requestId) external {
        AIRequest storage req = requests[requestId];
        require(req.status == RequestStatus.Pending, "Not pending");
        require(block.number > req.deadline, "Not expired yet");

        req.status = RequestStatus.Expired;

        // Refund requester
        payable(req.requester).transfer(req.reward);

        emit AIRequestExpired(requestId);
    }

    // ========== ADMIN FUNCTIONS ==========

    /**
     * @notice Approve AI model for requests
     * @param modelHash Model hash to approve
     */
    function approveModel(bytes32 modelHash) external onlyOwner {
        approvedModels[modelHash] = true;
        emit ModelApproved(modelHash);
    }

    /**
     * @notice Revoke model approval
     * @param modelHash Model hash to revoke
     */
    function revokeModel(bytes32 modelHash) external onlyOwner {
        approvedModels[modelHash] = false;
        emit ModelRevoked(modelHash);
    }

    /**
     * @notice Set zkML verifier address
     * @param _verifier Verifier contract address
     */
    function setZkMLVerifier(address _verifier) external onlyOwner {
        zkmlVerifier = _verifier;
    }

    /**
     * @notice Set protocol fee
     * @param _feeBps Fee in basis points (100 = 1%)
     */
    function setProtocolFee(uint256 _feeBps) external onlyOwner {
        require(_feeBps <= 1000, "Fee too high"); // Max 10%
        protocolFeeBps = _feeBps;
    }

    /**
     * @notice Set default timeout
     * @param _timeout Timeout in blocks
     */
    function setDefaultTimeout(uint256 _timeout) external onlyOwner {
        require(_timeout >= 10 && _timeout <= 10000, "Invalid timeout");
        defaultTimeout = _timeout;
    }

    /**
     * @notice Withdraw accumulated fees
     */
    function withdrawFees() external onlyOwner {
        uint256 amount = accumulatedFees;
        accumulatedFees = 0;
        payable(owner()).transfer(amount);
    }

    // ========== VIEW FUNCTIONS ==========

    /**
     * @notice Get user's request IDs
     * @param user User address
     * @return Array of request IDs
     */
    function getUserRequests(
        address user
    ) external view returns (bytes32[] memory) {
        return userRequests[user];
    }

    /**
     * @notice Check if model is approved
     * @param modelHash Model hash
     * @return Whether model is approved
     */
    function isModelApproved(bytes32 modelHash) external view returns (bool) {
        return approvedModels[modelHash];
    }

    /**
     * @notice Get request details
     * @param requestId Request ID
     * @return Full request struct
     */
    function getRequest(
        bytes32 requestId
    ) external view returns (AIRequest memory) {
        return requests[requestId];
    }
}
