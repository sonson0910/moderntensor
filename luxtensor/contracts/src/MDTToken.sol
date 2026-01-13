// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/**
 * @title MDTToken
 * @dev ModernTensor ERC20 Token with controlled minting and permanent lock capability
 *
 * Token Supply: 21,000,000 MDT (18 decimals)
 *
 * Allocation:
 * - Emission Rewards: 45% (9,450,000 MDT) - Minted to RewardsPool
 * - Ecosystem Grants: 12% (2,520,000 MDT) - Minted to DAO
 * - Team & Core Dev: 10% (2,100,000 MDT) - Minted to VestingContract
 * - Private Sale: 8% (1,680,000 MDT) - Minted to VestingContract
 * - IDO: 5% (1,050,000 MDT) - Minted to VestingContract
 * - DAO Treasury: 10% (2,100,000 MDT) - Minted to DAO Multi-sig
 * - Initial Liquidity: 5% (1,050,000 MDT) - Minted to Liquidity Pool
 * - Foundation Reserve: 5% (1,050,000 MDT) - Minted to Foundation Multi-sig
 */

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Burnable.sol";
import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Permit.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

contract MDTToken is ERC20, ERC20Burnable, ERC20Permit, Ownable {
    /// @notice Maximum total supply: 21,000,000 MDT
    uint256 public constant MAX_SUPPLY = 21_000_000 * 10**18;

    /// @notice Whether minting is permanently finished
    bool public mintingFinished = false;

    /// @notice TGE timestamp
    uint256 public tgeTimestamp;

    /// @notice Allocation categories
    enum Category {
        EmissionRewards,    // 45%
        EcosystemGrants,    // 12%
        TeamCoreDev,        // 10%
        PrivateSale,        // 8%
        IDO,                // 5%
        DaoTreasury,        // 10%
        InitialLiquidity,   // 5%
        FoundationReserve   // 5%
    }

    /// @notice Track minted amounts per category
    mapping(Category => uint256) public categoryMinted;

    /// @notice Category allocation amounts
    mapping(Category => uint256) public categoryAllocation;

    /// @notice Events
    event TGEExecuted(uint256 timestamp, uint256 totalMinted);
    event MintingFinished(uint256 timestamp);
    event CategoryMinted(Category indexed category, address indexed to, uint256 amount);

    /// @notice Modifier to check minting is still allowed
    modifier canMint() {
        require(!mintingFinished, "MDTToken: Minting permanently finished");
        _;
    }

    constructor()
        ERC20("ModernTensor", "MDT")
        ERC20Permit("ModernTensor")
        Ownable(msg.sender)
    {
        // Set allocation amounts
        categoryAllocation[Category.EmissionRewards] = MAX_SUPPLY * 45 / 100;    // 9,450,000 MDT
        categoryAllocation[Category.EcosystemGrants] = MAX_SUPPLY * 12 / 100;    // 2,520,000 MDT
        categoryAllocation[Category.TeamCoreDev] = MAX_SUPPLY * 10 / 100;        // 2,100,000 MDT
        categoryAllocation[Category.PrivateSale] = MAX_SUPPLY * 8 / 100;         // 1,680,000 MDT
        categoryAllocation[Category.IDO] = MAX_SUPPLY * 5 / 100;                 // 1,050,000 MDT
        categoryAllocation[Category.DaoTreasury] = MAX_SUPPLY * 10 / 100;        // 2,100,000 MDT
        categoryAllocation[Category.InitialLiquidity] = MAX_SUPPLY * 5 / 100;    // 1,050,000 MDT
        categoryAllocation[Category.FoundationReserve] = MAX_SUPPLY * 5 / 100;   // 1,050,000 MDT
    }

    /**
     * @notice Mint tokens for a specific category
     * @param category The allocation category
     * @param to Recipient address
     * @param amount Amount to mint
     */
    function mintCategory(Category category, address to, uint256 amount) external onlyOwner canMint {
        require(to != address(0), "MDTToken: mint to zero address");
        require(
            categoryMinted[category] + amount <= categoryAllocation[category],
            "MDTToken: exceeds category allocation"
        );
        require(
            totalSupply() + amount <= MAX_SUPPLY,
            "MDTToken: exceeds max supply"
        );

        categoryMinted[category] += amount;
        _mint(to, amount);

        emit CategoryMinted(category, to, amount);
    }

    /**
     * @notice Execute TGE - mint full allocation for a category
     * @param category The allocation category
     * @param to Recipient address (vesting contract, DAO, etc.)
     */
    function executeTGE(Category category, address to) external onlyOwner canMint {
        require(to != address(0), "MDTToken: mint to zero address");
        require(categoryMinted[category] == 0, "MDTToken: category already minted");

        uint256 amount = categoryAllocation[category];
        categoryMinted[category] = amount;
        _mint(to, amount);

        if (tgeTimestamp == 0) {
            tgeTimestamp = block.timestamp;
        }

        emit CategoryMinted(category, to, amount);
    }

    /**
     * @notice Check if all categories have been minted
     */
    function allCategoriesMinted() public view returns (bool) {
        return categoryMinted[Category.EmissionRewards] == categoryAllocation[Category.EmissionRewards] &&
               categoryMinted[Category.EcosystemGrants] == categoryAllocation[Category.EcosystemGrants] &&
               categoryMinted[Category.TeamCoreDev] == categoryAllocation[Category.TeamCoreDev] &&
               categoryMinted[Category.PrivateSale] == categoryAllocation[Category.PrivateSale] &&
               categoryMinted[Category.IDO] == categoryAllocation[Category.IDO] &&
               categoryMinted[Category.DaoTreasury] == categoryAllocation[Category.DaoTreasury] &&
               categoryMinted[Category.InitialLiquidity] == categoryAllocation[Category.InitialLiquidity] &&
               categoryMinted[Category.FoundationReserve] == categoryAllocation[Category.FoundationReserve];
    }

    /**
     * @notice Permanently finish minting - IRREVERSIBLE
     * @dev Can only be called after all categories are minted
     *      After this, no more tokens can ever be minted
     */
    function finishMinting() external onlyOwner canMint {
        require(allCategoriesMinted(), "MDTToken: not all categories minted");
        require(totalSupply() == MAX_SUPPLY, "MDTToken: supply mismatch");

        mintingFinished = true;

        emit MintingFinished(block.timestamp);
        emit TGEExecuted(tgeTimestamp, totalSupply());

        // Renounce ownership - no one can mint ever again
        renounceOwnership();
    }

    /**
     * @notice Get remaining mintable amount for a category
     */
    function remainingAllocation(Category category) external view returns (uint256) {
        return categoryAllocation[category] - categoryMinted[category];
    }

    /**
     * @notice Get all category allocations
     */
    function getAllocations() external view returns (
        uint256 emissionRewards,
        uint256 ecosystemGrants,
        uint256 teamCoreDev,
        uint256 privateSale,
        uint256 ido,
        uint256 daoTreasury,
        uint256 initialLiquidity,
        uint256 foundationReserve
    ) {
        return (
            categoryAllocation[Category.EmissionRewards],
            categoryAllocation[Category.EcosystemGrants],
            categoryAllocation[Category.TeamCoreDev],
            categoryAllocation[Category.PrivateSale],
            categoryAllocation[Category.IDO],
            categoryAllocation[Category.DaoTreasury],
            categoryAllocation[Category.InitialLiquidity],
            categoryAllocation[Category.FoundationReserve]
        );
    }
}
