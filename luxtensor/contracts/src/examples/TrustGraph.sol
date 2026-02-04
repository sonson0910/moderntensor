// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import "../libraries/LuxTensorAI.sol";
import "../interfaces/ISemanticRegistry.sol";
import "../interfaces/ILuxTensorAI.sol";

/**
 * @title TrustGraph - Social Trust Scoring System
 * @notice Semantic-based reputation and trust classification
 * @dev Real-world case study: Cross-protocol reputation with AI classification
 *
 * Use Cases:
 * - DeFi: Under-collateralized lending based on reputation
 * - DAO: Voting weight based on trust score
 * - Marketplace: Buyer/seller trust verification
 * - Social: Content creator authenticity
 *
 * Architecture:
 * 1. User behavior is encoded into reputation vectors
 * 2. Vectors are registered in Social domain
 * 3. Trust classification uses k-NN against labeled profiles
 * 4. Cross-protocol queries enable portable reputation
 *
 * Trust Levels:
 * - 0: Unknown (new or insufficient data)
 * - 1: Risky (flagged behavior patterns)
 * - 2: Neutral (normal behavior)
 * - 3: Trusted (positive behavior patterns)
 * - 4: Verified (exceptional reputation)
 */
contract TrustGraph is Ownable, ReentrancyGuard {
    // ==================== ENUMS ====================

    /// @notice Trust classification levels
    enum TrustLevel {
        Unknown, // 0 - New user, insufficient data
        Risky, // 1 - Flagged patterns
        Neutral, // 2 - Normal behavior
        Trusted, // 3 - Good reputation
        Verified // 4 - Exceptional
    }

    // ==================== STRUCTS ====================

    /// @notice User trust profile
    struct TrustProfile {
        uint64 vectorId; // Global ID in Social domain
        TrustLevel level; // Current classification
        uint256 confidence; // Classification confidence (0-1e18)
        uint256 positiveActions; // Count of positive signals
        uint256 negativeActions; // Count of negative signals
        uint256 lastUpdated; // Timestamp
        bool isVerified; // Manual verification status
    }

    /// @notice Trust attestation from another protocol
    struct Attestation {
        address attester; // Protocol that attested
        TrustLevel attestedLevel; // Attested trust level
        uint256 weight; // Attestation weight
        uint256 timestamp;
        string reason;
    }

    /// @notice Reference profile for classification
    struct ReferenceProfile {
        uint64 vectorId;
        TrustLevel level;
        uint256 weight;
    }

    // ==================== STATE ====================

    /// @notice User profiles
    mapping(address => TrustProfile) public profiles;

    /// @notice Attestations per user
    mapping(address => Attestation[]) public attestations;

    /// @notice Approved attesters (protocols that can attest)
    mapping(address => bool) public approvedAttesters;

    /// @notice Reference profiles for classification (per trust level)
    mapping(TrustLevel => ReferenceProfile[]) public referenceProfiles;

    /// @notice Embedding dimension
    uint256 public embeddingDimension;

    /// @notice Minimum confidence for valid classification
    uint256 public minConfidence;

    /// @notice Attestation weight thresholds
    uint256 public constant WEIGHT_THRESHOLD = 1000;

    // ==================== EVENTS ====================

    event ProfileCreated(address indexed user, uint64 vectorId);

    event TrustClassified(
        address indexed user,
        TrustLevel level,
        uint256 confidence
    );

    event AttestationReceived(
        address indexed user,
        address indexed attester,
        TrustLevel level
    );

    event TrustLevelUpgraded(
        address indexed user,
        TrustLevel from,
        TrustLevel to
    );

    event TrustLevelDowngraded(
        address indexed user,
        TrustLevel from,
        TrustLevel to
    );

    event ReferenceProfileAdded(TrustLevel level, uint64 vectorId);

    // ==================== ERRORS ====================

    error ProfileNotFound(address user);
    error InvalidConfidence(uint256 confidence);
    error UnauthorizedAttester(address attester);
    error InsufficientData(address user);
    error AlreadyVerified(address user);
    error MaxAttestationsReached(address user);

    // ==================== CONSTANTS ====================

    /// @notice Maximum attestations per user to prevent Gas DoS
    /// @dev CR-12 security fix
    uint256 public constant MAX_ATTESTATIONS = 100;

    // ==================== CONSTRUCTOR ====================

    /**
     * @notice Initialize TrustGraph
     * @param dimension_ Reputation embedding dimension
     * @param minConfidence_ Minimum classification confidence
     */
    constructor(
        uint256 dimension_,
        uint256 minConfidence_
    ) Ownable(msg.sender) {
        embeddingDimension = dimension_;
        minConfidence = minConfidence_;
    }

    // ==================== PROFILE MANAGEMENT ====================

    /**
     * @notice Register a user's trust profile
     * @param user User address
     * @param behaviorEmbedding Initial behavior embedding
     * @param tags Profile tags
     * @return vectorId Global vector ID
     *
     * @dev Example:
     * ```solidity
     * // Protocol registers user after observing behavior
     * int256[] memory embedding = encodeUserBehavior(
     *     txCount,
     *     avgTxValue,
     *     protocolInteractions,
     *     accountAge
     * );
     * bytes32[] memory tags = [keccak256("defi_user")];
     * trustGraph.registerProfile(user, embedding, tags);
     * ```
     */
    function registerProfile(
        address user,
        int256[] calldata behaviorEmbedding,
        bytes32[] calldata tags
    ) external nonReentrant returns (uint64 vectorId) {
        require(
            behaviorEmbedding.length == embeddingDimension,
            "Invalid dimension"
        );
        require(profiles[user].vectorId == 0, "Profile exists");

        // Register in Social domain
        (vectorId, ) = LuxTensorAI.registerVector(
            ISemanticRegistry.SemanticDomain.Social,
            behaviorEmbedding,
            tags,
            0 // Permanent
        );

        profiles[user] = TrustProfile({
            vectorId: vectorId,
            level: TrustLevel.Unknown,
            confidence: 0,
            positiveActions: 0,
            negativeActions: 0,
            lastUpdated: block.timestamp,
            isVerified: false
        });

        emit ProfileCreated(user, vectorId);
    }

    /**
     * @notice Classify user's trust level using AI
     * @param user User to classify
     * @return level Classified trust level
     * @return confidence Classification confidence
     *
     * @dev Uses k-NN classification against reference profiles
     */
    function classifyTrust(
        address user
    ) external nonReentrant returns (TrustLevel level, uint256 confidence) {
        TrustProfile storage profile = profiles[user];
        if (profile.vectorId == 0) revert ProfileNotFound(user);

        // Get user's embedding
        (bool exists, int256[] memory embedding) = LuxTensorAI.semanticRelate(
            profile.vectorId
        );
        require(exists, "Vector not found");

        // Build labeled vectors from reference profiles
        uint256 totalRefs = _countReferenceProfiles();
        if (totalRefs < 3) revert InsufficientData(user);

        ILuxTensorAI.LabeledVector[] memory labels = _buildLabeledVectors();

        // Classify using k-NN
        uint32 classLabel;
        (classLabel, confidence) = LuxTensorAI.classify(embedding, labels, 5);

        level = TrustLevel(classLabel);

        // Update profile if confidence is sufficient
        if (confidence >= minConfidence) {
            TrustLevel oldLevel = profile.level;
            profile.level = level;
            profile.confidence = confidence;
            profile.lastUpdated = block.timestamp;

            emit TrustClassified(user, level, confidence);

            if (level > oldLevel) {
                emit TrustLevelUpgraded(user, oldLevel, level);
            } else if (level < oldLevel) {
                emit TrustLevelDowngraded(user, oldLevel, level);
            }
        }
    }

    /**
     * @notice Get trust level for a user (view only)
     * @param user User address
     * @return level Current trust level
     * @return confidence Classification confidence
     * @return isStale True if profile needs reclassification
     */
    function getTrustLevel(
        address user
    )
        external
        view
        returns (TrustLevel level, uint256 confidence, bool isStale)
    {
        TrustProfile storage profile = profiles[user];
        if (profile.vectorId == 0) {
            return (TrustLevel.Unknown, 0, true);
        }

        level = profile.level;
        confidence = profile.confidence;

        // Consider stale if older than 7 days
        isStale = block.timestamp > profile.lastUpdated + 7 days;
    }

    /**
     * @notice Check if user meets trust requirement
     * @param user User to check
     * @param minLevel Minimum required trust level
     * @return meets True if user meets requirement
     * @return actual User's actual trust level
     */
    function meetsTrustRequirement(
        address user,
        TrustLevel minLevel
    ) external view returns (bool meets, TrustLevel actual) {
        TrustProfile storage profile = profiles[user];
        if (profile.vectorId == 0) {
            return (minLevel == TrustLevel.Unknown, TrustLevel.Unknown);
        }

        actual = profile.level;
        meets = uint8(actual) >= uint8(minLevel);
    }

    // ==================== ATTESTATIONS ====================

    /**
     * @notice Attest to a user's trust level (protocol-to-protocol)
     * @param user User to attest
     * @param level Attested trust level
     * @param weight Attestation weight
     * @param reason Reason for attestation
     *
     * @dev Only approved attesters can call this
     *
     * Example:
     * ```solidity
     * // DeFi protocol attests after successful loan repayment
     * trustGraph.attest(borrower, TrustLevel.Trusted, 100, "Repaid 5 loans");
     * ```
     */
    function attest(
        address user,
        TrustLevel level,
        uint256 weight,
        string calldata reason
    ) external nonReentrant {
        if (!approvedAttesters[msg.sender]) {
            revert UnauthorizedAttester(msg.sender);
        }

        TrustProfile storage profile = profiles[user];
        if (profile.vectorId == 0) revert ProfileNotFound(user);
        // CR-12: Prevent unbounded attestation arrays
        if (attestations[user].length >= MAX_ATTESTATIONS)
            revert MaxAttestationsReached(user);

        attestations[user].push(
            Attestation({
                attester: msg.sender,
                attestedLevel: level,
                weight: weight,
                timestamp: block.timestamp,
                reason: reason
            })
        );

        // Update action counts based on attestation
        if (level >= TrustLevel.Trusted) {
            profile.positiveActions += weight;
        } else if (level == TrustLevel.Risky) {
            profile.negativeActions += weight;
        }

        emit AttestationReceived(user, msg.sender, level);
    }

    /**
     * @notice Get aggregated attestation score
     * @param user User address
     * @return totalWeight Total attestation weight
     * @return avgLevel Average attested level (scaled by 1e18)
     */
    function getAttestationScore(
        address user
    ) external view returns (uint256 totalWeight, uint256 avgLevel) {
        Attestation[] storage userAttestations = attestations[user];
        if (userAttestations.length == 0) return (0, 0);

        uint256 weightedSum = 0;
        for (uint256 i = 0; i < userAttestations.length; i++) {
            totalWeight += userAttestations[i].weight;
            weightedSum +=
                uint256(userAttestations[i].attestedLevel) *
                userAttestations[i].weight;
        }

        avgLevel = (weightedSum * 1e18) / totalWeight;
    }

    // ==================== REFERENCE MANAGEMENT ====================

    /**
     * @notice Add reference profile for classification
     * @param level Trust level this reference represents
     * @param embedding Reference behavior embedding
     * @param weight Reference weight for classification
     */
    function addReferenceProfile(
        TrustLevel level,
        int256[] calldata embedding,
        uint256 weight
    ) external onlyOwner returns (uint64 vectorId) {
        require(embedding.length == embeddingDimension, "Invalid dimension");

        // Register reference in Social domain
        bytes32[] memory tags = new bytes32[](1);
        tags[0] = keccak256(abi.encode("reference", uint8(level)));

        (vectorId, ) = LuxTensorAI.registerVector(
            ISemanticRegistry.SemanticDomain.Social,
            embedding,
            tags,
            0
        );

        referenceProfiles[level].push(
            ReferenceProfile({vectorId: vectorId, level: level, weight: weight})
        );

        emit ReferenceProfileAdded(level, vectorId);
    }

    // ==================== ADMIN ====================

    /**
     * @notice Approve an attester protocol
     * @param attester Protocol address
     */
    function approveAttester(address attester) external onlyOwner {
        approvedAttesters[attester] = true;
    }

    /**
     * @notice Revoke attester approval
     * @param attester Protocol address
     */
    function revokeAttester(address attester) external onlyOwner {
        approvedAttesters[attester] = false;
    }

    /**
     * @notice Manually verify a user
     * @param user User to verify
     */
    function verifyUser(address user) external onlyOwner {
        TrustProfile storage profile = profiles[user];
        if (profile.vectorId == 0) revert ProfileNotFound(user);
        if (profile.isVerified) revert AlreadyVerified(user);

        profile.isVerified = true;
        profile.level = TrustLevel.Verified;
        profile.confidence = 1e18; // 100% confidence

        emit TrustClassified(user, TrustLevel.Verified, 1e18);
    }

    /**
     * @notice Update minimum confidence threshold
     * @param newMinConfidence New threshold (0-1e18)
     */
    function setMinConfidence(uint256 newMinConfidence) external onlyOwner {
        require(newMinConfidence <= 1e18, "Invalid confidence");
        minConfidence = newMinConfidence;
    }

    // ==================== INTERNAL ====================

    /**
     * @dev Count total reference profiles
     */
    function _countReferenceProfiles() internal view returns (uint256 total) {
        for (uint8 i = 0; i <= uint8(TrustLevel.Verified); i++) {
            total += referenceProfiles[TrustLevel(i)].length;
        }
    }

    /**
     * @dev Build labeled vectors array for classification
     */
    function _buildLabeledVectors()
        internal
        view
        returns (ILuxTensorAI.LabeledVector[] memory labels)
    {
        uint256 total = _countReferenceProfiles();
        labels = new ILuxTensorAI.LabeledVector[](total);

        uint256 idx = 0;
        for (uint8 i = 0; i <= uint8(TrustLevel.Verified); i++) {
            ReferenceProfile[] storage refs = referenceProfiles[TrustLevel(i)];
            for (uint256 j = 0; j < refs.length; j++) {
                labels[idx] = ILuxTensorAI.LabeledVector({
                    vectorId: refs[j].vectorId,
                    label: uint32(i)
                });
                idx++;
            }
        }
    }

    // ==================== VIEW ====================

    /**
     * @notice Get user's full profile
     * @param user User address
     * @return Profile data
     */
    function getProfile(
        address user
    ) external view returns (TrustProfile memory) {
        return profiles[user];
    }

    /**
     * @notice Get attestations for a user
     * @param user User address
     * @return Array of attestations
     */
    function getAttestations(
        address user
    ) external view returns (Attestation[] memory) {
        return attestations[user];
    }

    /**
     * @notice Check if address is approved attester
     * @param attester Address to check
     * @return True if approved
     */
    function isApprovedAttester(address attester) external view returns (bool) {
        return approvedAttesters[attester];
    }
}
