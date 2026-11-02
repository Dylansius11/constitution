// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;
import {VaultV2Honest} from "src/demo/VaultV2Honest.sol";
contract ImplementationSlot is VaultV2Honest {
    function implementationSlot() external pure returns (bytes32) {
        return 0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc;
    }
}
