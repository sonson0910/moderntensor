// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/**
 * @title ISemanticRegistry - Interface for World Semantic Index
 * @notice Global, domain-sharded vector registry for cross-contract composability
 * @dev Precompile addresses: 0x27 (register), 0x28 (search)
 *
 * The World Semantic Index enables:
 * - Cross-dApp vector discovery
 * - Domain-specific categorization
 * - Quota-based storage allocation
 * - Time-based vector expiry (TTL)
 */
interface ISemanticRegistry {
    // ==================== ENUMS ====================

    /// @notice Semantic domains for vector categorization
    enum SemanticDomain {
        General, // 0 - Default domain
        Finance, // 1 - DeFi, payments, risk
        Gaming, // 2 - Player profiles, items
        Social, // 3 - Content, reputation
        Identity, // 4 - Credentials, biometrics
        SupplyChain, // 5 - Products, logistics
        Healthcare, // 6 - Anonymized health data
        Custom // 255 - User-defined
    }

    // ==================== ERRORS ====================

    /// @notice Storage quota exceeded for address
    error QuotaExceeded(address owner, uint256 limit);

    /// @notice Vector dimension mismatch
    error DimensionMismatch(uint256 expected, uint256 got);

    /// @notice Vector not found
    error VectorNotFound(uint64 globalId);

    /// @notice Vector has expired
    error VectorExpired(uint64 globalId, uint256 expiredAt);

    /// @notice Precompile call failed
    error RegistryCallFailed(bytes reason);

    // ==================== EVENTS ====================

    /// @notice Emitted when vector is registered
    event VectorRegistered(
        uint64 indexed globalId,
        address indexed owner,
        SemanticDomain domain,
        bytes32[] tags
    );

    /// @notice Emitted when search is performed
    event GlobalSearchPerformed(
        address indexed searcher,
        uint256 resultsCount,
        SemanticDomain[] domains
    );

    // ==================== STRUCTS ====================

    /// @notice Vector metadata
    struct VectorMetadata {
        address owner;
        SemanticDomain domain;
        uint256 registeredAt;
        uint256 expiresAt; // 0 = permanent
        bytes32[] tags;
    }

    /// @notice Registration result
    struct RegisterResult {
        uint64 globalId;
        uint256 remainingQuota;
    }

    /// @notice Search result entry
    struct SearchResult {
        uint64 globalId;
        uint256 score; // Distance score (lower = closer)
        SemanticDomain domain;
    }

    // ==================== WRITE FUNCTIONS ====================

    /**
     * @notice Register a vector in the global semantic index
     * @param domain Semantic domain for categorization
     * @param vector The vector data (fixed-point float32)
     * @param tags Optional tags for discovery (max 10)
     * @param ttlBlocks Time-to-live in blocks (0 = permanent)
     * @return result Registration result with global ID and remaining quota
     *
     * @dev Calls precompile at 0x27
     * Gas: ~35,000 base + 50/dimension + 200/tag
     *
     * Example:
     * ```solidity
     * RegisterResult memory result = registry.registerVector(
     *     SemanticDomain.Finance,
     *     userProfileEmbedding,
     *     [keccak256("user"), keccak256("defi")],
     *     0 // Permanent
     * );
     * ```
     */
    function registerVector(
        SemanticDomain domain,
        int256[] calldata vector,
        bytes32[] calldata tags,
        uint64 ttlBlocks
    ) external returns (RegisterResult memory result);

    // ==================== VIEW FUNCTIONS ====================

    /**
     * @notice Search for similar vectors across all domains
     * @param query The query vector
     * @param k Maximum number of results
     * @return results Array of search results sorted by similarity
     *
     * @dev Calls precompile at 0x28
     * Gas: ~40,000 base + 5,000/domain searched
     *
     * Example:
     * ```solidity
     * SearchResult[] memory matches = registry.globalSearch(
     *     queryEmbedding,
     *     10 // Top 10 results
     * );
     * ```
     */
    function globalSearch(
        int256[] calldata query,
        uint256 k
    ) external view returns (SearchResult[] memory results);

    /**
     * @notice Search within a specific domain
     * @param domain Domain to search
     * @param query The query vector
     * @param k Maximum number of results
     * @return results Array of search results
     */
    function domainSearch(
        SemanticDomain domain,
        int256[] calldata query,
        uint256 k
    ) external view returns (SearchResult[] memory results);

    /**
     * @notice Get vector metadata by global ID
     * @param globalId The vector's global ID
     * @return metadata Vector metadata
     */
    function getMetadata(
        uint64 globalId
    ) external view returns (VectorMetadata memory metadata);

    /**
     * @notice Get remaining quota for an address
     * @param owner Address to check
     * @return remaining Number of vectors that can still be registered
     */
    function getRemainingQuota(
        address owner
    ) external view returns (uint256 remaining);

    /**
     * @notice Check if a vector has expired
     * @param globalId The vector's global ID
     * @return expired True if vector has expired
     */
    function isExpired(uint64 globalId) external view returns (bool expired);
}
