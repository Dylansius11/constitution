// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;
import {VaultV2Honest} from "src/demo/VaultV2Honest.sol";
contract Delegatecall is VaultV2Honest {
    function execute(address target, bytes calldata data) external returns (bytes memory) {
        (bool ok, bytes memory result) = target.delegatecall(data);
        require(ok);
        return result;
    }
}
