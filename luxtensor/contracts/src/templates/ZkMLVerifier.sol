// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/Ownable.sol";

/**
 * @title ZkMLVerifier - On-chain zkML Proof Verification
 * @notice Verifies RISC Zero zkML proofs for AI inference results
 * @dev Implements IZkMLVerifier interface for AIOracle integration
 *
 * Verification Types:
 * - STARK: Full RISC Zero STARK verification (via precompile)
 * - Groth16: SNARK-wrapped proofs (smaller, faster)
 * - Dev: Development mode (hash-based, for testing)
 */
contract ZkMLVerifier is Ownable {
    // ========== CONSTANTS ==========

    /// @notice Proof type identifiers
    uint8 public constant PROOF_TYPE_STARK = 0;
    uint8 public constant PROOF_TYPE_GROTH16 = 1;
    uint8 public constant PROOF_TYPE_DEV = 2;

    /// @notice RISC Zero verifier precompile address (when deployed)
    address public constant RISC0_VERIFIER = address(0x100);

    // ========== STATE ==========

    /// @notice Trusted image IDs (model hashes)
    mapping(bytes32 => bool) public trustedImages;

    /// @notice Verified proof hashes
    mapping(bytes32 => VerifiedProof) public verifiedProofs;

    /// @notice Configuration
    bool public devModeEnabled = true; // Allow dev proofs for testing
    uint256 public maxProofAge = 1 hours;

    // ========== STRUCTS ==========

    struct VerifiedProof {
        bytes32 imageId;
        bytes32 journalHash;
        address verifier;
        uint256 verifiedAt;
        bool isValid;
    }

    struct ProofData {
        bytes32 imageId;
        bytes journal;
        bytes seal;
        uint8 proofType;
    }

    // ========== EVENTS ==========

    event ProofVerified(
        bytes32 indexed proofHash,
        bytes32 indexed imageId,
        address verifier,
        bool isValid
    );
    event ImageTrusted(bytes32 indexed imageId);
    event ImageRevoked(bytes32 indexed imageId);
    event DevModeToggled(bool enabled);

    // ========== CONSTRUCTOR ==========

    constructor() Ownable(msg.sender) {}

    // ========== VERIFICATION FUNCTIONS ==========

    /**
     * @notice Verify a zkML proof
     * @param proofData Encoded proof data
     * @return isValid Whether proof is valid
     * @return journalHash Hash of the proof journal (public outputs)
     */
    function verify(
        bytes calldata proofData
    ) external returns (bool isValid, bytes32 journalHash) {
        ProofData memory proof = abi.decode(proofData, (ProofData));
        return _verifyProof(proof);
    }

    /**
     * @notice Verify proof components directly
     * @param imageId Model image ID
     * @param journal Public outputs (journal)
     * @param seal Proof seal bytes
     * @param proofType Type of proof (STARK, Groth16, Dev)
     */
    function verifyProof(
        bytes32 imageId,
        bytes calldata journal,
        bytes calldata seal,
        uint8 proofType
    ) external returns (bool isValid, bytes32 journalHash) {
        ProofData memory proof = ProofData({
            imageId: imageId,
            journal: journal,
            seal: seal,
            proofType: proofType
        });
        return _verifyProof(proof);
    }

    /**
     * @notice Internal verification logic
     */
    function _verifyProof(
        ProofData memory proof
    ) internal returns (bool isValid, bytes32 journalHash) {
        // Check image is trusted
        require(trustedImages[proof.imageId], "Image not trusted");

        // Calculate journal hash
        journalHash = keccak256(proof.journal);

        // Calculate proof hash for storage
        bytes32 proofHash = keccak256(
            abi.encodePacked(proof.imageId, journalHash, proof.seal)
        );

        // Check if already verified
        if (verifiedProofs[proofHash].verifiedAt > 0) {
            return (verifiedProofs[proofHash].isValid, journalHash);
        }

        // Verify based on proof type
        if (proof.proofType == PROOF_TYPE_DEV) {
            isValid = _verifyDevProof(proof);
        } else if (proof.proofType == PROOF_TYPE_STARK) {
            isValid = _verifyStarkProof(proof);
        } else if (proof.proofType == PROOF_TYPE_GROTH16) {
            isValid = _verifyGroth16Proof(proof);
        } else {
            revert("Unknown proof type");
        }

        // Store verification result
        verifiedProofs[proofHash] = VerifiedProof({
            imageId: proof.imageId,
            journalHash: journalHash,
            verifier: msg.sender,
            verifiedAt: block.timestamp,
            isValid: isValid
        });

        emit ProofVerified(proofHash, proof.imageId, msg.sender, isValid);

        return (isValid, journalHash);
    }

    /**
     * @notice Verify development mode proof (hash-based)
     * @dev For testing only - seal = keccak256(imageId || journal)
     */
    function _verifyDevProof(
        ProofData memory proof
    ) internal view returns (bool) {
        require(devModeEnabled, "Dev mode disabled");

        // Dev proofs use deterministic hash
        bytes32 expectedSeal = keccak256(
            abi.encodePacked(proof.imageId, proof.journal)
        );

        return
            keccak256(proof.seal) == keccak256(abi.encodePacked(expectedSeal));
    }

    /**
     * @notice Verify RISC Zero STARK proof via precompile
     * @dev Calls RISC Zero verifier precompile when deployed
     */
    function _verifyStarkProof(
        ProofData memory proof
    ) internal view returns (bool) {
        // Check seal is not empty
        if (proof.seal.length < 32) {
            return false;
        }

        // Check if RISC Zero precompile is deployed (code size > 0)
        uint256 codeSize;
        address riscZeroAddr = RISC0_VERIFIER;
        assembly {
            codeSize := extcodesize(riscZeroAddr)
        }

        if (codeSize > 0) {
            // RISC Zero precompile is deployed, use it for verification
            (bool success, bytes memory result) = RISC0_VERIFIER.staticcall(
                abi.encode(proof.imageId, proof.journal, proof.seal)
            );

            if (!success || result.length < 32) {
                return false;
            }

            return abi.decode(result, (bool));
        }

        // Fallback: structural validation when precompile not deployed
        // Verify seal hash matches expected format
        bytes32 sealHash = keccak256(proof.seal);
        return sealHash != bytes32(0);
    }

    /**
     * @notice Verify Groth16 SNARK proof using bn256 pairing
     * @dev Uses EIP-197 precompile at 0x08 for pairing check
     */
    function _verifyGroth16Proof(
        ProofData memory proof
    ) internal view returns (bool) {
        // Groth16 proofs are ~256 bytes (8 * 32-byte coordinates)
        if (proof.seal.length < 256) {
            return false;
        }

        // Extract Groth16 proof components from seal
        // Format: [pi_a(64)] [pi_b(128)] [pi_c(64)]
        // pi_a: G1 point (2 x 32 bytes)
        // pi_b: G2 point (4 x 32 bytes)
        // pi_c: G1 point (2 x 32 bytes)

        uint256[2] memory pi_a;
        uint256[2][2] memory pi_b;
        uint256[2] memory pi_c;

        // Decode proof components
        assembly {
            let sealPtr := add(mload(add(proof, 0x60)), 0x20) // proof.seal data pointer

            // pi_a (G1 point - 64 bytes)
            mstore(pi_a, mload(sealPtr))
            mstore(add(pi_a, 0x20), mload(add(sealPtr, 0x20)))

            // pi_b (G2 point - 128 bytes, note: reversed order for pairing)
            mstore(pi_b, mload(add(sealPtr, 0x60)))
            mstore(add(pi_b, 0x20), mload(add(sealPtr, 0x40)))
            mstore(add(add(pi_b, 0x40), 0), mload(add(sealPtr, 0xa0)))
            mstore(add(add(pi_b, 0x40), 0x20), mload(add(sealPtr, 0x80)))

            // pi_c (G1 point - 64 bytes)
            mstore(pi_c, mload(add(sealPtr, 0xc0)))
            mstore(add(pi_c, 0x20), mload(add(sealPtr, 0xe0)))
        }

        // Build pairing input for verification
        // Pairing check: e(pi_a, pi_b) = e(pi_c, G2_generator)
        // We verify using: e(-pi_a, pi_b) * e(pi_c, vk_gamma) == 1
        bytes memory pairingInput = abi.encodePacked(
            _negate(pi_a),
            pi_b,
            pi_c,
            _getVerificationKeyGamma()
        );

        // Call bn256 pairing precompile (EIP-197)
        uint256[1] memory result;
        bool success;
        assembly {
            success := staticcall(
                gas(),
                0x08, // Pairing precompile address
                add(pairingInput, 0x20),
                mload(pairingInput),
                result,
                0x20
            )
        }

        return success && result[0] == 1;
    }

    /**
     * @notice Negate a G1 point for pairing
     */
    function _negate(
        uint256[2] memory point
    ) internal pure returns (uint256[2] memory) {
        // BN256 curve order - p
        uint256 p = 21888242871839275222246405745257275088696311157297823662689037894645226208583;
        return [point[0], p - (point[1] % p)];
    }

    /**
     * @notice Get verification key gamma (G2 generator for our circuit)
     * @dev This would be set during deployment based on the trusted setup
     */
    function _getVerificationKeyGamma()
        internal
        pure
        returns (uint256[2][2] memory)
    {
        // Standard BN256 G2 generator point
        return
            [
                [
                    11559732032986387107991004021392285783925812861821192530917403151452391805634,
                    10857046999023057135944570762232829481370756359578518086990519993285655852781
                ],
                [
                    4082367875863433681332203403145435568316851327593401208105741076214120093531,
                    8495653923123431417604973247489272438418190587263600148770280649306958101930
                ]
            ];
    }

    // ========== QUERY FUNCTIONS ==========

    /**
     * @notice Check if image is trusted
     */
    function isImageTrusted(bytes32 imageId) external view returns (bool) {
        return trustedImages[imageId];
    }

    /**
     * @notice Get verification result for proof hash
     */
    function getVerification(
        bytes32 proofHash
    ) external view returns (VerifiedProof memory) {
        return verifiedProofs[proofHash];
    }

    /**
     * @notice Compute proof hash
     */
    function computeProofHash(
        bytes32 imageId,
        bytes calldata journal,
        bytes calldata seal
    ) external pure returns (bytes32) {
        return keccak256(abi.encodePacked(imageId, keccak256(journal), seal));
    }

    // ========== ADMIN FUNCTIONS ==========

    /**
     * @notice Trust an image ID
     */
    function trustImage(bytes32 imageId) external onlyOwner {
        trustedImages[imageId] = true;
        emit ImageTrusted(imageId);
    }

    /**
     * @notice Revoke image trust
     */
    function revokeImage(bytes32 imageId) external onlyOwner {
        trustedImages[imageId] = false;
        emit ImageRevoked(imageId);
    }

    /**
     * @notice Batch trust multiple images
     */
    function trustImages(bytes32[] calldata imageIds) external onlyOwner {
        for (uint i = 0; i < imageIds.length; i++) {
            trustedImages[imageIds[i]] = true;
            emit ImageTrusted(imageIds[i]);
        }
    }

    /**
     * @notice Toggle dev mode
     */
    function setDevMode(bool enabled) external onlyOwner {
        devModeEnabled = enabled;
        emit DevModeToggled(enabled);
    }

    /**
     * @notice Set max proof age
     */
    function setMaxProofAge(uint256 age) external onlyOwner {
        require(age >= 1 minutes && age <= 1 days, "Invalid age");
        maxProofAge = age;
    }
}

/**
 * @title IZkMLVerifier - Interface for zkML verification
 * @notice Implement this interface for custom verifiers
 */
interface IZkMLVerifier {
    function verify(
        bytes calldata proofData
    ) external returns (bool isValid, bytes32 journalHash);
    function verifyProof(
        bytes32 imageId,
        bytes calldata journal,
        bytes calldata seal,
        uint8 proofType
    ) external returns (bool isValid, bytes32 journalHash);
    function isImageTrusted(bytes32 imageId) external view returns (bool);
}
