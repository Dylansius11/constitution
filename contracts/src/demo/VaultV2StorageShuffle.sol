// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;

import {Initializable} from "@openzeppelin/contracts/proxy/utils/Initializable.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

/// @notice Proposed V2 groups the deposit total before account balances.
/// @dev Deliberately incompatible fixture: moving members inside an existing ERC-7201 namespace
/// does not migrate the state already written by V1. R1 must reject this upgrade.
contract VaultV2StorageShuffle is Initializable {
    using SafeERC20 for IERC20;

    /// @custom:storage-location erc7201:constitution.storage.Vault
    struct VaultStorage {
        IERC20 token;
        uint256 totalDeposits;
        mapping(address => uint256) balances;
    }

    // The namespace is unchanged; only its member order is (incorrectly) changed.
    bytes32 internal constant VAULT_STORAGE_LOCATION = 0x199cdf1a4abaec4823e24931ac666120eb2410beefcb1fe17a5d0d2c4f416800;

    event Deposited(address indexed account, uint256 amount);
    event Withdrawn(address indexed account, uint256 amount);
    error ZeroToken();
    error InsufficientBalance(uint256 balance, uint256 amount);

    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() { _disableInitializers(); }

    function initialize(IERC20 token) public initializer {
        if (address(token) == address(0)) revert ZeroToken();
        _vaultStorage().token = token;
    }

    function deposit(uint256 amount) public {
        VaultStorage storage s = _vaultStorage();
        s.balances[msg.sender] += amount;
        s.totalDeposits += amount;
        s.token.safeTransferFrom(msg.sender, address(this), amount);
        emit Deposited(msg.sender, amount);
    }

    function withdraw(uint256 amount) public {
        VaultStorage storage s = _vaultStorage();
        uint256 balance = s.balances[msg.sender];
        if (amount > balance) revert InsufficientBalance(balance, amount);
        s.balances[msg.sender] = balance - amount;
        s.totalDeposits -= amount;
        s.token.safeTransfer(msg.sender, amount);
        emit Withdrawn(msg.sender, amount);
    }

    function balanceOf(address account) public view returns (uint256) {
        return _vaultStorage().balances[account];
    }

    function totalDeposits() public view returns (uint256) {
        return _vaultStorage().totalDeposits;
    }

    function _vaultStorage() internal pure returns (VaultStorage storage s) {
        bytes32 slot = VAULT_STORAGE_LOCATION;
        assembly ("memory-safe") { s.slot := slot }
    }
}
