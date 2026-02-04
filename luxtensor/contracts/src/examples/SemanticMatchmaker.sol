// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import "../libraries/LuxTensorAI.sol";
import "../interfaces/ISemanticRegistry.sol";
import "../interfaces/ILuxTensorAI.sol";

/**
 * @title SemanticMatchmaker - Gaming Player Matchmaking
 * @notice Match players based on semantic skill/playstyle embeddings
 * @dev Real-world case study: Cross-game matchmaking with World Semantic Index
 *
 * Use Cases:
 * - Battle Royale: Match players by skill level and playstyle
 * - MMO: Form balanced raid parties
 * - Esports: Create fair tournament brackets
 * - Cross-game: Portable player reputation
 *
 * Architecture:
 * 1. Players register profiles (skill embeddings) in Gaming domain
 * 2. Matchmaking queries global registry for similar players
 * 3. Cluster assignment ensures balanced teams
 * 4. Match quality is tracked for feedback loop
 */
contract SemanticMatchmaker is Ownable, ReentrancyGuard {
    // ==================== CONSTANTS ====================

    /// @notice Maximum players per match
    uint256 public constant MAX_PLAYERS_PER_MATCH = 100;

    /// @notice Minimum similarity for valid match (0.3 = 30%)
    uint256 public constant MIN_SIMILARITY = 3e17;

    /// @notice Maximum similarity for balanced match (0.9 = 90%)
    uint256 public constant MAX_SIMILARITY = 9e17;

    /// @notice Profile update cooldown (blocks)
    uint256 public constant PROFILE_COOLDOWN = 100;

    // ==================== STRUCTS ====================

    /// @notice Player profile
    struct PlayerProfile {
        uint64 vectorId; // Global ID in World Semantic Index
        uint256 skillRating; // ELO-style rating
        uint256 matchesPlayed;
        uint256 lastUpdated; // Block number
        bool isActive;
    }

    /// @notice Match information
    struct Match {
        uint256 matchId;
        address[] players;
        uint64[] profileIds;
        uint256 averageRating;
        uint256 createdAt;
        MatchStatus status;
    }

    /// @notice Match status
    enum MatchStatus {
        Pending,
        InProgress,
        Completed,
        Cancelled
    }

    /// @notice Matchmaking request
    struct MatchRequest {
        address player;
        uint64 profileId;
        uint256 requestedAt;
        uint256 minPlayers;
        uint256 maxPlayers;
    }

    // ==================== STATE ====================

    /// @notice Player profiles by address
    mapping(address => PlayerProfile) public profiles;

    /// @notice Matches by ID
    mapping(uint256 => Match) public matches;

    /// @notice Pending match requests (queue)
    MatchRequest[] public matchQueue;

    /// @notice Match counter
    uint256 public nextMatchId;

    /// @notice Profile embedding dimension
    uint256 public embeddingDimension;

    // ==================== EVENTS ====================

    event ProfileRegistered(
        address indexed player,
        uint64 vectorId,
        uint256 initialRating
    );

    event ProfileUpdated(address indexed player, uint64 newVectorId);

    event MatchRequested(address indexed player, uint256 queuePosition);

    event MatchCreated(
        uint256 indexed matchId,
        address[] players,
        uint256 averageRating
    );

    event MatchCompleted(uint256 indexed matchId, address winner);

    // ==================== ERRORS ====================

    error ProfileNotFound(address player);
    error ProfileCooldown(uint256 unlocksAt);
    error AlreadyInQueue(address player);
    error InvalidPlayerCount(uint256 min, uint256 max);
    error NoSuitableMatch();
    error MatchNotFound(uint256 matchId);

    // ==================== CONSTRUCTOR ====================

    /**
     * @notice Initialize matchmaker
     * @param dimension_ Profile embedding dimension
     */
    constructor(uint256 dimension_) Ownable(msg.sender) {
        embeddingDimension = dimension_;
        nextMatchId = 1;
    }

    // ==================== PROFILE MANAGEMENT ====================

    /**
     * @notice Register player profile with skill embedding
     * @param embedding Skill/playstyle embedding vector
     * @param tags Profile tags (e.g., "aggressive", "support")
     * @return vectorId Global vector ID
     *
     * @dev Example:
     * ```solidity
     * // Player registers after playing 10 games
     * int256[] memory skillVector = calculateSkillVector(gameHistory);
     * bytes32[] memory tags = [keccak256("shooter"), keccak256("aggressive")];
     * uint64 profileId = matchmaker.registerProfile(skillVector, tags);
     * ```
     */
    function registerProfile(
        int256[] calldata embedding,
        bytes32[] calldata tags
    ) external nonReentrant returns (uint64 vectorId) {
        require(embedding.length == embeddingDimension, "Invalid dimension");
        require(profiles[msg.sender].vectorId == 0, "Profile exists");

        // Register in Gaming domain
        (vectorId, ) = LuxTensorAI.registerVector(
            ISemanticRegistry.SemanticDomain.Gaming,
            embedding,
            tags,
            0 // Permanent profile
        );

        // Create profile
        profiles[msg.sender] = PlayerProfile({
            vectorId: vectorId,
            skillRating: 1000, // Starting ELO
            matchesPlayed: 0,
            lastUpdated: block.number,
            isActive: true
        });

        emit ProfileRegistered(msg.sender, vectorId, 1000);
    }

    /**
     * @notice Update player profile with new embedding
     * @param embedding New skill embedding
     * @param tags New tags
     * @return newVectorId New global vector ID
     */
    function updateProfile(
        int256[] calldata embedding,
        bytes32[] calldata tags
    ) external nonReentrant returns (uint64 newVectorId) {
        PlayerProfile storage profile = profiles[msg.sender];
        if (profile.vectorId == 0) revert ProfileNotFound(msg.sender);

        // Check cooldown
        if (block.number < profile.lastUpdated + PROFILE_COOLDOWN) {
            revert ProfileCooldown(profile.lastUpdated + PROFILE_COOLDOWN);
        }

        require(embedding.length == embeddingDimension, "Invalid dimension");

        // Register new vector (old one remains for historical matching)
        (newVectorId, ) = LuxTensorAI.registerVector(
            ISemanticRegistry.SemanticDomain.Gaming,
            embedding,
            tags,
            0
        );

        profile.vectorId = newVectorId;
        profile.lastUpdated = block.number;

        emit ProfileUpdated(msg.sender, newVectorId);
    }

    // ==================== MATCHMAKING ====================

    /**
     * @notice Request matchmaking
     * @param minPlayers Minimum players for match
     * @param maxPlayers Maximum players for match
     * @return queuePosition Position in queue
     *
     * @dev Example:
     * ```solidity
     * // Player wants a 2v2 match
     * uint256 pos = matchmaker.requestMatch(4, 4);
     * ```
     */
    function requestMatch(
        uint256 minPlayers,
        uint256 maxPlayers
    ) external nonReentrant returns (uint256 queuePosition) {
        PlayerProfile storage profile = profiles[msg.sender];
        if (profile.vectorId == 0) revert ProfileNotFound(msg.sender);
        if (
            minPlayers < 2 ||
            maxPlayers > MAX_PLAYERS_PER_MATCH ||
            minPlayers > maxPlayers
        ) {
            revert InvalidPlayerCount(minPlayers, maxPlayers);
        }

        // Check not already in queue
        for (uint256 i = 0; i < matchQueue.length; i++) {
            if (matchQueue[i].player == msg.sender) {
                revert AlreadyInQueue(msg.sender);
            }
        }

        matchQueue.push(
            MatchRequest({
                player: msg.sender,
                profileId: profile.vectorId,
                requestedAt: block.timestamp,
                minPlayers: minPlayers,
                maxPlayers: maxPlayers
            })
        );

        queuePosition = matchQueue.length;
        emit MatchRequested(msg.sender, queuePosition);

        // Try to form a match
        _tryFormMatch();
    }

    /**
     * @notice Find similar players for a given profile
     * @param player Player address
     * @param count Max results
     * @return similarPlayers Addresses of similar players
     * @return similarities Similarity scores
     */
    function findSimilarPlayers(
        address player,
        uint256 count
    )
        external
        view
        returns (address[] memory similarPlayers, uint256[] memory similarities)
    {
        PlayerProfile storage profile = profiles[player];
        if (profile.vectorId == 0) revert ProfileNotFound(player);

        // Get player's embedding
        (bool exists, int256[] memory embedding) = LuxTensorAI.semanticRelate(
            profile.vectorId
        );
        require(exists, "Profile vector not found");

        // Search for similar players in Gaming domain
        (uint64[] memory ids, uint256[] memory scores, ) = LuxTensorAI
            .globalSearch(embedding, count + 1); // +1 to exclude self

        // Filter results (exclude self and apply similarity bounds)
        uint256 validCount = 0;
        for (uint256 i = 0; i < ids.length; i++) {
            if (
                ids[i] != profile.vectorId &&
                scores[i] >= MIN_SIMILARITY &&
                scores[i] <= MAX_SIMILARITY
            ) {
                validCount++;
            }
        }

        similarPlayers = new address[](validCount);
        similarities = new uint256[](validCount);

        // This is simplified - in production, you'd need a vectorId -> address mapping
        // For now, return empty as the registry doesn't store owner addresses
    }

    /**
     * @notice Classify player into skill tier
     * @param player Player address
     * @return tier Skill tier (0=Bronze, 1=Silver, 2=Gold, 3=Platinum, 4=Diamond)
     * @return confidence Classification confidence
     */
    function classifyTier(
        address player
    ) external view returns (uint32 tier, uint256 confidence) {
        PlayerProfile storage profile = profiles[player];
        if (profile.vectorId == 0) revert ProfileNotFound(player);

        // Get player's embedding
        (bool exists, int256[] memory embedding) = LuxTensorAI.semanticRelate(
            profile.vectorId
        );
        require(exists, "Profile vector not found");

        // Use CLASSIFY precompile with tier centroids
        // In production, these would be pre-registered tier reference vectors
        ILuxTensorAI.LabeledVector[]
            memory tierLabels = new ILuxTensorAI.LabeledVector[](5);
        // tierLabels would be populated with tier centroid IDs

        (tier, confidence) = LuxTensorAI.classify(embedding, tierLabels, 3);
    }

    // ==================== INTERNAL FUNCTIONS ====================

    /**
     * @dev Attempt to form a match from queued players
     */
    function _tryFormMatch() internal {
        if (matchQueue.length < 2) return;

        // Simple greedy matching for demonstration
        // In production, use more sophisticated algorithms

        MatchRequest memory first = matchQueue[0];

        // Find compatible players
        address[] memory matchedPlayers = new address[](first.maxPlayers);
        uint64[] memory matchedIds = new uint64[](first.maxPlayers);
        uint256 matchedCount = 1;
        matchedPlayers[0] = first.player;
        matchedIds[0] = first.profileId;

        uint256[] memory toRemove = new uint256[](matchQueue.length);
        uint256 removeCount = 1;
        toRemove[0] = 0;

        for (
            uint256 i = 1;
            i < matchQueue.length && matchedCount < first.maxPlayers;
            i++
        ) {
            MatchRequest memory candidate = matchQueue[i];

            // Check compatibility (simplified)
            if (candidate.minPlayers <= first.maxPlayers) {
                matchedPlayers[matchedCount] = candidate.player;
                matchedIds[matchedCount] = candidate.profileId;
                matchedCount++;
                toRemove[removeCount++] = i;
            }
        }

        // Check if we have enough players
        if (matchedCount >= first.minPlayers) {
            // Trim arrays
            address[] memory finalPlayers = new address[](matchedCount);
            uint64[] memory finalIds = new uint64[](matchedCount);
            uint256 totalRating = 0;

            for (uint256 i = 0; i < matchedCount; i++) {
                finalPlayers[i] = matchedPlayers[i];
                finalIds[i] = matchedIds[i];
                totalRating += profiles[matchedPlayers[i]].skillRating;
            }

            // Create match
            uint256 matchId = nextMatchId++;
            matches[matchId] = Match({
                matchId: matchId,
                players: finalPlayers,
                profileIds: finalIds,
                averageRating: totalRating / matchedCount,
                createdAt: block.timestamp,
                status: MatchStatus.Pending
            });

            // Remove from queue (from end to avoid index shifting issues)
            for (uint256 i = removeCount; i > 0; i--) {
                _removeFromQueue(toRemove[i - 1]);
            }

            emit MatchCreated(
                matchId,
                finalPlayers,
                totalRating / matchedCount
            );
        }
    }

    /**
     * @dev Remove player from queue by index
     */
    function _removeFromQueue(uint256 index) internal {
        require(index < matchQueue.length, "Invalid index");
        matchQueue[index] = matchQueue[matchQueue.length - 1];
        matchQueue.pop();
    }

    // ==================== VIEW FUNCTIONS ====================

    /**
     * @notice Get player profile
     * @param player Player address
     * @return Profile data
     */
    function getProfile(
        address player
    ) external view returns (PlayerProfile memory) {
        return profiles[player];
    }

    /**
     * @notice Get queue length
     * @return Current queue size
     */
    function getQueueLength() external view returns (uint256) {
        return matchQueue.length;
    }

    /**
     * @notice Get match details
     * @param matchId Match ID
     * @return Match data
     */
    function getMatch(uint256 matchId) external view returns (Match memory) {
        return matches[matchId];
    }
}
