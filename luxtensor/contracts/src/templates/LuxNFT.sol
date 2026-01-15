// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC721/ERC721.sol";
import "@openzeppelin/contracts/token/ERC721/extensions/ERC721URIStorage.sol";
import "@openzeppelin/contracts/token/ERC721/extensions/ERC721Burnable.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

/**
 * @title LuxNFT - AI-Powered NFT Template for LuxTensor
 * @notice NFT collection with AI-generated metadata support
 * @dev Deploy on LuxTensor network (chain ID: 1337 testnet, 777 mainnet)
 *
 * Features:
 * - ERC-721 standard compliant
 * - URI storage for AI-generated metadata
 * - Burnable tokens
 * - Mint with AI prompt support
 * - Royalty support (ERC-2981 ready)
 *
 * Use Case:
 * - AI-generated art NFTs
 * - Proof of AI computation NFTs
 * - AI model ownership NFTs
 */
contract LuxNFT is ERC721, ERC721URIStorage, ERC721Burnable, Ownable {
    /// @notice Current token ID counter
    uint256 private _nextTokenId;

    /// @notice Maximum supply (0 = unlimited)
    uint256 public maxSupply;

    /// @notice Mint price in MDT (wei)
    uint256 public mintPrice;

    /// @notice Base URI for metadata
    string private _baseTokenURI;

    /// @notice AI prompt used to generate each NFT
    mapping(uint256 => string) public tokenPrompts;

    /// @notice AI model used for generation
    mapping(uint256 => bytes32) public tokenModelHash;

    /// @notice Emitted when NFT is minted with AI
    event AIGenerated(
        uint256 indexed tokenId,
        address indexed owner,
        string prompt,
        bytes32 modelHash
    );

    /**
     * @notice Constructor
     * @param name Collection name (e.g., "AI Art Collection")
     * @param symbol Collection symbol (e.g., "AIART")
     * @param _maxSupply Maximum supply (0 for unlimited)
     * @param _mintPrice Mint price in MDT wei
     */
    constructor(
        string memory name,
        string memory symbol,
        uint256 _maxSupply,
        uint256 _mintPrice
    ) ERC721(name, symbol) Ownable(msg.sender) {
        maxSupply = _maxSupply;
        mintPrice = _mintPrice;
    }

    /**
     * @notice Mint new NFT
     * @param to Recipient address
     * @param uri Token metadata URI
     * @return tokenId The minted token ID
     */
    function mint(
        address to,
        string memory uri
    ) public payable returns (uint256) {
        if (mintPrice > 0) {
            require(msg.value >= mintPrice, "Insufficient payment");
        }

        if (maxSupply > 0) {
            require(_nextTokenId < maxSupply, "Max supply reached");
        }

        uint256 tokenId = _nextTokenId++;
        _safeMint(to, tokenId);
        _setTokenURI(tokenId, uri);

        return tokenId;
    }

    /**
     * @notice Mint NFT with AI generation info
     * @param to Recipient address
     * @param uri Token metadata URI
     * @param prompt AI prompt used for generation
     * @param modelHash Hash of AI model used
     * @return tokenId The minted token ID
     */
    function mintWithAI(
        address to,
        string memory uri,
        string memory prompt,
        bytes32 modelHash
    ) public payable returns (uint256) {
        uint256 tokenId = mint(to, uri);

        tokenPrompts[tokenId] = prompt;
        tokenModelHash[tokenId] = modelHash;

        emit AIGenerated(tokenId, to, prompt, modelHash);

        return tokenId;
    }

    /**
     * @notice Owner mints without payment
     * @param to Recipient address
     * @param uri Token metadata URI
     * @return tokenId The minted token ID
     */
    function ownerMint(
        address to,
        string memory uri
    ) public onlyOwner returns (uint256) {
        if (maxSupply > 0) {
            require(_nextTokenId < maxSupply, "Max supply reached");
        }

        uint256 tokenId = _nextTokenId++;
        _safeMint(to, tokenId);
        _setTokenURI(tokenId, uri);

        return tokenId;
    }

    /**
     * @notice Set base URI for all tokens
     * @param baseURI New base URI
     */
    function setBaseURI(string memory baseURI) public onlyOwner {
        _baseTokenURI = baseURI;
    }

    /**
     * @notice Set mint price
     * @param _mintPrice New mint price in MDT wei
     */
    function setMintPrice(uint256 _mintPrice) public onlyOwner {
        mintPrice = _mintPrice;
    }

    /**
     * @notice Withdraw collected MDT to owner
     */
    function withdraw() public onlyOwner {
        uint256 balance = address(this).balance;
        require(balance > 0, "No balance");
        (bool success, ) = payable(owner()).call{value: balance}("");
        require(success, "Transfer failed");
    }

    /**
     * @notice Get total minted count
     * @return Total number of minted tokens
     */
    function totalMinted() public view returns (uint256) {
        return _nextTokenId;
    }

    // Required overrides
    function tokenURI(
        uint256 tokenId
    ) public view override(ERC721, ERC721URIStorage) returns (string memory) {
        return super.tokenURI(tokenId);
    }

    function supportsInterface(
        bytes4 interfaceId
    ) public view override(ERC721, ERC721URIStorage) returns (bool) {
        return super.supportsInterface(interfaceId);
    }

    function _baseURI() internal view override returns (string memory) {
        return _baseTokenURI;
    }
}
