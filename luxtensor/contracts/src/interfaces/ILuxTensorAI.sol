// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/**
 * @title ILuxTensorAI - Interface for LuxTensor AI Primitives
 * @notice Native AI operations accessible via precompiles (0x22-0x26)
 * @dev Each function maps to a specific precompile address
 *
 * Precompile Addresses:
 * - 0x22: CLASSIFY - k-NN classification with confidence
 * - 0x23: CLUSTER_ASSIGN - Assign to nearest cluster
 * - 0x24: ANOMALY_SCORE - Anomaly detection score
 * - 0x25: SIMILARITY_GATE - Semantic access control
 * - 0x26: SEMANTIC_RELATE - Cross-contract vector sharing
 */
interface ILuxTensorAI {
    // ==================== ERRORS ====================

    /// @notice Vector dimension mismatch
    error DimensionMismatch(uint256 expected, uint256 got);

    /// @notice Precompile call failed
    error PrecompileFailed(address precompile, bytes reason);

    /// @notice Vector not found in store
    error VectorNotFound(uint64 vectorId);

    /// @notice Insufficient gas for operation
    error InsufficientGas(uint256 required, uint256 provided);

    // ==================== EVENTS ====================

    /// @notice Emitted when classification is performed
    event Classification(
        address indexed caller,
        uint32 label,
        uint256 confidence
    );

    /// @notice Emitted when anomaly is detected
    event AnomalyDetected(
        address indexed caller,
        uint256 score,
        bool isAnomaly
    );

    /// @notice Emitted when similarity gate is checked
    event SimilarityChecked(
        address indexed caller,
        uint256 similarity,
        bool passed
    );

    // ==================== STRUCTS ====================

    /// @notice Labeled vector for classification
    struct LabeledVector {
        uint64 vectorId;
        uint32 label;
    }

    /// @notice Classification result
    struct ClassifyResult {
        uint32 label;
        uint256 confidence; // 0-1e18 (18 decimals)
    }

    /// @notice Cluster assignment result
    struct ClusterResult {
        uint64 centroidId;
        uint256 distance;
    }

    /// @notice Similarity check result
    struct SimilarityResult {
        bool passed;
        uint256 similarity; // 0-1e18 (18 decimals)
    }

    // ==================== VIEW FUNCTIONS ====================

    /**
     * @notice Classify a query vector using k-NN
     * @param query The query vector (fixed-point float32 array)
     * @param labels Array of labeled vectors for classification
     * @param k Number of neighbors to consider
     * @return result Classification result with label and confidence
     *
     * @dev Calls precompile at 0x22
     * Gas: ~25,000 base + 100 per label
     */
    function classify(
        int256[] calldata query,
        LabeledVector[] calldata labels,
        uint256 k
    ) external view returns (ClassifyResult memory result);

    /**
     * @notice Assign query to nearest cluster centroid
     * @param query The query vector
     * @param centroidIds Array of centroid vector IDs
     * @return result Nearest centroid ID and distance
     *
     * @dev Calls precompile at 0x23
     * Gas: ~28,000 base + 50 per centroid
     */
    function clusterAssign(
        int256[] calldata query,
        uint64[] calldata centroidIds
    ) external view returns (ClusterResult memory result);

    /**
     * @notice Calculate anomaly score for query vector
     * @param query The query vector
     * @return score Anomaly score (0-1e18, higher = more anomalous)
     * @return isAnomaly True if score exceeds default threshold (0.7)
     *
     * @dev Calls precompile at 0x24
     * Gas: ~30,000 base
     */
    function anomalyScore(
        int256[] calldata query
    ) external view returns (uint256 score, bool isAnomaly);

    /**
     * @notice Check semantic similarity between two vectors
     * @param vectorA First vector
     * @param vectorB Second vector
     * @param threshold Minimum similarity to pass (0-1e18)
     * @return result Similarity result with pass/fail and score
     *
     * @dev Calls precompile at 0x25
     * Gas: ~10,000 base
     */
    function similarityGate(
        int256[] calldata vectorA,
        int256[] calldata vectorB,
        uint256 threshold
    ) external view returns (SimilarityResult memory result);

    /**
     * @notice Retrieve a vector by ID for cross-contract sharing
     * @param vectorId The vector ID to retrieve
     * @return exists Whether the vector exists
     * @return vector The vector data (empty if not found)
     *
     * @dev Calls precompile at 0x26
     * Gas: ~20,000 base
     */
    function semanticRelate(
        uint64 vectorId
    ) external view returns (bool exists, int256[] memory vector);
}
