require("@nomicfoundation/hardhat-toolbox");

/** @type import('hardhat/config').HardhatUserConfig */
module.exports = {
    solidity: {
        version: "0.8.20",
        settings: {
            viaIR: true,
            optimizer: {
                enabled: true,
                runs: 200
            }
        }
    },
    networks: {
        // Local luxtensor node
        luxtensor_local: {
            url: "http://127.0.0.1:8545",
            chainId: 1337,
            accounts: process.env.PRIVATE_KEY ? [process.env.PRIVATE_KEY] : []
        },
        // Testnet
        luxtensor_testnet: {
            url: process.env.TESTNET_RPC || "http://127.0.0.1:8545",
            chainId: 1337,
            accounts: process.env.PRIVATE_KEY ? [process.env.PRIVATE_KEY] : []
        },
        // Mainnet (future)
        luxtensor_mainnet: {
            url: process.env.MAINNET_RPC || "http://127.0.0.1:8545",
            chainId: 21000000, // MDT total supply as chain ID
            accounts: process.env.PRIVATE_KEY ? [process.env.PRIVATE_KEY] : []
        }
    },
    paths: {
        sources: "./src",
        tests: "./test",
        cache: "./cache",
        artifacts: "./artifacts"
    }
};
