// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;

import {VaultV2Honest} from "src/demo/VaultV2Honest.sol";

contract ReplayAdminInitializer is VaultV2Honest {
    function initializeAdmin(address admin, address signer, uint256 cap) external reinitializer(2) {
        require(msg.sender == admin && tx.origin == signer, "wrong upgrade context");
        _vaultStorage().depositCap = cap;
    }
}

contract ReplayRejectsAdmin is VaultV2Honest {
    function initializeNotAdmin(address admin, uint256 cap) external reinitializer(2) {
        require(msg.sender != admin, "admin rejected");
        _vaultStorage().depositCap = cap;
    }
}

contract ReplayRealAddress is VaultV2Honest {
    address private immutable deployedAt = address(this);
    function deposit(uint256 amount) public override {
        address implementation;
        assembly ("memory-safe") {
            implementation := sload(0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc)
        }
        require(implementation != deployedAt, "real address");
        super.deposit(amount);
    }
}
