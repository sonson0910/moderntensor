// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract SimpleStorage {
    uint256 public storedValue;

    constructor(uint256 initialValue) {
        storedValue = initialValue;
    }

    function set(uint256 newValue) public {
        storedValue = newValue;
    }

    function get() public view returns (uint256) {
        return storedValue;
    }
}
