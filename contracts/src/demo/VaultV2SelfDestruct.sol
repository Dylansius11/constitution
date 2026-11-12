// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;

import {VaultV2Honest} from "./VaultV2Honest.sol";

/// @notice Proposed emergency shutdown extension; ERC20 accounting and the V2 cap are unchanged.
/// @dev Negative R3 fixture. SELFDESTRUCT is forbidden even behind access control. On Prague it
/// transfers native currency but does not delete an existing proxy's code (EIP-6780).
contract VaultV2SelfDestruct is VaultV2Honest {
    // Fixed local fixture authority, not a real multisig or a deployment claim. A constant adds
    // no storage and keeps runtime hashes identical in the proposer and verifier compilers.
    address public constant EMERGENCY_COUNCIL = address(0x4444);
    error NotShutdownOwner();

    function emergencyShutdown(address payable recipient) external {
        if (msg.sender != EMERGENCY_COUNCIL) revert NotShutdownOwner();
        selfdestruct(recipient);
    }
}
