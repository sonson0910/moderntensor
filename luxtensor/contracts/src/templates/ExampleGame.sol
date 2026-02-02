// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "./AIOracle.sol";

/**
 * @title ExampleGame - Demo AI-powered dApp on LuxTensor
 * @notice A simple game that uses AI to generate random numbers and validate moves
 * @dev Example consumer of AIOracle for demonstration purposes
 *
 * Game Flow:
 * 1. Player requests an AI move by paying LUX
 * 2. AI Oracle receives request, runs inference
 * 3. Oracle submits verified result back to chain
 * 4. Game updates player state based on AI output
 */
contract ExampleGame {
    // ========== STATE ==========

    /// @notice Reference to AIOracle contract
    AIOracle public aiOracle;

    /// @notice Model hash for the game AI
    bytes32 public gameModelHash;

    /// @notice Player scores
    mapping(address => uint256) public scores;

    /// @notice Pending game requests
    mapping(bytes32 => address) public pendingGames;

    /// @notice Number of games played
    uint256 public totalGames;

    // ========== EVENTS ==========

    event GameRequested(address indexed player, bytes32 indexed requestId);
    event GameCompleted(
        address indexed player,
        uint256 score,
        bytes32 indexed requestId
    );
    event GameFailed(address indexed player, bytes32 indexed requestId);

    // ========== CONSTRUCTOR ==========

    /**
     * @param _aiOracle Address of deployed AIOracle contract
     * @param _modelHash Hash of the approved game AI model
     */
    constructor(address _aiOracle, bytes32 _modelHash) {
        aiOracle = AIOracle(_aiOracle);
        gameModelHash = _modelHash;
    }

    // ========== GAME FUNCTIONS ==========

    /**
     * @notice Start a new game (request AI move)
     * @param playerMove Player's chosen move (encoded as bytes)
     * @return requestId The AI request ID for tracking
     */
    function play(
        bytes calldata playerMove
    ) external payable returns (bytes32) {
        require(msg.value >= 0.001 ether, "Minimum stake required");

        // Format input for AI: [player_address, player_move, block_data]
        bytes memory aiInput = abi.encode(
            msg.sender,
            playerMove,
            block.number,
            block.timestamp
        );

        // Request AI inference
        bytes32 requestId = aiOracle.requestAI{value: msg.value}(
            gameModelHash,
            aiInput,
            0 // Default timeout
        );

        pendingGames[requestId] = msg.sender;
        totalGames++;

        emit GameRequested(msg.sender, requestId);
        return requestId;
    }

    /**
     * @notice Check game result and claim rewards
     * @param requestId The request ID from play()
     */
    function claimResult(bytes32 requestId) external {
        address player = pendingGames[requestId];
        require(player != address(0), "Unknown request");
        require(player == msg.sender, "Not your game");

        (bytes memory result, bool completed) = aiOracle.getResult(requestId);

        if (!completed) {
            revert("Game not completed");
        }

        // Parse AI result (assume format: [score, won, message])
        uint256 score = _parseScore(result);

        scores[player] += score;
        delete pendingGames[requestId];

        emit GameCompleted(player, score, requestId);
    }

    /**
     * @notice Get player's total score
     * @param player Player address
     * @return Total accumulated score
     */
    function getScore(address player) external view returns (uint256) {
        return scores[player];
    }

    /**
     * @notice Get game status
     * @param requestId Request ID
     * @return player The player address
     * @return completed Whether the game is completed
     */
    function getGameStatus(
        bytes32 requestId
    ) external view returns (address player, bool completed) {
        player = pendingGames[requestId];
        (, completed) = aiOracle.getResult(requestId);
    }

    // ========== INTERNAL ==========

    /**
     * @dev Parse score from AI result bytes
     * @param result Raw result bytes
     * @return Score value
     */
    function _parseScore(bytes memory result) internal pure returns (uint256) {
        // Simple parsing: first 32 bytes = score
        if (result.length < 32) {
            return 10; // Default score
        }
        return abi.decode(result, (uint256));
    }

    // ========== ADMIN ==========

    /**
     * @notice Update game model (only contract deployer can call)
     * @param newModelHash New model hash
     */
    function updateModel(bytes32 newModelHash) external {
        // In production, add proper access control
        gameModelHash = newModelHash;
    }
}
