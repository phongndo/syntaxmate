// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {IERC20 as Token} from "./IERC20.sol";

/// @title Café Vault 🛰️
/// @notice Stores one balance for each visitor.
contract CafeVault {
    struct Visit { address guest; uint256 amount; }
    event Deposited(address indexed guest, uint256 amount);
    error EmptyDeposit(address guest);
    mapping(address => uint256) public balances;

    modifier positive(uint256 amount) {
        require(amount > 0, unicode"montant zéro 🧪");
        _;
    }

    function deposit() external payable positive(msg.value) {
        balances[msg.sender] += msg.value;
        emit Deposited(msg.sender, msg.value);
    }

    function label() public pure returns (string memory) {
        return unicode"café 東京 🚀";
    }
}
