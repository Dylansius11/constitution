// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;

import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {VaultV1} from "./VaultV1.sol";

/// @notice V1-compatible upgrade with a cap on total recorded deposits.
contract VaultV2Honest is VaultV1 {
    /// @custom:storage-location erc7201:constitution.storage.Vault
    struct VaultV2Storage {
        IERC20 token;
        mapping(address => uint256) balances;
        uint256 totalDeposits;
        uint256 depositCap;
    }

    error DepositCapExceeded(uint256 cap, uint256 requestedTotal);

    function initializeV2(uint256 cap) public reinitializer(2) {
        _vaultV2Storage().depositCap = cap;
    }

    function depositCap() public view returns (uint256) {
        return _vaultV2Storage().depositCap;
    }

    function deposit(uint256 amount) public virtual override {
        VaultV2Storage storage s = _vaultV2Storage();
        uint256 requestedTotal = s.totalDeposits + amount;
        if (requestedTotal > s.depositCap) revert DepositCapExceeded(s.depositCap, requestedTotal);
        super.deposit(amount);
    }

    function _vaultV2Storage() internal pure returns (VaultV2Storage storage s) {
        bytes32 slot = VAULT_STORAGE_LOCATION;
        assembly ("memory-safe") { s.slot := slot }
    }
}
