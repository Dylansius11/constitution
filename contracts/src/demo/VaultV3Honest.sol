// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;

import {VaultV2Honest} from "./VaultV2Honest.sol";

/// @notice V2-compatible capacity preview for deposit interfaces; no new initializer or fees.
/// @dev Inherits the unchanged V2 storage namespace and all existing observable behavior.
contract VaultV3Honest is VaultV2Honest {
    function remainingCapacity() external view returns (uint256) {
        VaultStorage storage s = _vaultStorage();
        return s.totalDeposits < s.depositCap ? s.depositCap - s.totalDeposits : 0;
    }
}
