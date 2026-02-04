// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import "@openzeppelin/contracts/token/ERC721/ERC721.sol";
import "../libraries/LuxTensorAI.sol";
import "../interfaces/ISemanticRegistry.sol";

/**
 * @title ContentAuthenticator - NFT Originality Verification
 * @notice Prevent NFT plagiarism using semantic similarity detection
 * @dev Real-world case study: Content verification with World Semantic Index
 *
 * Use Cases:
 * - Art NFTs: Verify artwork originality before minting
 * - Music NFTs: Detect audio similarity/copying
 * - Photography: Prevent duplicate image minting
 *
 * Architecture:
 * 1. Original artwork embeddings are registered in World Semantic Index
 * 2. New uploads are checked against global registry via SIMILARITY_GATE
 * 3. Too-similar content is rejected with similarity score
 * 4. Verified originals get minted with on-chain provenance
 */
contract ContentAuthenticator is ERC721, Ownable, ReentrancyGuard {
    // ==================== CONSTANTS ====================

    /// @notice Default similarity threshold (0.85 = 85% similar = plagiarism)
    uint256 public constant DEFAULT_SIMILARITY_THRESHOLD = 85e16; // 0.85

    /// @notice Maximum embedding dimension
    uint256 public constant MAX_DIMENSION = 1024;

    // ==================== STATE ====================

    /// @notice Content embedding dimension
    uint256 public embeddingDimension;

    /// @notice Plagiarism similarity threshold
    uint256 public similarityThreshold;

    /// @notice Token ID counter
    uint256 private _nextTokenId;

    /// @notice Content hash to token ID
    mapping(bytes32 => uint256) public contentToToken;

    /// @notice Token ID to global vector ID in registry
    mapping(uint256 => uint64) public tokenToVectorId;

    /// @notice Token ID to content metadata
    mapping(uint256 => ContentMetadata) public metadata;

    /// @notice Struct for content metadata
    struct ContentMetadata {
        bytes32 contentHash; // IPFS CID or hash
        uint64 globalVectorId; // ID in World Semantic Index
        uint256 createdAt;
        address originalCreator;
        uint256 similarityScore; // 0 = fully original
    }

    // ==================== EVENTS ====================

    event ContentVerified(
        uint256 indexed tokenId,
        bytes32 indexed contentHash,
        uint64 globalVectorId,
        address creator
    );

    event PlagiarismDetected(
        bytes32 indexed contentHash,
        uint256 similarity,
        uint64 similarTo
    );

    event ThresholdUpdated(uint256 oldThreshold, uint256 newThreshold);

    // ==================== ERRORS ====================

    error ContentTooSimilar(uint256 similarity, uint64 similarTo);
    error InvalidEmbedding(uint256 got, uint256 expected);
    error ContentAlreadyMinted(bytes32 contentHash);
    error TokenNotFound(uint256 tokenId);

    // ==================== CONSTRUCTOR ====================

    /**
     * @notice Initialize ContentAuthenticator
     * @param name_ NFT collection name
     * @param symbol_ NFT collection symbol
     * @param dimension_ Embedding dimension for content vectors
     */
    constructor(
        string memory name_,
        string memory symbol_,
        uint256 dimension_
    ) ERC721(name_, symbol_) Ownable(msg.sender) {
        if (dimension_ == 0 || dimension_ > MAX_DIMENSION) {
            revert InvalidEmbedding(dimension_, MAX_DIMENSION);
        }
        embeddingDimension = dimension_;
        similarityThreshold = DEFAULT_SIMILARITY_THRESHOLD;
        _nextTokenId = 1;
    }

    // ==================== CORE FUNCTIONS ====================

    /**
     * @notice Mint NFT after verifying content originality
     * @param contentHash IPFS CID or content hash
     * @param embedding Content embedding vector
     * @param tags Discovery tags for the content
     * @return tokenId The minted token ID
     *
     * @dev Flow:
     * 1. Check embedding dimension
     * 2. Verify not already minted
     * 3. Search global registry for similar content
     * 4. If too similar, reject with similarity info
     * 5. Register in World Semantic Index
     * 6. Mint NFT with provenance data
     *
     * Example:
     * ```solidity
     * // Artist uploads artwork
     * bytes32 ipfsCid = 0x1234...;
     * int256[] memory artEmbedding = modelOutput; // From off-chain ML
     * bytes32[] memory tags = new bytes32[](2);
     * tags[0] = keccak256("art");
     * tags[1] = keccak256("landscape");
     *
     * uint256 tokenId = authenticator.mintVerified(ipfsCid, artEmbedding, tags);
     * ```
     */
    function mintVerified(
        bytes32 contentHash,
        int256[] calldata embedding,
        bytes32[] calldata tags
    ) external nonReentrant returns (uint256 tokenId) {
        // Validate embedding
        if (embedding.length != embeddingDimension) {
            revert InvalidEmbedding(embedding.length, embeddingDimension);
        }

        // Check not already minted
        if (contentToToken[contentHash] != 0) {
            revert ContentAlreadyMinted(contentHash);
        }

        // Search for similar content in global registry
        (
            uint64[] memory ids,
            uint256[] memory scores,
            ISemanticRegistry.SemanticDomain[] memory domains
        ) = LuxTensorAI.globalSearch(embedding, 1);

        // Check for plagiarism
        if (ids.length > 0 && scores[0] >= similarityThreshold) {
            emit PlagiarismDetected(contentHash, scores[0], ids[0]);
            revert ContentTooSimilar(scores[0], ids[0]);
        }

        // Register in World Semantic Index
        (uint64 globalId, ) = LuxTensorAI.registerVector(
            ISemanticRegistry.SemanticDomain.General,
            embedding,
            tags,
            0 // Permanent
        );

        // Mint NFT
        tokenId = _nextTokenId++;
        _safeMint(msg.sender, tokenId);

        // Store metadata
        contentToToken[contentHash] = tokenId;
        tokenToVectorId[tokenId] = globalId;

        uint256 similarityScore = ids.length > 0 ? scores[0] : 0;

        metadata[tokenId] = ContentMetadata({
            contentHash: contentHash,
            globalVectorId: globalId,
            createdAt: block.timestamp,
            originalCreator: msg.sender,
            similarityScore: similarityScore
        });

        emit ContentVerified(tokenId, contentHash, globalId, msg.sender);
    }

    /**
     * @notice Check content similarity without minting
     * @param embedding Content embedding to check
     * @return isOriginal True if sufficiently original
     * @return highestSimilarity Highest similarity score found
     * @return similarTo Most similar existing vector ID
     */
    function checkOriginality(
        int256[] calldata embedding
    )
        external
        view
        returns (bool isOriginal, uint256 highestSimilarity, uint64 similarTo)
    {
        if (embedding.length != embeddingDimension) {
            revert InvalidEmbedding(embedding.length, embeddingDimension);
        }

        (uint64[] memory ids, uint256[] memory scores, ) = LuxTensorAI
            .globalSearch(embedding, 1);

        if (ids.length == 0) {
            return (true, 0, 0);
        }

        highestSimilarity = scores[0];
        similarTo = ids[0];
        isOriginal = highestSimilarity < similarityThreshold;
    }

    /**
     * @notice Verify two content pieces are different
     * @param embeddingA First content
     * @param embeddingB Second content
     * @return areDifferent True if sufficiently different
     * @return similarity Similarity score between them
     */
    function compareContent(
        int256[] calldata embeddingA,
        int256[] calldata embeddingB
    ) external view returns (bool areDifferent, uint256 similarity) {
        (bool passed, uint256 score) = LuxTensorAI.similarityGate(
            embeddingA,
            embeddingB,
            similarityThreshold
        );

        // If passed = true, they are SIMILAR (above threshold)
        // We want areDifferent, so invert
        areDifferent = !passed;
        similarity = score;
    }

    // ==================== VIEW FUNCTIONS ====================

    /**
     * @notice Get content metadata for a token
     * @param tokenId Token ID
     * @return Content metadata
     */
    function getContentMetadata(
        uint256 tokenId
    ) external view returns (ContentMetadata memory) {
        if (!_exists(tokenId)) revert TokenNotFound(tokenId);
        return metadata[tokenId];
    }

    /**
     * @notice Get token ID for a content hash
     * @param contentHash Content hash to look up
     * @return tokenId Token ID (0 if not minted)
     */
    function getTokenByContent(
        bytes32 contentHash
    ) external view returns (uint256) {
        return contentToToken[contentHash];
    }

    /**
     * @notice Check if token exists
     * @param tokenId Token ID to check
     * @return exists True if token exists
     */
    function _exists(uint256 tokenId) internal view returns (bool) {
        return tokenId > 0 && tokenId < _nextTokenId;
    }

    /**
     * @notice Get total minted tokens
     * @return Total supply
     */
    function totalSupply() external view returns (uint256) {
        return _nextTokenId - 1;
    }

    // ==================== ADMIN FUNCTIONS ====================

    /**
     * @notice Update similarity threshold
     * @param newThreshold New threshold (0-1e18)
     */
    function setThreshold(uint256 newThreshold) external onlyOwner {
        require(newThreshold <= 1e18, "Threshold too high");

        uint256 oldThreshold = similarityThreshold;
        similarityThreshold = newThreshold;

        emit ThresholdUpdated(oldThreshold, newThreshold);
    }
}
