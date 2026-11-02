// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;
import {Initializable} from "@openzeppelin/contracts/proxy/utils/Initializable.sol";
contract LayoutBreaking is Initializable {
    /// @custom:storage-location erc7201:constitution.storage.Vault
    struct VaultStorage {
        uint256 totalDeposits;
        mapping(address => uint256) balances;
        address token;
    }
    bytes32 constant SLOT = 0x199cdf1a4abaec4823e24931ac666120eb2410beefcb1fe17a5d0d2c4f416800;
    function totalDeposits() external view returns (uint256 result) {
        bytes32 slot = SLOT;
        VaultStorage storage s;
        assembly { s.slot := slot }
        return s.totalDeposits;
    }
}
