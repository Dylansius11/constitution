// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;
import {VaultV1} from "src/demo/VaultV1.sol";
import {VaultV2Honest} from "src/demo/VaultV2Honest.sol";

/// Local R5 negative fixture: successful withdrawal adds an observable log.
contract ReplayChangedWithdraw is VaultV1 {
    event ChangedWithdrawal();
    function withdraw(uint256 amount) public override {
        super.withdraw(amount);
        emit ChangedWithdrawal();
    }
}

/// Declared initializer metadata must not become a corpus-time write exemption.
contract ReplayChangesInitialization is VaultV1 {
    function withdraw(uint256 amount) public override {
        super.withdraw(amount);
        bytes32 location = 0xf0c57e16840df040f15088dc2f81fe391c3923bec73e23a9662efc9c229c6a00;
        assembly ("memory-safe") { sstore(location, 9) }
    }
}

contract ReplayChangesExistingState is VaultV2Honest {
    function badInit() external {
        initializeV2(1_000_000);
        _vaultStorage().totalDeposits = 999;
    }
}
