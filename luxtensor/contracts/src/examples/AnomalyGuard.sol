// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import "@openzeppelin/contracts/utils/Pausable.sol";
import "../libraries/LuxTensorAI.sol";
import "../interfaces/ILuxTensorAI.sol";

/**
 * @title AnomalyGuard - DeFi Fraud Detection using Semantic AI
 * @notice Protect DeFi protocols from suspicious transactions using ML anomaly detection
 * @dev Real-world case study: Transaction validation with native AI primitives
 *
 * Use Cases:
 * - DEX: Block abnormal swap patterns (sandwich attacks, flash loan exploits)
 * - Lending: Detect suspicious collateral movements
 * - Bridge: Flag unusual cross-chain transfers
 *
 * Architecture:
 * 1. Transaction data is embedded into a semantic vector
 * 2. Vector is scored against historical patterns via ANOMALY_SCORE precompile
 * 3. High-risk transactions are paused for manual review
 * 4. Whitelisted patterns bypass checks
 */
contract AnomalyGuard is Ownable, ReentrancyGuard, Pausable {
    // ==================== CONSTANTS ====================

    /// @notice Default anomaly threshold (0.7 = 70%)
    uint256 public constant DEFAULT_THRESHOLD = 7e17;

    /// @notice Maximum embedding dimension
    uint256 public constant MAX_DIMENSION = 512;

    /// @notice Cooldown period after flagged transaction (blocks)
    uint256 public constant FLAG_COOLDOWN = 10;

    // ==================== STATE ====================

    /// @notice Current anomaly threshold (0-1e18)
    uint256 public anomalyThreshold;

    /// @notice Embedding dimension for transaction vectors
    uint256 public embeddingDimension;

    /// @notice Flagged addresses with cooldown
    mapping(address => uint256) public flaggedUntil;

    /// @notice Whitelisted transaction patterns (hash => approved)
    mapping(bytes32 => bool) public whitelistedPatterns;

    /// @notice Historical anomaly scores per address
    mapping(address => uint256[]) public scoreHistory;

    /// @notice Total transactions analyzed
    uint256 public totalAnalyzed;

    /// @notice Total anomalies detected
    uint256 public totalAnomalies;

    // ==================== EVENTS ====================

    event TransactionAnalyzed(
        address indexed sender,
        bytes32 indexed txHash,
        uint256 score,
        bool passed
    );

    event AnomalyFlagged(
        address indexed sender,
        bytes32 indexed txHash,
        uint256 score,
        uint256 cooldownUntil
    );

    event PatternWhitelisted(bytes32 indexed patternHash);
    event PatternRemoved(bytes32 indexed patternHash);
    event ThresholdUpdated(uint256 oldThreshold, uint256 newThreshold);

    // ==================== ERRORS ====================

    error TransactionBlocked(uint256 anomalyScore);
    error AddressFlagged(address flagged, uint256 cooldownUntil);
    error InvalidEmbedding(uint256 length, uint256 expected);
    error ThresholdOutOfRange(uint256 value);

    // ==================== CONSTRUCTOR ====================

    /**
     * @notice Initialize AnomalyGuard
     * @param _dimension Embedding dimension for transaction vectors
     * @param _threshold Initial anomaly threshold (0-1e18)
     */
    constructor(uint256 _dimension, uint256 _threshold) Ownable(msg.sender) {
        if (_dimension == 0 || _dimension > MAX_DIMENSION) {
            revert InvalidEmbedding(_dimension, MAX_DIMENSION);
        }
        if (_threshold > 1e18) revert ThresholdOutOfRange(_threshold);

        embeddingDimension = _dimension;
        anomalyThreshold = _threshold;
    }

    // ==================== CORE FUNCTIONS ====================

    /**
     * @notice Validate a transaction embedding for anomalies
     * @param embedding Transaction embedding vector
     * @param txHash Transaction hash for logging
     * @return passed True if transaction is safe
     * @return score Anomaly score (0-1e18)
     *
     * @dev This is the main entry point for DeFi protocols:
     *
     * Example integration with a DEX:
     * ```solidity
     * function swap(uint256 amountIn, uint256 minOut) external {
     *     int256[] memory embedding = _createSwapEmbedding(msg.sender, amountIn, minOut);
     *     (bool safe, ) = anomalyGuard.validateTransaction(embedding, keccak256(msg.data));
     *     require(safe, "Transaction flagged as suspicious");
     *     // ... proceed with swap
     * }
     * ```
     */
    function validateTransaction(
        int256[] calldata embedding,
        bytes32 txHash
    ) external whenNotPaused nonReentrant returns (bool passed, uint256 score) {
        // Check embedding dimension
        if (embedding.length != embeddingDimension) {
            revert InvalidEmbedding(embedding.length, embeddingDimension);
        }

        // Check if sender is in cooldown
        if (block.number < flaggedUntil[msg.sender]) {
            revert AddressFlagged(msg.sender, flaggedUntil[msg.sender]);
        }

        // Check whitelist
        bytes32 patternHash = keccak256(abi.encode(embedding));
        if (whitelistedPatterns[patternHash]) {
            totalAnalyzed++;
            emit TransactionAnalyzed(msg.sender, txHash, 0, true);
            return (true, 0);
        }

        // Call ANOMALY_SCORE precompile (0x24)
        bool isAnomaly;
        (score, isAnomaly) = LuxTensorAI.anomalyScore(embedding);

        totalAnalyzed++;

        // Store score in history
        scoreHistory[msg.sender].push(score);

        // Check against threshold
        passed = score < anomalyThreshold;

        if (!passed) {
            totalAnomalies++;
            flaggedUntil[msg.sender] = block.number + FLAG_COOLDOWN;

            emit AnomalyFlagged(
                msg.sender,
                txHash,
                score,
                flaggedUntil[msg.sender]
            );
            revert TransactionBlocked(score);
        }

        emit TransactionAnalyzed(msg.sender, txHash, score, passed);
    }

    /**
     * @notice Batch validate multiple transaction embeddings
     * @param embeddings Array of embeddings
     * @param txHashes Corresponding transaction hashes
     * @return passed Array of pass/fail results
     * @return scores Array of anomaly scores
     */
    function batchValidate(
        int256[][] calldata embeddings,
        bytes32[] calldata txHashes
    )
        external
        whenNotPaused
        nonReentrant
        returns (bool[] memory passed, uint256[] memory scores)
    {
        require(embeddings.length == txHashes.length, "Length mismatch");
        require(embeddings.length <= 10, "Batch too large");

        passed = new bool[](embeddings.length);
        scores = new uint256[](embeddings.length);

        for (uint256 i = 0; i < embeddings.length; i++) {
            (passed[i], scores[i]) = this.validateTransaction(
                embeddings[i],
                txHashes[i]
            );
        }
    }

    // ==================== VIEW FUNCTIONS ====================

    /**
     * @notice Get average anomaly score for an address
     * @param account Address to check
     * @return average Average score (0-1e18)
     */
    function getAverageScore(
        address account
    ) external view returns (uint256 average) {
        uint256[] storage history = scoreHistory[account];
        if (history.length == 0) return 0;

        uint256 sum = 0;
        for (uint256 i = 0; i < history.length; i++) {
            sum += history[i];
        }
        average = sum / history.length;
    }

    /**
     * @notice Check if address is currently flagged
     * @param account Address to check
     * @return flagged_ True if in cooldown
     * @return cooldownEnds Block number when cooldown ends
     */
    function isFlagged(
        address account
    ) external view returns (bool flagged_, uint256 cooldownEnds) {
        cooldownEnds = flaggedUntil[account];
        flagged_ = block.number < cooldownEnds;
    }

    /**
     * @notice Get protocol statistics
     * @return analyzed Total transactions analyzed
     * @return flagged Total anomalies detected
     * @return rate Detection rate (flagged/analyzed * 1e18)
     */
    function getStats()
        external
        view
        returns (uint256 analyzed, uint256 flagged, uint256 rate)
    {
        analyzed = totalAnalyzed;
        flagged = totalAnomalies;
        rate = analyzed > 0 ? (flagged * 1e18) / analyzed : 0;
    }

    // ==================== ADMIN FUNCTIONS ====================

    /**
     * @notice Update anomaly threshold
     * @param newThreshold New threshold (0-1e18)
     */
    function setThreshold(uint256 newThreshold) external onlyOwner {
        if (newThreshold > 1e18) revert ThresholdOutOfRange(newThreshold);

        uint256 oldThreshold = anomalyThreshold;
        anomalyThreshold = newThreshold;

        emit ThresholdUpdated(oldThreshold, newThreshold);
    }

    /**
     * @notice Whitelist a known safe pattern
     * @param embedding The pattern to whitelist
     */
    function whitelistPattern(int256[] calldata embedding) external onlyOwner {
        bytes32 patternHash = keccak256(abi.encode(embedding));
        whitelistedPatterns[patternHash] = true;
        emit PatternWhitelisted(patternHash);
    }

    /**
     * @notice Remove pattern from whitelist
     * @param patternHash Pattern hash to remove
     */
    function removePattern(bytes32 patternHash) external onlyOwner {
        whitelistedPatterns[patternHash] = false;
        emit PatternRemoved(patternHash);
    }

    /**
     * @notice Clear flagged status for an address
     * @param account Address to unflag
     */
    function unflag(address account) external onlyOwner {
        flaggedUntil[account] = 0;
    }

    /**
     * @notice Pause the guard (emergency)
     */
    function pause() external onlyOwner {
        _pause();
    }

    /**
     * @notice Unpause the guard
     */
    function unpause() external onlyOwner {
        _unpause();
    }
}
