// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "../interfaces/ILuxTensorAI.sol";
import "../interfaces/ISemanticRegistry.sol";

/**
 * @title LuxTensorAI - Library for LuxTensor AI Precompile Calls
 * @notice Provides low-level access to AI primitives and semantic registry
 * @dev Wraps raw precompile calls with proper ABI encoding/decoding
 *
 * Usage:
 * ```solidity
 * using LuxTensorAI for int256[];
 *
 * function checkFraud(int256[] calldata embedding) external view {
 *     (uint256 score, bool isAnomaly) = embedding.anomalyScore();
 *     if (isAnomaly) revert FraudDetected();
 * }
 * ```
 */
library LuxTensorAI {
    // ==================== PRECOMPILE ADDRESSES ====================

    /// @notice CLASSIFY precompile (0x22)
    address internal constant CLASSIFY = address(0x22);

    /// @notice CLUSTER_ASSIGN precompile (0x23)
    address internal constant CLUSTER_ASSIGN = address(0x23);

    /// @notice ANOMALY_SCORE precompile (0x24)
    address internal constant ANOMALY_SCORE = address(0x24);

    /// @notice SIMILARITY_GATE precompile (0x25)
    address internal constant SIMILARITY_GATE = address(0x25);

    /// @notice SEMANTIC_RELATE precompile (0x26)
    address internal constant SEMANTIC_RELATE = address(0x26);

    /// @notice REGISTER_VECTOR precompile (0x27)
    address internal constant REGISTER_VECTOR = address(0x27);

    /// @notice GLOBAL_SEARCH precompile (0x28)
    address internal constant GLOBAL_SEARCH = address(0x28);

    // ==================== GAS CONSTANTS ====================

    uint256 internal constant CLASSIFY_BASE_GAS = 25000;
    uint256 internal constant CLASSIFY_PER_LABEL_GAS = 100;
    uint256 internal constant CLUSTER_BASE_GAS = 28000;
    uint256 internal constant ANOMALY_BASE_GAS = 30000;
    uint256 internal constant SIMILARITY_BASE_GAS = 10000;
    uint256 internal constant RELATE_BASE_GAS = 20000;
    uint256 internal constant REGISTER_BASE_GAS = 35000;
    uint256 internal constant SEARCH_BASE_GAS = 40000;

    // ==================== ERRORS ====================

    error PrecompileCallFailed(address precompile);
    error InvalidVectorLength();
    error InsufficientGas();

    // ==================== AI PRIMITIVES (0x22-0x26) ====================

    /**
     * @notice Classify a vector using k-NN
     * @param query Query vector
     * @param labels Labeled vectors for classification
     * @param k Number of neighbors
     * @return label Predicted label
     * @return confidence Confidence score (0-1e18)
     */
    function classify(
        int256[] memory query,
        ILuxTensorAI.LabeledVector[] memory labels,
        uint256 k
    ) internal view returns (uint32 label, uint256 confidence) {
        uint256 gasRequired = CLASSIFY_BASE_GAS +
            (labels.length * CLASSIFY_PER_LABEL_GAS);
        if (gasleft() < gasRequired) revert InsufficientGas();

        bytes memory input = abi.encode(query, labels, k);

        (bool success, bytes memory result) = CLASSIFY.staticcall{
            gas: gasRequired
        }(input);
        if (!success) revert PrecompileCallFailed(CLASSIFY);

        (label, confidence) = abi.decode(result, (uint32, uint256));
    }

    /**
     * @notice Assign vector to nearest cluster
     * @param query Query vector
     * @param centroidIds Cluster centroid IDs
     * @return centroidId Nearest centroid ID
     * @return distance Distance to centroid
     */
    function clusterAssign(
        int256[] memory query,
        uint64[] memory centroidIds
    ) internal view returns (uint64 centroidId, uint256 distance) {
        uint256 gasRequired = CLUSTER_BASE_GAS + (centroidIds.length * 50);
        if (gasleft() < gasRequired) revert InsufficientGas();

        bytes memory input = abi.encode(query, centroidIds);

        (bool success, bytes memory result) = CLUSTER_ASSIGN.staticcall{
            gas: gasRequired
        }(input);
        if (!success) revert PrecompileCallFailed(CLUSTER_ASSIGN);

        (centroidId, distance) = abi.decode(result, (uint64, uint256));
    }

    /**
     * @notice Calculate anomaly score
     * @param query Query vector
     * @return score Anomaly score (0-1e18, higher = more anomalous)
     * @return isAnomaly True if score > 0.7e18
     */
    function anomalyScore(
        int256[] memory query
    ) internal view returns (uint256 score, bool isAnomaly) {
        if (gasleft() < ANOMALY_BASE_GAS) revert InsufficientGas();

        bytes memory input = abi.encode(query);

        (bool success, bytes memory result) = ANOMALY_SCORE.staticcall{
            gas: ANOMALY_BASE_GAS
        }(input);
        if (!success) revert PrecompileCallFailed(ANOMALY_SCORE);

        (score, isAnomaly) = abi.decode(result, (uint256, bool));
    }

    /**
     * @notice Check similarity between two vectors
     * @param vectorA First vector
     * @param vectorB Second vector
     * @param threshold Minimum similarity (0-1e18)
     * @return passed True if similarity >= threshold
     * @return similarity Actual similarity score
     */
    function similarityGate(
        int256[] memory vectorA,
        int256[] memory vectorB,
        uint256 threshold
    ) internal view returns (bool passed, uint256 similarity) {
        if (vectorA.length != vectorB.length) revert InvalidVectorLength();
        if (gasleft() < SIMILARITY_BASE_GAS) revert InsufficientGas();

        bytes memory input = abi.encode(vectorA, vectorB, threshold);

        (bool success, bytes memory result) = SIMILARITY_GATE.staticcall{
            gas: SIMILARITY_BASE_GAS
        }(input);
        if (!success) revert PrecompileCallFailed(SIMILARITY_GATE);

        (passed, similarity) = abi.decode(result, (bool, uint256));
    }

    /**
     * @notice Retrieve vector by ID for cross-contract sharing
     * @param vectorId Vector ID
     * @return exists True if vector exists
     * @return vector The vector data
     */
    function semanticRelate(
        uint64 vectorId
    ) internal view returns (bool exists, int256[] memory vector) {
        if (gasleft() < RELATE_BASE_GAS) revert InsufficientGas();

        bytes memory input = abi.encode(vectorId);

        (bool success, bytes memory result) = SEMANTIC_RELATE.staticcall{
            gas: RELATE_BASE_GAS
        }(input);
        if (!success) revert PrecompileCallFailed(SEMANTIC_RELATE);

        (exists, vector) = abi.decode(result, (bool, int256[]));
    }

    // ==================== WORLD SEMANTIC INDEX (0x27-0x28) ====================

    /**
     * @notice Register vector in global registry
     * @param domain Semantic domain
     * @param vector Vector data
     * @param tags Discovery tags
     * @param ttlBlocks Time-to-live (0 = permanent)
     * @return globalId Assigned global ID
     * @return remainingQuota Remaining registration quota
     */
    function registerVector(
        ISemanticRegistry.SemanticDomain domain,
        int256[] memory vector,
        bytes32[] memory tags,
        uint64 ttlBlocks
    ) internal returns (uint64 globalId, uint256 remainingQuota) {
        uint256 gasRequired = REGISTER_BASE_GAS +
            (vector.length * 50) +
            (tags.length * 200);
        if (gasleft() < gasRequired) revert InsufficientGas();

        bytes memory input = abi.encode(uint8(domain), vector, tags, ttlBlocks);

        (bool success, bytes memory result) = REGISTER_VECTOR.call{
            gas: gasRequired
        }(input);
        if (!success) revert PrecompileCallFailed(REGISTER_VECTOR);

        (globalId, remainingQuota) = abi.decode(result, (uint64, uint256));
    }

    /**
     * @notice Search for similar vectors globally
     * @param query Query vector
     * @param k Maximum results
     * @return ids Vector IDs
     * @return scores Distance scores
     * @return domains Source domains
     */
    function globalSearch(
        int256[] memory query,
        uint256 k
    )
        internal
        view
        returns (
            uint64[] memory ids,
            uint256[] memory scores,
            ISemanticRegistry.SemanticDomain[] memory domains
        )
    {
        uint256 gasRequired = SEARCH_BASE_GAS + (4 * 5000); // 4 domains
        if (gasleft() < gasRequired) revert InsufficientGas();

        bytes memory input = abi.encode(query, k);

        (bool success, bytes memory result) = GLOBAL_SEARCH.staticcall{
            gas: gasRequired
        }(input);
        if (!success) revert PrecompileCallFailed(GLOBAL_SEARCH);

        (ids, scores, domains) = abi.decode(
            result,
            (uint64[], uint256[], ISemanticRegistry.SemanticDomain[])
        );
    }

    // ==================== UTILITY FUNCTIONS ====================

    /**
     * @notice Convert float to fixed-point int256
     * @param value Float value (e.g., 0.5 = 5e17)
     * @return Fixed-point representation
     */
    function toFixedPoint(uint256 value) internal pure returns (int256) {
        return int256(value);
    }

    /**
     * @notice Estimate gas for classify operation
     * @param labelCount Number of labels
     * @return Estimated gas
     */
    function estimateClassifyGas(
        uint256 labelCount
    ) internal pure returns (uint256) {
        return CLASSIFY_BASE_GAS + (labelCount * CLASSIFY_PER_LABEL_GAS);
    }

    /**
     * @notice Estimate gas for register operation
     * @param vectorLen Vector dimension
     * @param tagCount Number of tags
     * @return Estimated gas
     */
    function estimateRegisterGas(
        uint256 vectorLen,
        uint256 tagCount
    ) internal pure returns (uint256) {
        return REGISTER_BASE_GAS + (vectorLen * 50) + (tagCount * 200);
    }
}
