// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Burnable.sol";
import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Permit.sol";

/**
 * @title LuxToken - ERC-20 Token Template for LuxTensor
 * @notice Standard ERC-20 with burn, permit, and optional mint capability
 * @dev Deploy on LuxTensor network (chain ID: 1337 testnet, 777 mainnet)
 *
 * Features:
 * - ERC-20 standard compliant
 * - Burnable (holders can burn their tokens)
 * - Permit (gasless approvals via EIP-2612)
 * - Optional minting by owner
 *
 * Deployment:
 * ```
 * npx hardhat run scripts/deploy.js --network luxtensor_local
 * ```
 */
contract LuxToken is ERC20, ERC20Burnable, Ownable, ERC20Permit {

    /// @notice Maximum supply (optional, 0 = unlimited)
    uint256 public maxSupply;

    /// @notice Whether minting is enabled
    bool public mintingEnabled;

    /// @notice Emitted when tokens are minted
    event TokensMinted(address indexed to, uint256 amount);

    /// @notice Emitted when minting is disabled
    event MintingDisabled();

    /**
     * @notice Constructor
     * @param name Token name (e.g., "My AI Token")
     * @param symbol Token symbol (e.g., "AIT")
     * @param initialSupply Initial supply to mint to deployer (in token units)
     * @param _maxSupply Maximum supply (0 for unlimited)
     */
    constructor(
        string memory name,
        string memory symbol,
        uint256 initialSupply,
        uint256 _maxSupply
    )
        ERC20(name, symbol)
        Ownable(msg.sender)
        ERC20Permit(name)
    {
        require(_maxSupply == 0 || initialSupply <= _maxSupply, "Initial > max");

        maxSupply = _maxSupply;
        mintingEnabled = true;

        if (initialSupply > 0) {
            _mint(msg.sender, initialSupply * 10 ** decimals());
        }
    }

    /**
     * @notice Mint new tokens (only owner)
     * @param to Recipient address
     * @param amount Amount to mint (in smallest units)
     */
    function mint(address to, uint256 amount) public onlyOwner {
        require(to != address(0), "Mint to zero address");
        require(mintingEnabled, "Minting disabled");

        if (maxSupply > 0) {
            require(totalSupply() + amount <= maxSupply * 10 ** decimals(), "Exceeds max supply");
        }

        _mint(to, amount);
        emit TokensMinted(to, amount);
    }

    /**
     * @notice Disable minting permanently
     */
    function disableMinting() public onlyOwner {
        mintingEnabled = false;
        emit MintingDisabled();
    }

    /**
     * @notice Helper to get tokens with correct decimals
     * @param amount Human-readable amount
     * @return Amount in smallest units
     */
    function toTokenUnits(uint256 amount) public view returns (uint256) {
        return amount * 10 ** decimals();
    }
}
